//! How many Stars a game may pay out.
//!
//! Stars are real money in this shop — 1⭐ = 1฿ at checkout. A game that
//! credits them is a mint, and `/api/stars/add` accepts any amount up to a
//! million per call from anyone who can authenticate. Without a ceiling per
//! source, one player with a paused game loop, or one person replaying the
//! request by hand, walks off with the till.
//!
//! Lives here rather than in the handler because `src/ui` and `src/api` cannot
//! share code otherwise, and because a rule about money deserves tests that
//! run on every build.

/// Sources whose payouts are capped because a game, not a purchase, produced
/// them. Anything else — a manual admin credit, a refund — is deliberate and
/// not throttled here.
// `skate` remains for historical ledger rows and replay checks. New Ride runs
// use `ride`; removing the old spelling would make an old idempotent retry
// bypass the cap merely because the UI was renamed.
pub const GAME_SOURCES: &[&str] = &["ride", "skate", "woodshop", "game"];

/// Most Stars a single game source may pay one player in 24 hours.
///
/// Chosen against the shop: 300฿ is roughly one small order, so a dedicated
/// player earns a real but bounded discount, and an exploit is worth a capped
/// amount rather than an unbounded one.
pub const DAILY_GAME_STARS_CAP: i64 = 300;

/// Whether this source is subject to the daily cap.
pub fn is_game_source(source: &str) -> bool {
    GAME_SOURCES.contains(&source)
}

/// How much of a requested credit may actually be paid, given what this source
/// has already paid the player today.
///
/// Returns 0 when the cap is already reached — the caller should then accept
/// the request without crediting rather than fail it, so a player who has hit
/// the ceiling still finishes their run instead of seeing an error.
pub fn allowed_credit(requested: i64, already_today: i64) -> i64 {
    if requested <= 0 {
        return 0;
    }
    let remaining = DAILY_GAME_STARS_CAP.saturating_sub(already_today.max(0));
    requested.min(remaining).max(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_player_gets_what_they_earned() {
        assert_eq!(allowed_credit(50, 0), 50);
    }

    #[test]
    fn a_credit_is_trimmed_to_what_is_left_of_the_cap() {
        assert_eq!(allowed_credit(100, DAILY_GAME_STARS_CAP - 30), 30);
    }

    #[test]
    fn nothing_is_paid_once_the_cap_is_reached() {
        assert_eq!(allowed_credit(100, DAILY_GAME_STARS_CAP), 0);
        assert_eq!(allowed_credit(100, DAILY_GAME_STARS_CAP + 999), 0);
    }

    #[test]
    fn the_cap_cannot_be_beaten_by_repeating_a_request() {
        // The exploit this exists for: replaying the payout call. Each replay
        // sees the previous total, so the sum can never pass the ceiling.
        let mut paid = 0;
        for _ in 0..100 {
            paid += allowed_credit(250, paid);
        }
        assert_eq!(paid, DAILY_GAME_STARS_CAP);
    }

    #[test]
    fn nonsense_amounts_pay_nothing() {
        assert_eq!(allowed_credit(0, 0), 0);
        assert_eq!(allowed_credit(-500, 0), 0);
    }

    #[test]
    fn corrupt_history_cannot_raise_the_ceiling() {
        // A negative "already paid" must not turn into extra headroom.
        assert_eq!(allowed_credit(10_000, -1_000), DAILY_GAME_STARS_CAP);
    }

    #[test]
    fn only_game_sources_are_capped() {
        assert!(is_game_source("ride"));
        assert!(is_game_source("skate"));
        assert!(is_game_source("woodshop"));
        // An admin correction is deliberate and must not be silently trimmed.
        assert!(!is_game_source("admin_grant"));
        assert!(!is_game_source("refund"));
    }
}
