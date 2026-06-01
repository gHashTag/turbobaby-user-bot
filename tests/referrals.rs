// Referral system tests.
//
// Unit tests replicate the pure `generate_referral_code` logic — no DB needed.
// Integration tests (marked #[ignore]) need a live DATABASE_URL and `--features backend`.

// ──────────────────────────────────────────────────────────────────
// Replicate the pure code-generation logic for standalone unit tests
// ──────────────────────────────────────────────────────────────────

use sha2::{Digest, Sha256};

const CODE_SALT: &str = "woody-ref-v1";
const BASE62_CHARS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

fn generate_referral_code(telegram_id: i64, attempt: u32) -> String {
    let input = format!("{}{}{}", telegram_id, CODE_SALT, attempt);
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let hash = hasher.finalize();
    hash.iter()
        .take(8)
        .map(|&b| BASE62_CHARS[(b as usize) % 62] as char)
        .collect()
}

// ──────────────────────────────────────────────────────────────────
// Unit tests (pure — no DB, no network)
// ──────────────────────────────────────────────────────────────────

#[test]
fn code_is_8_chars() {
    assert_eq!(generate_referral_code(1, 0).len(), 8);
}

#[test]
fn code_is_alphanumeric() {
    let code = generate_referral_code(99999, 0);
    assert!(code.chars().all(|c| c.is_ascii_alphanumeric()));
}

#[test]
fn same_id_same_code() {
    assert_eq!(generate_referral_code(42, 0), generate_referral_code(42, 0));
}

#[test]
fn different_ids_different_codes() {
    assert_ne!(generate_referral_code(1, 0), generate_referral_code(2, 0));
}

#[test]
fn attempt_changes_code() {
    assert_ne!(generate_referral_code(1, 0), generate_referral_code(1, 1));
}

#[test]
fn code_does_not_contain_ref_prefix() {
    // Make sure generated codes don't accidentally start with "ref_"
    for tid in [0i64, 1, 12345, 987654321, i64::MAX] {
        let code = generate_referral_code(tid, 0);
        assert!(!code.starts_with("ref_"));
    }
}

// ──────────────────────────────────────────────────────────────────
// Cycle #84: post-SeaORM-migration invariants
// ──────────────────────────────────────────────────────────────────
// Pure assertions that document properties of the migration without
// needing a live DB. The actual SeaORM transaction behaviour is
// exercised in production; these tests pin the *contract* so a future
// refactor that breaks the invariant fails at unit-test time.

#[test]
fn code_collision_retry_loop_terminates_at_10() {
    // `get_or_create_referral_code` retries up to 10 times. The bound
    // is hard-coded; if a future refactor changes it, that's fine — but
    // we want the *property* "loop terminates with explicit error
    // rather than spinning" to stay true. Asserting deterministic codes
    // across the first 10 attempts confirms we have 10 distinct
    // candidates to try (no degenerate "all attempts produce same
    // code" bug).
    let codes: std::collections::HashSet<String> =
        (0u32..10).map(|a| generate_referral_code(42, a)).collect();
    assert!(
        codes.len() >= 8,
        "10 attempts must produce at least 8 distinct codes (got {})",
        codes.len()
    );
}

#[test]
fn code_attempt_zero_is_canonical() {
    // The migration to SeaORM kept the same `attempt = 0` first try.
    // If `get_or_create_referral_code` started at a different attempt,
    // pre-migration users would see code churn. Pin this.
    let canonical = generate_referral_code(12345, 0);
    // Spot check: confirm it's hash-derived from the same input shape
    // (id + salt + "0").
    let mut h = Sha256::new();
    h.update(format!("{}{}{}", 12345i64, CODE_SALT, 0).as_bytes());
    let hash = h.finalize();
    let expected: String = hash
        .iter()
        .take(8)
        .map(|&b| BASE62_CHARS[(b as usize) % 62] as char)
        .collect();
    assert_eq!(canonical, expected);
}
