//! The Mini App's half of the referral credit (owner, R3, 2026-09-26), held
//! to the shape the spec fixes.
//!
//! The owner's words, verbatim: «Должно начисляться исключительно за то кто
//! арендовал 10% скидка», «Пригласивший и может забрать скидкой за аренду или
//! деньгами», and on the points «Убрать, только скидка 10%». The server records
//! the rentals and keeps the ledger; the client shows the balance, lets the
//! customer hold it against a rental or ask for a payout, and prints nothing
//! the server did not send.
//!
//! A source scan, like `tests/customer_surface_wiring.rs`, because `src/lib.rs`
//! compiles `pub mod ui` only for wasm32 and `cargo test` therefore compiles no
//! line of it. The arithmetic the screens need is in `src/trios/referral_credit.rs`,
//! which the host does compile and test; what is checked here is that the
//! screens call it instead of doing their own.

use std::fs;
use std::path::{Path, PathBuf};

const PAGE: &str = "src/ui/pages/referrals.rs";
const PROFILE: &str = "src/ui/screens/profile_screen.rs";
const ADMIN: &str = "src/ui/screens/admin_screen.rs";
const I18N: &str = "src/trios/i18n.rs";

/// The admin panel and its helpers: everything from its first type to the end
/// of `admin_screen.rs`, where it was appended so that no cited line moved.
fn admin_panel_region(admin_code: &str) -> &str {
    let start = admin_code
        .find("struct ReferralRentalForm")
        .expect("the referral panel's form type is in admin_screen.rs");
    &admin_code[start..]
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// A shipped file, LF-normalised; its absence fails the test rather than
/// emptying it.
fn source(relative: &str) -> String {
    fs::read_to_string(repo_root().join(relative))
        .unwrap_or_else(|e| panic!("{relative} must be readable: {e}"))
        .replace("\r\n", "\n")
}

/// The source with every `//` comment removed (doc comments included), so
/// prose about a call is never counted as the call. Naive by intent: no needle
/// below contains `//`, and the one check that must read a URL reads the raw
/// source instead.
fn code_of(text: &str) -> String {
    text.lines()
        .map(|line| match line.find("//") {
            Some(at) => &line[..at],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The body of the first item whose header starts with `header`, braces
/// counted outside string literals.
fn body_of(text: &str, header: &str) -> String {
    let start = text
        .find(header)
        .unwrap_or_else(|| panic!("`{header}` is not in the file"));
    let open = start
        + text[start..]
            .find('{')
            .unwrap_or_else(|| panic!("`{header}` has no body"));
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    let mut in_string = false;
    let mut i = open;
    while i < bytes.len() {
        let c = bytes[i];
        if in_string {
            if c == b'\\' {
                i += 2;
                continue;
            }
            if c == b'"' {
                in_string = false;
            }
        } else if c == b'"' {
            in_string = true;
        } else if c == b'{' {
            depth += 1;
        } else if c == b'}' {
            depth -= 1;
            if depth == 0 {
                return text[open..=i].to_string();
            }
        }
        i += 1;
    }
    panic!("`{header}` has an unbalanced body");
}

/// Every `.rs` file under `dir`, recursively, as (repo-relative path, text).
fn rust_sources(dir: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut stack = vec![repo_root().join(dir)];
    while let Some(d) = stack.pop() {
        for entry in fs::read_dir(&d).unwrap_or_else(|e| panic!("read_dir {d:?}: {e}")) {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let relative = path
                    .strip_prefix(repo_root())
                    .expect("under the repo")
                    .display()
                    .to_string()
                    .replace('\\', "/");
                let text = fs::read_to_string(&path)
                    .unwrap_or_else(|e| panic!("{relative}: {e}"))
                    .replace("\r\n", "\n");
                out.push((relative, text));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn the_scan_reads_the_client_tree() {
    let ui = rust_sources("src/ui");
    assert!(
        ui.len() >= 40,
        "only {} files under src/ui: a walker that reads nothing passes every check below",
        ui.len()
    );
    assert!(ui.iter().any(|(path, _)| path == PAGE));
    assert!(ui.iter().any(|(path, _)| path == PROFILE));
}

/// The page reads the balance from its route, asks for a hold through the
/// customer's own transport, and prints the balance only through
/// `shown_balance` and the market formatter: a negative balance reads as zero,
/// and the currency is `format_baht`'s (D15).
#[test]
fn the_page_asks_the_server_and_prints_the_balance_through_the_shared_core() {
    let code = code_of(&source(PAGE));

    for needle in [
        // GET #1 and POST #2 of the spec's shared API.
        "\"{}/api/referral-credit/me/{}\"",
        "\"{}/api/referral-credit/me/{}/requests\"",
        "fetch_text_authed_full(",
        "post_json_authed(",
        // The body and both responses are the shared wire types, not a copy.
        "OpenRequestBody {",
        "OpenRequestResponse",
        "ReferralCredit",
        "use crate::trios::referral_credit::",
        "format_baht(shown_balance(state.balance_thb) as f64)",
        // The two kinds, spelled as the migration spells them.
        "open_request(\"redeem\"",
        "open_request(\"payout\"",
    ] {
        assert!(code.contains(needle), "the referral page lost `{needle}`");
    }

    // Every read of the balance goes through `shown_balance`; nothing prints
    // the signed figure.
    for line in code.lines().filter(|l| l.contains("balance_thb")) {
        assert!(
            line.contains("shown_balance("),
            "the balance is read without shown_balance: {}",
            line.trim()
        );
    }
}

/// After every request the page reads the balance again and shows what the
/// server holds; only when that holds no open request after a failure does it
/// print the generic error. A request of the other kind (409) therefore shows
/// the request that is open, not an error.
#[test]
fn a_request_is_followed_by_a_fresh_read_and_fails_only_into_the_generic_sentence() {
    let code = code_of(&source(PAGE));
    let request = body_of(&code, "fn open_request(");
    let post = request
        .find("post_json_authed(")
        .expect("the request posts");
    let refetch = request
        .find("fetch_credit(")
        .expect("the request reads the balance again");
    assert!(post < refetch, "the fresh read must follow the request");
    assert!(request.contains("failed.set(!something_open)"));

    let panel = body_of(&code, "fn credit_panel(");
    for needle in [
        "T_REFERRAL_BALANCE",
        "T_REFERRAL_APPLY_TO_RENTAL",
        "T_REFERRAL_REQUEST_PAYOUT",
        "T_REFERRAL_PAYOUT_REQUESTED",
        "T_API_ERR_UNKNOWN",
        // A held redemption: its own label with a tick, and pressed.
        "format!(\"\\u{2705} {}\", t(lang, T_REFERRAL_APPLY_TO_RENTAL))",
        "disabled: !can_request",
    ] {
        assert!(panel.contains(needle), "the balance block lost `{needle}`");
    }
    // A failed read hides the block rather than printing a zero.
    assert!(code.contains("if let Some(state) = balance {"));
    let fetch = body_of(&code, "async fn fetch_credit(");
    assert!(fetch.contains("Ok((200, text))"));
}

/// Retired with R3, and nowhere in shipped code: the ladder's three keys, the
/// fourth stat card's, the friend-facing share text, the empty friends panel's
/// promise and the profile's per-friend amount. Comments may name them (the
/// i18n table keeps one comment line per deleted line); code may not.
#[test]
fn the_retired_referral_keys_have_no_reader() {
    let retired = [
        "T_REFERRAL_MILESTONE_",
        "T_REFERRAL_SHARE_TEXT",
        "T_REFERRAL_STAT_BONUS",
        "T_REFERRAL_INVITEES_EMPTY",
        "T_PROFILE_EARN_PER_REF",
    ];
    let mut scanned = 0;
    let mut offences = Vec::new();
    for dir in ["src/ui", "src/trios"] {
        for (path, text) in rust_sources(dir) {
            scanned += 1;
            for (n, line) in code_of(&text).lines().enumerate() {
                for key in retired {
                    if line.contains(key) {
                        offences.push(format!("{path}:{}: {}", n + 1, line.trim()));
                    }
                }
            }
        }
    }
    assert!(scanned >= 60, "only {scanned} files scanned");
    assert!(
        offences.is_empty(),
        "a key R3 retired is back in code:\n  {}",
        offences.join("\n  ")
    );
    // And the page no longer asks for the ladder at all.
    let page = code_of(&source(PAGE));
    assert!(!page.contains("/milestones"));
    assert!(!page.contains("MilestonesResponse"));
}

/// The share button sends the link and nothing else: the text that rode along
/// promised the friend bonuses, and no sentence was worded to replace it.
#[test]
fn the_share_link_carries_no_text() {
    let raw = source(PAGE);
    assert!(
        raw.contains("\"https://t.me/share/url?url={}\""),
        "the share URL changed shape"
    );
    assert!(
        !raw.contains("&text="),
        "the share URL carries a text again"
    );
}

/// The empty friends panel is not rendered: its only sentence promised the
/// friend bonuses.
#[test]
fn the_empty_friends_panel_is_not_rendered() {
    let code = code_of(&source(PAGE));
    assert!(code.contains("if !list.is_empty() {"));
    let panel = body_of(&code, "fn invitees_panel(");
    assert!(
        !panel.contains("is_empty"),
        "the friends panel has an empty state of its own again"
    );
}

/// No screen computes the credit: the rate and the rounding live in
/// `crate::trios::referral_credit` alone. A client copy of the rate is right
/// until the day the owner changes it. The admin preview may PRINT the rate
/// (`{REFERRAL_CREDIT_PERCENT}% = ฿N`), but no line that names it may multiply
/// or divide, and neither the page nor the panel may spell a tenth of its own.
#[test]
fn no_percent_arithmetic_outside_the_shared_core() {
    let mut offences = Vec::new();
    let mut rate_readers = 0;
    for (path, text) in rust_sources("src/ui") {
        for (n, line) in code_of(&text).lines().enumerate() {
            if line.contains("REFERRAL_CREDIT_PERCENT") {
                rate_readers += 1;
                if line.contains('*') || line.contains(" / ") {
                    offences.push(format!("{path}:{}: {}", n + 1, line.trim()));
                }
            }
        }
    }
    let page = code_of(&source(PAGE));
    let admin = code_of(&source(ADMIN));
    let panel = admin_panel_region(&admin);
    for (name, text) in [(PAGE, page.as_str()), (ADMIN, panel)] {
        for (n, line) in text.lines().enumerate() {
            // Spellings of a tenth: `* 10 / 100`, `/ 10`, `* 0.1`. (`0.15` in
            // a colour is not one, so the bare `0.1` is not a needle.)
            for arithmetic in ["/ 100", "* 10", "/ 10", "* 0."] {
                if line.contains(arithmetic) {
                    offences.push(format!("{name} (+{}): {}", n + 1, line.trim()));
                }
            }
        }
    }
    assert!(
        offences.is_empty(),
        "the credit is computed outside trios::referral_credit:\n  {}",
        offences.join("\n  ")
    );
    // The preview prints the rate from the one constant, and the credit and
    // the redemption come from the shared functions.
    assert!(
        rate_readers >= 1,
        "the admin preview no longer names the rate"
    );
    assert!(panel.contains("credit_for_rental(rental, applied)"));
    assert!(panel.contains("applied_redemption(r.amount_thb, rental, r.balance_thb)"));
}

/// The profile's referral card prints the rule, not an amount per friend: the
/// config field it read is gone, and so is the key. The bonus history's
/// `"referral_bonus"` label arm is a different thing -- it names the points
/// already credited, which stay -- and is not what this checks.
#[test]
fn the_profile_prints_the_rule_and_reads_no_amount_per_friend() {
    let code = code_of(&source(PROFILE));
    let config = body_of(&code, "struct LoyaltyConfigData");
    assert!(
        !config.contains("referral_bonus"),
        "LoyaltyConfigData reads referral_bonus again"
    );
    assert!(!code.contains(".referral_bonus"));
    assert!(!code.contains("T_PROFILE_EARN_PER_REF"));
    assert!(code.contains("t(lang, crate::trios::i18n::T_REFERRAL_SUBTITLE)"));
    assert!(code.contains("\"{referral_rule}\""));
}

/// The rule sentence is the operator's wording and its rate is the shared
/// constant's (the value itself is asserted by the i18n unit test, which the
/// host compiles).
#[test]
fn the_rule_sentence_is_the_operators_in_both_tables() {
    let i18n = source(I18N);
    for arm in [
        "T_REFERRAL_SUBTITLE => \"10% с каждой аренды приглашённого друга\",",
        "T_REFERRAL_SUBTITLE => \"10% of every rental your invited friend completes\",",
        "T_REFERRAL_BALANCE => \"Реферальный баланс\",",
        "T_REFERRAL_BALANCE => \"Referral balance\",",
        "T_REFERRAL_APPLY_TO_RENTAL => \"Списать в счёт аренды\",",
        "T_REFERRAL_APPLY_TO_RENTAL => \"Apply to this rental\",",
        "T_REFERRAL_REQUEST_PAYOUT => \"Запросить выплату\",",
        "T_REFERRAL_REQUEST_PAYOUT => \"Request a payout\",",
        "T_REFERRAL_PAYOUT_REQUESTED => \"Запрос на выплату отправлен менеджеру\",",
        "T_REFERRAL_PAYOUT_REQUESTED => \"Payout request sent to the manager\",",
    ] {
        assert_eq!(i18n.matches(arm).count(), 1, "{arm}");
    }
    let core = source("src/trios/referral_credit.rs");
    assert!(core.contains("pub const REFERRAL_CREDIT_PERCENT: i64 = 10;"));
}

/// The admin view is the Loyalty tab's third sub-tab, and its panel reaches
/// the four admin routes of the spec's shared API through `AdminAuth` (init
/// data, the admin token, the admin's id) and nothing else.
#[test]
fn the_admin_panel_calls_the_four_admin_paths_through_admin_auth() {
    let admin = code_of(&source(ADMIN));
    let tab = body_of(&admin, "fn LoyaltyTab(");
    for needle in [
        "active_sub.set(\"referrals\".into())",
        "\"🤝 Рефералы\"",
        "ReferralCreditPanel {}",
    ] {
        assert!(tab.contains(needle), "LoyaltyTab lost `{needle}`");
    }

    let panel = admin_panel_region(&admin);
    for path in [
        "\"{}/api/admin/referral-credit/overview\"",
        "\"{}/api/admin/referral-credit/rentals\"",
        "\"{}/api/admin/referral-credit/rentals/{}/reverse\"",
        "\"{}/api/admin/referral-credit/requests/{}/resolve\"",
    ] {
        assert_eq!(panel.matches(path).count(), 1, "the panel's call to {path}");
    }
    assert_eq!(
        panel.matches("crate::ui::api::http::AdminAuth {").count(),
        2,
        "the overview read and the one POST helper each build the admin auth"
    );
    assert!(panel.contains("crate::ui::api::http::fetch_text_admin(&url, &auth)"));
    assert!(panel.contains("crate::ui::api::http::post_json_admin_full(&url, &auth, &body)"));
    assert_eq!(
        panel
            .matches("referral_admin_post(url, json, telegram_id, init)")
            .count(),
        2,
        "the record and the confirmation both post through the helper"
    );
    for other in ["HTTP_CLIENT", ".header(", "post_json_authed"] {
        assert!(
            !panel.contains(other),
            "the panel reaches the server some other way: {other}"
        );
    }
    // Appended at the end, so no line cited above it moved.
    let helper = admin
        .find("async fn admin_add_failure_reason(")
        .expect("the Add helper is in admin_screen.rs");
    assert!(admin.find("fn ReferralCreditPanel(").expect("the panel") > helper);
}

/// The recording form: the key minted when the form opens and reused by every
/// retry, the shared body type, and the operator-facing amount label saying
/// what the amount excludes. «Выплачено» is offered for a payout only.
#[test]
fn the_admin_form_records_through_the_shared_body_with_a_minted_key() {
    let admin = code_of(&source(ADMIN));
    let panel = admin_panel_region(&admin);
    for needle in [
        "idempotency_key: uuid::Uuid::new_v4().to_string()",
        "idempotency_key: current.idempotency_key.clone()",
        "crate::trios::referral_credit::RecordRentalBody {",
        "crate::trios::referral_credit::ResolveBody {",
        "crate::trios::referral_credit::ReverseBody { note }",
        "\"Сумма аренды без депозита и доставки, ฿\"",
        "action: \"paid\"",
        "action: \"declined\"",
    ] {
        assert!(panel.contains(needle), "the referral panel lost `{needle}`");
    }
    // The key is minted in exactly one place: when the form opens.
    assert_eq!(panel.matches("uuid::Uuid::new_v4()").count(), 1);
    // «Выплачено» sits in the payout branch only (the button is the label's
    // last occurrence; the confirmation's own button text comes first).
    let paid = panel.rfind("\"Выплачено\"").expect("the paid button");
    let branch = panel[..paid]
        .rfind("if is_payout {")
        .expect("the paid button is inside the payout branch");
    assert!(!panel[branch..paid].contains("} else {"));
}
