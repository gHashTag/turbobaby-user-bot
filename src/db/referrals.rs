use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use deadpool_postgres::Pool;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

// ──────────────────────────────────────────────────────────────────
// Domain types
// ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ReferralEvent {
    pub id: Uuid,
    pub referrer_id: i64,
    pub referred_id: i64,
    pub code: String,
    pub bonus_paid: f64,
    pub status: String,
    pub source: Option<String>,
    pub created_at: DateTime<Utc>,
    pub confirmed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferrerStats {
    pub total_invited: i64,
    pub confirmed: i64,
    pub pending: i64,
    pub total_bonus_earned: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopReferrer {
    pub telegram_id: i64,
    pub first_name: Option<String>,
    pub referral_count: i64,
    pub total_bonus_earned: f64,
}

// ──────────────────────────────────────────────────────────────────
// Code generation
// ──────────────────────────────────────────────────────────────────

/// Salt used when generating referral codes — not a secret, just prevents
/// trivial enumeration of sequential IDs.
const CODE_SALT: &str = "woody-ref-v1";

const BASE62_CHARS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

/// Generate an 8-character base-62 referral code derived from telegram_id + salt.
/// The `attempt` parameter is incremented on collision to produce a different code.
pub fn generate_referral_code(telegram_id: i64, attempt: u32) -> String {
    let input = format!("{}{}{}", telegram_id, CODE_SALT, attempt);
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let hash = hasher.finalize();

    // Take first 8 bytes and map each byte to a base-62 character
    hash.iter()
        .take(8)
        .map(|&b| BASE62_CHARS[(b as usize) % 62] as char)
        .collect()
}

/// Get existing referral code for `telegram_id`, or generate and persist a new one.
/// Retries on collision (up to 10 attempts).
pub async fn get_or_create_referral_code(pool: &Pool, telegram_id: i64) -> Result<String> {
    let client = pool.get().await.context("db pool")?;

    // Check existing code first
    if let Some(row) = client
        .query_opt(
            "SELECT referral_code FROM loyalty_profiles WHERE telegram_id = $1 AND referral_code IS NOT NULL",
            &[&telegram_id],
        )
        .await?
    {
        let code: String = row.try_get("referral_code").unwrap_or_default();
        return Ok(code);
    }

    // Ensure profile row exists
    client
        .execute(
            "INSERT INTO loyalty_profiles (telegram_id) VALUES ($1) ON CONFLICT (telegram_id) DO NOTHING",
            &[&telegram_id],
        )
        .await?;

    // Try up to 10 times to find a unique code
    for attempt in 0u32..10 {
        let code = generate_referral_code(telegram_id, attempt);

        let updated = client
            .execute(
                "UPDATE loyalty_profiles SET referral_code = $1
                 WHERE telegram_id = $2 AND referral_code IS NULL",
                &[&code, &telegram_id],
            )
            .await?;

        if updated > 0 {
            return Ok(code);
        }

        // Check if our telegram_id now has a code (another concurrent request may have set it)
        if let Some(row) = client
            .query_opt(
                "SELECT referral_code FROM loyalty_profiles WHERE telegram_id = $1 AND referral_code IS NOT NULL",
                &[&telegram_id],
            )
            .await?
        {
            return Ok(row.try_get("referral_code").unwrap_or_default());
        }
        // Otherwise the code was taken by someone else — try next attempt
    }

    anyhow::bail!(
        "Failed to generate unique referral code for telegram_id={}",
        telegram_id
    )
}

// ──────────────────────────────────────────────────────────────────
// Referral lifecycle
// ──────────────────────────────────────────────────────────────────

/// Find the telegram_id of the owner of `code` (looks in loyalty_profiles.referral_code).
pub async fn find_referrer_by_code(pool: &Pool, code: &str) -> Result<Option<i64>> {
    let client = pool.get().await.context("db pool")?;
    let row = client
        .query_opt(
            "SELECT telegram_id FROM loyalty_profiles WHERE referral_code = $1",
            &[&code],
        )
        .await?;
    Ok(row.map(|r| r.try_get("telegram_id").unwrap_or(0)))
}

/// Record a new pending referral event.
/// Returns the new event UUID.
pub async fn record_referral(
    pool: &Pool,
    referrer_id: i64,
    referred_id: i64,
    code: &str,
    source: Option<&str>,
) -> Result<Uuid> {
    let client = pool.get().await.context("db pool")?;
    let id = Uuid::new_v4();

    // Ensure referred user has a loyalty profile row
    client
        .execute(
            "INSERT INTO loyalty_profiles (telegram_id, referred_by)
             VALUES ($1, $2)
             ON CONFLICT (telegram_id) DO UPDATE SET referred_by = COALESCE(loyalty_profiles.referred_by, EXCLUDED.referred_by)",
            &[&referred_id, &referrer_id],
        )
        .await?;

    // Insert event — ignore if the referred_id already has an event (UNIQUE constraint)
    client
        .execute(
            "INSERT INTO referral_events (id, referrer_id, referred_id, code, status, source)
             VALUES ($1, $2, $3, $4, 'pending', $5)
             ON CONFLICT (referred_id) DO NOTHING",
            &[&id, &referrer_id, &referred_id, &code, &source],
        )
        .await?;

    Ok(id)
}

/// Confirm a referral (first purchase of `referred_id`).
/// - Sets status = 'confirmed' + confirmed_at
/// - Credits bonus to referrer's balance via bonus_transactions
/// - Increments referrer's referral_count
pub async fn confirm_referral(pool: &Pool, referred_id: i64, bonus: f64) -> Result<()> {
    if !bonus.is_finite() || bonus < 0.0 {
        anyhow::bail!("invalid bonus: {}", bonus);
    }
    let mut client = pool.get().await.context("db pool")?;
    let tx = client.transaction().await.context("start tx")?;

    // Find the pending event (FOR UPDATE prevents double-credit races)
    let row = tx
        .query_opt(
            "SELECT id, referrer_id FROM referral_events
             WHERE referred_id = $1 AND status = 'pending' FOR UPDATE",
            &[&referred_id],
        )
        .await?;

    let (event_id, referrer_id): (Uuid, i64) = match row {
        Some(r) => (
            r.try_get("id").unwrap_or_default(),
            r.try_get("referrer_id").unwrap_or(0),
        ),
        None => {
            // Cycle #76: was `.ok()` — pool/connection issues here were
            // invisible. Empty-tx commit has no data effects to lose, but
            // a failure still signals real infra trouble worth seeing.
            if let Err(e) = tx.commit().await {
                tracing::warn!(
                    "referrals.confirm_referral: empty-tx commit failed for referred_id={}: {}",
                    referred_id,
                    e
                );
            }
            return Ok(()); // nothing pending — semantically a no-op
        }
    };

    // Mark confirmed
    tx.execute(
        "UPDATE referral_events
         SET status = 'confirmed', confirmed_at = NOW(), bonus_paid = $1
         WHERE id = $2",
        &[&bonus, &event_id],
    )
    .await?;

    // Ensure referrer loyalty profile exists
    tx.execute(
        "INSERT INTO loyalty_profiles (telegram_id, bonus_balance, total_spent) VALUES ($1, 0, 0) ON CONFLICT (telegram_id) DO NOTHING",
        &[&referrer_id],
    ).await?;

    // Credit bonus to referrer
    let tx_id = Uuid::new_v4().to_string();
    tx.execute(
        "INSERT INTO bonus_transactions (id, telegram_id, amount, tx_type, description)
         VALUES ($1, $2, $3, 'referral_bonus', 'Referral bonus for new user')",
        &[&tx_id, &referrer_id, &bonus],
    )
    .await?;

    let updated = tx
        .execute(
            "UPDATE loyalty_profiles SET bonus_balance = bonus_balance + $1 WHERE telegram_id = $2",
            &[&bonus, &referrer_id],
        )
        .await?;
    if updated == 0 {
        if let Err(e) = tx.rollback().await {
            tracing::error!("confirm_referral rollback error: {}", e);
        }
        anyhow::bail!(
            "confirm_referral: loyalty profile missing for referrer_id={}",
            referrer_id
        );
    }

    // Increment referral_count
    tx.execute(
        "UPDATE loyalty_profiles SET referral_count = referral_count + 1 WHERE telegram_id = $1",
        &[&referrer_id],
    )
    .await?;

    tx.commit().await.context("commit referral tx")?;
    Ok(())
}

// ──────────────────────────────────────────────────────────────────
// Statistics / Leaderboard
// ──────────────────────────────────────────────────────────────────

/// Return referral statistics for a specific user.
pub async fn get_referrer_stats(pool: &Pool, telegram_id: i64) -> Result<ReferrerStats> {
    let client = pool.get().await.context("db pool")?;

    let row = client
        .query_one(
            "SELECT
                COUNT(*)                                    AS total_invited,
                COUNT(*) FILTER (WHERE status = 'confirmed' OR status = 'paid') AS confirmed,
                COUNT(*) FILTER (WHERE status = 'pending')  AS pending,
                COALESCE(SUM(bonus_paid)::float8, 0)        AS total_bonus_earned
             FROM referral_events
             WHERE referrer_id = $1",
            &[&telegram_id],
        )
        .await?;

    Ok(ReferrerStats {
        total_invited: row.try_get::<_, i64>("total_invited").unwrap_or(0),
        confirmed: row.try_get::<_, i64>("confirmed").unwrap_or(0),
        pending: row.try_get::<_, i64>("pending").unwrap_or(0),
        total_bonus_earned: {
            let v = row.try_get::<_, f64>("total_bonus_earned").unwrap_or(0.0);
            if v.is_finite() {
                v.max(0.0)
            } else {
                0.0
            }
        },
    })
}

/// Return top referrers leaderboard.
/// `period` accepts "weekly" | "monthly" | "all" (anything else → all-time).
pub async fn get_top_referrers(pool: &Pool, period: &str, limit: i64) -> Result<Vec<TopReferrer>> {
    let client = pool.get().await.context("db pool")?;

    let sql = match period {
        "weekly" => {
            "SELECT
                re.referrer_id                              AS telegram_id,
                COALESCE(MAX(ul.first_name), 'Anonymous') AS first_name,
                COUNT(*)                                    AS referral_count,
                COALESCE(SUM(re.bonus_paid)::float8, 0)     AS total_bonus_earned
             FROM referral_events re
             LEFT JOIN user_languages ul ON re.referrer_id = ul.telegram_id
             WHERE (re.status = 'confirmed' OR re.status = 'paid')
             AND re.created_at >= NOW() - INTERVAL '7 days'
             GROUP BY re.referrer_id
             ORDER BY referral_count DESC, total_bonus_earned DESC
             LIMIT $1"
        }
        "monthly" => {
            "SELECT
                re.referrer_id                              AS telegram_id,
                COALESCE(MAX(ul.first_name), 'Anonymous') AS first_name,
                COUNT(*)                                  AS referral_count,
                COALESCE(SUM(re.bonus_paid)::float8, 0)   AS total_bonus_earned
             FROM referral_events re
             LEFT JOIN user_languages ul ON re.referrer_id = ul.telegram_id
             WHERE (re.status = 'confirmed' OR re.status = 'paid')
             AND re.created_at >= NOW() - INTERVAL '30 days'
             GROUP BY re.referrer_id
             ORDER BY referral_count DESC, total_bonus_earned DESC
             LIMIT $1"
        }
        _ => {
            "SELECT
                re.referrer_id                              AS telegram_id,
                COALESCE(MAX(ul.first_name), 'Anonymous') AS first_name,
                COUNT(*)                                  AS referral_count,
                COALESCE(SUM(re.bonus_paid)::float8, 0)   AS total_bonus_earned
             FROM referral_events re
             LEFT JOIN user_languages ul ON re.referrer_id = ul.telegram_id
             WHERE (re.status = 'confirmed' OR re.status = 'paid')
             GROUP BY re.referrer_id
             ORDER BY referral_count DESC, total_bonus_earned DESC
             LIMIT $1"
        }
    };

    let rows = client.query(sql, &[&limit]).await?;

    Ok(rows
        .iter()
        .map(|r| TopReferrer {
            telegram_id: r.try_get::<_, i64>("telegram_id").unwrap_or(0),
            first_name: r.try_get::<_, String>("first_name").ok(),
            referral_count: r.try_get::<_, i64>("referral_count").unwrap_or(0),
            total_bonus_earned: {
                let v = r.try_get::<_, f64>("total_bonus_earned").unwrap_or(0.0);
                if v.is_finite() {
                    v.max(0.0)
                } else {
                    0.0
                }
            },
        })
        .collect())
}

// ──────────────────────────────────────────────────────────────────
// Unit tests (no DB required)
// ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_referral_code_length() {
        let code = generate_referral_code(123456789, 0);
        assert_eq!(code.len(), 8, "code must be exactly 8 chars");
    }

    #[test]
    fn test_generate_referral_code_charset() {
        let code = generate_referral_code(987654321, 0);
        assert!(
            code.chars().all(|c| c.is_ascii_alphanumeric()),
            "code must be alphanumeric base-62"
        );
    }

    #[test]
    fn test_generate_referral_code_deterministic() {
        let a = generate_referral_code(42, 0);
        let b = generate_referral_code(42, 0);
        assert_eq!(a, b, "same inputs → same code");
    }

    #[test]
    fn test_generate_referral_code_different_ids() {
        let a = generate_referral_code(1, 0);
        let b = generate_referral_code(2, 0);
        assert_ne!(a, b, "different ids → different codes");
    }

    #[test]
    fn test_generate_referral_code_attempt_changes_code() {
        let a = generate_referral_code(1, 0);
        let b = generate_referral_code(1, 1);
        assert_ne!(a, b, "different attempts → different codes");
    }
}
