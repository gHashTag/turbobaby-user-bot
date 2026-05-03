/// Referral system tests.
///
/// Unit tests replicate the pure `generate_referral_code` logic — no DB needed.
/// Integration tests (marked #[ignore]) need a live DATABASE_URL and `--features backend`.

// ──────────────────────────────────────────────────────────────────
// Replicate the pure code-generation logic for standalone unit tests
// ──────────────────────────────────────────────────────────────────

use sha2::{Sha256, Digest};

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
    assert_eq!(
        generate_referral_code(42, 0),
        generate_referral_code(42, 0)
    );
}

#[test]
fn different_ids_different_codes() {
    assert_ne!(
        generate_referral_code(1, 0),
        generate_referral_code(2, 0)
    );
}

#[test]
fn attempt_changes_code() {
    assert_ne!(
        generate_referral_code(1, 0),
        generate_referral_code(1, 1)
    );
}

#[test]
fn code_does_not_contain_ref_prefix() {
    // Make sure generated codes don't accidentally start with "ref_"
    for tid in [0i64, 1, 12345, 987654321, i64::MAX] {
        let code = generate_referral_code(tid, 0);
        assert!(!code.starts_with("ref_"));
    }
}
