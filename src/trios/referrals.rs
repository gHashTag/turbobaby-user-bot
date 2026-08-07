//! Pure referral helpers shared between backend and tests.

/// Deterministically assign a share source from the configured list based on
/// the user's Telegram ID. The same user always gets the same source, making
/// A/B groups stable across sessions and share attempts.
///
/// Returns a clone of the selected source string so callers don't hold a
/// reference into the config vector.
pub fn assign_share_source(telegram_id: i64, sources: &[String]) -> String {
    if sources.is_empty() {
        return "default".to_string();
    }
    // Simple deterministic hash: mix the id, then take a positive remainder.
    let hash = telegram_id.wrapping_mul(31).wrapping_add(17);
    let idx = hash.rem_euclid(sources.len() as i64) as usize;
    sources[idx].clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assign_share_source_is_deterministic() {
        let sources = vec!["utm_a".to_string(), "utm_b".to_string()];
        let a = assign_share_source(12345, &sources);
        let b = assign_share_source(12345, &sources);
        assert_eq!(a, b);
    }

    #[test]
    fn test_assign_share_source_balances_groups() {
        let sources = vec!["utm_a".to_string(), "utm_b".to_string()];
        let mut counts: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for tid in 1..=100 {
            *counts
                .entry(assign_share_source(tid, &sources))
                .or_insert(0) += 1;
        }
        // With 100 sequential IDs and 2 sources, expect near-even split.
        assert!(counts["utm_a"] > 30, "utm_a under-assigned: {counts:?}");
        assert!(counts["utm_b"] > 30, "utm_b under-assigned: {counts:?}");
    }

    #[test]
    fn test_assign_share_source_falls_back_for_empty() {
        assert_eq!(assign_share_source(42, &[]), "default");
    }

    #[test]
    fn test_assign_share_source_negative_id() {
        let sources = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let idx: i64 = -7;
        let result = assign_share_source(idx, &sources);
        assert!(sources.contains(&result));
    }
}
