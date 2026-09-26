//! The referral credit of 2026-09-26 (R3), held to its shape by reading the
//! source as text. Default run: no database, no network.
//!
//! WHY IT EXISTS. The owner decided, verbatim: «Должно начисляться исключительно
//! за то кто арендовал 10% скидка», «Пригласивший и может забрать скидкой за
//! аренду или деньгами», and of the referral bonus points «Убрать, только
//! скидка 10%». So referral money is now a THB ledger of its own, born only
//! when a manager records a completed rental, and the old credits -- the fixed
//! referral bonus, the milestone ladder and the welcome credit -- are stopped.
//! Each rule below is one a later edit could quietly undo while every unit
//! test stayed green: a credit folded back into order completion, a points
//! write in the credit module, a referral field on the order body, a gate
//! moved after the database read, a reject that flips first and reverses
//! later. `specs/turbobaby/referral_credit.t27` records the rules; gate 3
//! binds the counts.

#![allow(clippy::panic, clippy::expect_used)]

use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// A tracked file, LF-normalised (a Windows checkout hands a matcher CRLF).
fn source(rel: &str) -> String {
    std::fs::read_to_string(root().join(rel))
        .unwrap_or_else(|e| panic!("{rel} must exist: {e}"))
        .replace("\r\n", "\n")
}

/// The text with every `//` comment cut, respecting string literals, so prose
/// that names what was removed never reads as code.
fn code_of(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        let bytes = line.as_bytes();
        let mut in_string = false;
        let mut cut = line.len();
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'\\' if in_string => i += 1,
                b'"' => in_string = !in_string,
                b'/' if !in_string && i + 1 < bytes.len() && bytes[i + 1] == b'/' => {
                    cut = i;
                    break;
                }
                _ => {}
            }
            i += 1;
        }
        out.push_str(&line[..cut]);
        out.push('\n');
    }
    out
}

/// The part of a source above its first test module.
fn shipped(text: &str) -> &str {
    match text.find("#[cfg(test)]") {
        Some(at) => &text[..at],
        None => text,
    }
}

/// The body of the item whose header starts with `header`: from its first
/// `{` to the matching `}`. Panics when the header is gone.
fn body_of<'a>(text: &'a str, header: &str) -> &'a str {
    let start = text
        .find(header)
        .unwrap_or_else(|| panic!("`{header}` is gone"));
    let open = start + text[start..].find('{').expect("an opening brace");
    let mut depth = 0usize;
    for (i, c) in text[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &text[open..open + i + 1];
                }
            }
            _ => {}
        }
    }
    panic!("`{header}` never closes")
}

/// Every `.rs` file under `src/`, repo-relative and LF-normalised.
fn rust_sources() -> Vec<(String, String)> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("readable").flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                out.push(path);
            }
        }
    }
    let mut paths = Vec::new();
    walk(&root().join("src"), &mut paths);
    paths.sort();
    assert!(
        paths.len() > 100,
        "the walk found only {} files",
        paths.len()
    );
    paths
        .into_iter()
        .map(|p| {
            let rel = p
                .strip_prefix(root())
                .expect("under the root")
                .to_string_lossy()
                .replace('\\', "/");
            let text = std::fs::read_to_string(&p)
                .expect("readable")
                .replace("\r\n", "\n");
            (rel, text)
        })
        .collect()
}

const ORDERS_DB: &str = "src/db/orders.rs";
const ORDERS_API: &str = "src/api/orders.rs";
const CREDIT_DB: &str = "src/db/referral_credit.rs";
const CREDIT_API: &str = "src/api/referral_credit.rs";
const LOYALTY_API: &str = "src/api/loyalty.rs";
const QUEST_API: &str = "src/api/quest.rs";
const API_MOD: &str = "src/api/mod.rs";
const CALLBACKS: &str = "src/bot/callbacks.rs";
const MIGRATION: &str = "migrations/089_referral_credit.sql";

#[test]
fn completion_credits_no_referral_money() {
    let orders = code_of(&source(ORDERS_DB));
    let body = body_of(&orders, "pub async fn complete_order_and_update_loyalty(");
    for gone in [
        "referral_bonus",
        "confirm_referral(",
        "maybe_award_referral_milestones",
        "enqueue_friend_ordered",
        "referred_welcome_bonus",
    ] {
        assert!(
            !body.contains(gone),
            "{ORDERS_DB}: order completion names `{gone}` again -- R3 stopped every referral credit there"
        );
    }
    assert!(
        body.contains("crate::db::referrals::confirm_referral_edge(orm, cid)"),
        "{ORDERS_DB}: the first completed order no longer confirms the referral edge"
    );
    let signature = &orders[orders
        .find("pub async fn complete_order_and_update_loyalty(")
        .expect("the function")..];
    let signature = &signature[..signature.find('{').expect("a body")];
    assert!(
        !signature.contains("f64"),
        "the welcome amount came back as a parameter: {signature}"
    );
}

#[test]
fn no_writer_composes_referral_points_or_milestones() {
    let mut writers = Vec::new();
    for (rel, text) in rust_sources() {
        let code = code_of(shipped(&text));
        for needle in [
            "tx_type: Set(\"referral_",
            "Set(\"referral_welcome\"",
            "fn maybe_award_referral_milestones(",
            "fn enqueue_friend_ordered(",
            "fn enqueue_milestone(",
            "fn confirm_referral(",
            "fn get_referrer_of(",
            "\"friend_ordered\"",
        ] {
            if code.contains(needle) {
                writers.push(format!("{rel}: {needle}"));
            }
        }
        if code
            .to_ascii_uppercase()
            .contains("INSERT INTO REFERRAL_MILESTONES")
        {
            writers.push(format!("{rel}: INSERT INTO referral_milestones"));
        }
    }
    assert!(
        writers.is_empty(),
        "a stopped referral credit has a writer again (owner, R3):\n  {}",
        writers.join("\n  ")
    );
    // The witnesses: the readers of the history stay.
    let referrals = source("src/db/referrals.rs");
    assert!(referrals.contains("pub(crate) async fn get_referral_milestones("));
    assert!(referrals.contains("pub(crate) async fn confirm_referral_edge_in<"));
}

#[test]
fn the_order_money_path_never_names_the_credit() {
    let orders = source(ORDERS_API);
    for token in [
        "referral_credit",
        "referral_rentals",
        "referral_requests",
        "referral_ledger",
    ] {
        assert!(
            !orders.contains(token),
            "{ORDERS_API} names `{token}`: the credit never enters the order-money path"
        );
    }
    assert!(
        orders.contains("fn promptpay_qr_amount("),
        "the witness moved"
    );
    let request = body_of(&orders, "pub(crate) struct CreateOrderRequest");
    assert!(
        !request.contains("referral"),
        "CreateOrderRequest carries a referral field: {request}"
    );
}

/// The six handlers, and the gates each calls before the database.
#[test]
fn every_credit_route_calls_its_gate_before_the_database() {
    let api = code_of(&source(CREDIT_API));
    let routes = body_of(&api, "pub(crate) fn routes(");
    let registered = routes.matches(".route(").count();
    assert_eq!(registered, 6, "{CREDIT_API} registers {registered} routes");

    for handler in ["async fn get_my_credit(", "async fn post_my_request("] {
        let body = body_of(&api, handler);
        let at = |needle: &str| {
            body.find(needle)
                .unwrap_or_else(|| panic!("{handler} lost `{needle}`"))
        };
        let (validate, owner, blocked, db) = (
            at("validate_telegram_id_param(telegram_id)"),
            at("check_owner(&headers, &state, telegram_id)"),
            at("check_not_blocked(&state, telegram_id)"),
            at("state.db"),
        );
        assert!(
            validate < owner && owner < blocked && blocked < db,
            "{handler}: the gates must run in order and before the database"
        );
        assert!(
            !body.contains("check_admin("),
            "{handler} is a customer route"
        );
    }
    for handler in [
        "async fn get_overview(",
        "async fn post_rental(",
        "async fn post_reversal(",
        "async fn post_resolution(",
    ] {
        let body = body_of(&api, handler);
        let gate = body
            .find("check_admin(&headers, &state)")
            .unwrap_or_else(|| panic!("{handler} has no admin gate"));
        let db = body
            .find("state.db")
            .expect("the handler reads the database");
        assert!(gate < db, "{handler} reads the database before its gate");
        assert!(
            !body.contains("check_owner("),
            "{handler} is an admin route"
        );
    }
    assert!(
        source(API_MOD).contains(".merge(referral_credit::routes())"),
        "the credit routes are not merged into /api"
    );
}

#[test]
fn the_credit_module_never_touches_points_and_loyalty_never_touches_credit() {
    let credit = source(CREDIT_DB);
    for token in ["bonus_balance", "BonusBalance", "bonus_transactions"] {
        assert!(
            !credit.contains(token),
            "{CREDIT_DB} names `{token}`: loyalty points never enter the referral ledger"
        );
    }
    assert!(
        credit.contains("INSERT INTO referral_ledger"),
        "the witness moved"
    );
    let loyalty = source(LOYALTY_API);
    for token in ["referral_credit", "referral_ledger"] {
        assert!(
            !loyalty.contains(token),
            "{LOYALTY_API} names `{token}`: the points routes never read the referral ledger"
        );
    }
    assert!(loyalty.contains("async fn use_bonus("), "the witness moved");
}

#[test]
fn the_credit_migration_only_creates_and_is_the_last_entry() {
    let sql = source(MIGRATION);
    let statements: Vec<&str> = sql
        .lines()
        .filter(|l| !l.trim_start().starts_with("--"))
        .collect();
    for line in &statements {
        for verb in ["UPDATE", "DELETE", "DROP", "TRUNCATE", "ALTER", "INSERT"] {
            assert!(
                !line
                    .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                    .any(|w| w == verb),
                "{MIGRATION} is additive only, and this line is not: {line}"
            );
        }
    }
    let tables: Vec<&str> = statements
        .iter()
        .filter_map(|l| l.strip_prefix("CREATE TABLE IF NOT EXISTS "))
        .map(|rest| rest.split_whitespace().next().expect("a name"))
        .collect();
    assert_eq!(
        tables,
        ["referral_rentals", "referral_requests", "referral_ledger"]
    );

    let db_mod = source("src/db/mod.rs");
    let list = &db_mod[db_mod.find("const MIGRATIONS").expect("the list")..];
    let list = &list[..list.find("];").expect("the list closes")];
    let last = list
        .rfind("\"0")
        .map(|at| &list[at + 1..at + 1 + "089_referral_credit.sql".len()])
        .expect("an entry");
    assert_eq!(last, "089_referral_credit.sql", "089 is not the last entry");

    let mut files: Vec<String> = std::fs::read_dir(root().join("migrations"))
        .expect("migrations/")
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.ends_with(".sql"))
        .collect();
    files.sort();
    assert_eq!(
        files.last().map(String::as_str),
        Some("089_referral_credit.sql")
    );
}

#[test]
fn the_bot_reject_reverses_before_the_flip_and_fails_closed() {
    let callbacks = code_of(&source(CALLBACKS));
    let arm = &callbacks[callbacks
        .find("CallbackAction::RejectOrder(order_id) =>")
        .expect("the reject arm")..];
    let reverse = arm
        .find("crate::db::referral_credit::reverse_rental_for_order(")
        .expect("the reject no longer reverses a referral record");
    let flip = arm
        .find("\"UPDATE orders SET status = 'rejected' WHERE id = $1\"")
        .expect("the reject's flip");
    assert!(
        reverse < flip,
        "the reject flips the order before reversing"
    );
    let tail = &arm[reverse..flip];
    assert!(
        tail.contains("&tx,"),
        "the reversal runs outside the reject's transaction"
    );
    assert!(
        arm[..reverse].trim_end().ends_with("if let Err(e) =")
            && tail.contains("refund_ok = false;"),
        "a failed reversal does not roll the reject back: {tail}"
    );
    assert_eq!(
        callbacks.matches("reverse_rental_for_order(").count(),
        1,
        "{CALLBACKS} reverses from more than one place"
    );
    // The completion arm no longer announces a referral bonus.
    let complete = &callbacks[callbacks
        .find("CallbackAction::CompleteOrder(order_id) =>")
        .expect("the completion arm")
        ..callbacks
            .find("CallbackAction::RejectOrder(order_id) =>")
            .expect("the reject arm")];
    assert!(!complete.contains("referral_bonus_credited"));
    assert!(!complete.contains("locale.referral_bonus"));
}

#[test]
fn r1_the_loyalty_leaderboard_is_admin_only() {
    let loyalty = code_of(&source(LOYALTY_API));
    let body = body_of(&loyalty, "async fn get_leaderboard(");
    assert!(
        body[1..]
            .trim_start()
            .starts_with("check_admin(&headers, &state)?;"),
        "get_leaderboard must start with the admin gate (owner, R1: «Только для админа»): {body}"
    );
    assert!(
        loyalty.contains(".route(\"/loyalty/leaderboard\", get(get_leaderboard))"),
        "the leaderboard route moved"
    );
}

#[test]
fn r2_closed_reads_call_admin_or_missing_route_first_and_the_fallback_delegates() {
    const FIRST: &str =
        "if let Err(miss) = crate::api::admin_or_missing_route(&headers, &state, &uri) {";
    for (file, header, route) in [
        (
            QUEST_API,
            "async fn get_quest_places(",
            ".route(\"/quest-places\", get(get_quest_places))",
        ),
        (
            QUEST_API,
            "async fn get_treasure_hunts(",
            ".route(\"/treasure-hunts\", get(get_treasure_hunts))",
        ),
        (
            LOYALTY_API,
            "async fn get_loyalty_config(",
            ".route(\"/loyalty/config\", get(get_loyalty_config))",
        ),
    ] {
        let code = code_of(&source(file));
        let body = body_of(&code, header);
        assert!(
            body[1..].trim_start().starts_with(FIRST),
            "{file} {header} must ask admin_or_missing_route first (owner, R2: «Закрыть для клиентов»)"
        );
        assert!(
            code.contains(route),
            "{file}: {route} is unregistered -- a closed read stays registered, or a customer gets 405"
        );
    }
    let api = code_of(&source(API_MOD));
    let fallback = body_of(&api, "async fn api_not_found(");
    assert!(
        fallback.contains("missing_route(uri.path())"),
        "api_not_found no longer builds its answer with missing_route: {fallback}"
    );
    let helper = body_of(&api, "pub(crate) fn admin_or_missing_route(");
    assert!(helper.contains("check_admin(headers, state)"));
    assert!(helper.contains("missing_route(uri.path())"));
    assert!(api.contains("router.fallback(api_not_found)"));
}

#[test]
fn the_credit_moves_no_money() {
    for file in [CREDIT_DB, CREDIT_API] {
        let text = source(file).to_ascii_lowercase();
        for token in ["promptpay", "build_payload", "invoice", "send_invoice"] {
            assert!(
                !text.contains(token),
                "{file} names `{token}`: nothing in the referral credit moves money"
            );
        }
    }
}

#[test]
fn credit_money_reads_fail_loud() {
    let credit = source(CREDIT_DB);
    for token in [
        "unwrap_or(0)",
        "unwrap_or(0.0)",
        "unwrap_or_default()",
        "try_get_warn!",
    ] {
        assert!(
            !credit.contains(token),
            "{CREDIT_DB} reads a money figure with a silent default (`{token}`)"
        );
    }
    assert!(credit.contains("SUM(amount_thb)"), "the balance sum moved");
    let main = source("src/main.rs");
    for name in ["record_rental", "resolve_request", "credit_summary"] {
        assert!(
            main.contains(&format!("(\"src/db/referral_credit.rs\", \"{name}\")")),
            "SENSITIVE_FNS no longer audits {name}"
        );
    }
}
