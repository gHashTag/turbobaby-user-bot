//! What a customer is told about the status of an order that is theirs, guarded
//! as text where the host cannot compile it.
//!
//! `src/lib.rs` gates `pub mod ui;` on `#[cfg(target_arch = "wasm32")]`, so
//! `cargo test` compiles nothing under `src/ui`. Reading the text is the only
//! instrument that reaches the two order screens and the stepper they share, so
//! that is the instrument this file uses, and it says so rather than pretending
//! to be something stronger. The DECISIONS -- which arm a status string falls
//! in, its label key, its colour, its chip, whether it is terminal, whether a
//! cancel or a reorder is offered, and how much of the progress bar it fills --
//! live in `src/trios/order_status_view.rs`, which the host compiles and tests.
//! What is guarded here is that the three files a customer sees are wired to
//! that one reading and decide nothing about the string on their own.
//!
//! Contract: `specs/turbobaby/order_presentation.t27` (turbobaby/order-presentation).
//! The vocabulary and the display pipeline's CONTENT belong to
//! `specs/turbobaby/order_status.t27` (turbobaby/order-status); nothing here
//! lists the nine names -- they are read out of the server's own whitelist.
//!
//! WHY IT EXISTS (measured 2026-09-21 and still true at 14b01ac): fifteen sites
//! across the three files read one status string, six of them lower-casing it and
//! nine comparing it raw; the stepper drew a status it could not read as a
//! finished delivery; and the server's own word was printed as a step label for
//! every cancelled, rejected or unrecognised order.
//!
//! This file does NOT import the module on purpose. It compiles against any
//! tree, so on a tree where the screens still decide for themselves it fails by
//! CONTENT, which is the failure a reviewer needs to see.

use std::fs;
use std::path::{Path, PathBuf};

const LIST_PATH: &str = "src/ui/screens/orders_screen.rs";
const DETAIL_PATH: &str = "src/ui/screens/order_detail_screen.rs";
const STEPPER_PATH: &str = "src/ui/components/status_stepper.rs";
const READING_PATH: &str = "src/trios/order_status_view.rs";
const SERVER_PATH: &str = "src/api/orders.rs";
const CONTRACT: &str = "specs/turbobaby/order_presentation.t27";
const OWNER_CONTRACT: &str = "specs/turbobaby/order_status.t27";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// A tracked file, normalised to LF. A Windows checkout (`core.autocrlf`) hands
/// the reader CRLF, and every anchor below that ends in a newline would then
/// match nothing -- a guard that quietly stops matching is the failure these
/// guards exist to prevent. A missing file is a failure, not an empty string.
fn source(relative: &str) -> String {
    fs::read_to_string(repo_root().join(relative))
        .unwrap_or_else(|e| panic!("{relative} must exist for this guard to mean anything: {e}"))
        .replace("\r\n", "\n")
}

/// Code lines only. Comments in these files explain removed defects on purpose,
/// and prose about a defect is not the defect.
fn code_lines(body: &str) -> Vec<&str> {
    body.lines()
        .filter(|l| {
            let t = l.trim_start();
            !(t.starts_with("//") || t.starts_with("/*") || t.starts_with('*'))
        })
        .collect()
}

/// The quoted names between `const VALID_STATUSES: &[&str] = &[` and `];` in the
/// server's validator -- the only list that decides what the column may hold.
/// Read, never written here: a sixth hand-written enumeration of the column is
/// exactly what the order-status contract counts against this repository.
fn vocabulary() -> Vec<String> {
    let server = source(SERVER_PATH);
    let open = "const VALID_STATUSES: &[&str] = &[";
    let start = server
        .find(open)
        .unwrap_or_else(|| panic!("{SERVER_PATH}: `{open}` is gone -- this guard reads nothing"));
    let rest = &server[start + open.len()..];
    let end = rest
        .find("];")
        .unwrap_or_else(|| panic!("{SERVER_PATH}: VALID_STATUSES never closes"));
    let names: Vec<String> = rest[..end]
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let open = line.find('"')?;
            let close = open + 1 + line[open + 1..].find('"')?;
            Some(line[open + 1..close].to_string())
        })
        .collect();
    // D16: a scan whose input came back empty passes every check below. The floor
    // is not the count: the count is the order-status contract's STATUS_COUNT and
    // gate 3 pins it; this only refuses to run blind.
    assert!(
        names.len() >= 9,
        "read only {} names out of VALID_STATUSES -- the reader is broken, not the code",
        names.len()
    );
    names
}

/// How many times any accepted status name appears as a double-quoted literal in
/// `text`, comments included. Comments count on purpose: gate 3 reads the raw text
/// with the same rule, and the two must not disagree about what a file holds.
fn quoted_status_names(text: &str, names: &[String]) -> usize {
    names
        .iter()
        .map(|n| text.matches(&format!("\"{n}\"")).count())
        .sum()
}

/// Every value of one contract constant: the single string of a `str`, or every
/// string of a `[N]str`, whether the array sits on one line or spans several.
fn spec_values(spec_path: &str, name: &str) -> Vec<String> {
    let spec = source(spec_path);
    let head = format!("pub const {name} : ");
    let mut lines = spec.lines().skip_while(|l| !l.starts_with(&head));
    let first = lines
        .next()
        .unwrap_or_else(|| panic!("`{name}` is no longer declared in {spec_path}"));
    let mut text = first.to_string();
    if first.contains("= [") && !first.trim_end().ends_with("];") {
        for line in lines.by_ref() {
            text.push('\n');
            text.push_str(line);
            if line.trim_start().starts_with("];") {
                break;
            }
        }
    }
    let body = &text[text.find('=').unwrap_or(0)..];
    let values: Vec<String> = body
        .split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_string)
        .collect();
    assert!(
        !values.is_empty(),
        "`{name}` in {spec_path} holds no string"
    );
    values
}

/// `path:N` or `path:N-M` -> (path, first, last), all 1-based.
fn parse_site(site: &str) -> (String, usize, usize) {
    let (path, range) = site
        .rsplit_once(':')
        .unwrap_or_else(|| panic!("`{site}` is not a path:line citation"));
    let (first, last) = match range.split_once('-') {
        Some((a, b)) => (a, b),
        None => (range, range),
    };
    let first: usize = first
        .parse()
        .unwrap_or_else(|_| panic!("`{site}` has no line number"));
    let last: usize = last
        .parse()
        .unwrap_or_else(|_| panic!("`{site}` has no end line"));
    assert!(
        first >= 1 && last >= first,
        "`{site}` is not a forward range"
    );
    (path.to_string(), first, last)
}

/// The cited lines of one citation, as text.
fn cited_lines(site: &str) -> Vec<String> {
    let (path, first, last) = parse_site(site);
    let text = source(&path);
    let lines: Vec<&str> = text.lines().collect();
    assert!(
        last <= lines.len(),
        "`{site}` cites past the end of {path} ({} lines)",
        lines.len()
    );
    lines[first - 1..last]
        .iter()
        .map(|l| l.to_string())
        .collect()
}

#[test]
fn the_guard_reads_the_files_it_claims_to_read() {
    for (label, floor) in [
        (LIST_PATH, 300),
        (DETAIL_PATH, 400),
        (STEPPER_PATH, 40),
        (SERVER_PATH, 3000),
    ] {
        let lines = source(label).lines().count();
        assert!(
            lines >= floor,
            "{label} read only {lines} lines (floor {floor}) -- a shrunken corpus passes \
             every check below by default"
        );
    }
    assert!(source(LIST_PATH).contains("pub fn OrdersScreen("));
    assert!(source(DETAIL_PATH).contains("fn OrderDetailCard("));
    assert!(source(STEPPER_PATH).contains("pub fn StatusStepper("));

    // Calibration: the counter sees a literal in code AND in a comment, and the
    // line filter drops the comment.
    let names = vocabulary();
    let sample = "let a = \"pending\";\n    // was \"ready\"\n";
    assert_eq!(quoted_status_names(sample, &names), 2);
    assert_eq!(code_lines(sample).len(), 1);
    assert_eq!(quoted_status_names("let a = \"pendingx\";", &names), 0);
}

/// Defect 4 (two readings of case over one string) and the helper copies behind it.
/// Neither screen may read the status string itself: no quoted status name, no
/// case folding, no decider of its own -- and each must call the one reading, the
/// positive witness that keeps these zeros from being vacuous (D16).
#[test]
fn no_owned_screen_reads_the_status_string_itself() {
    let names = vocabulary();
    let mut offences = Vec::new();
    for path in [LIST_PATH, DETAIL_PATH] {
        let text = source(path);
        let quoted = quoted_status_names(&text, &names);
        if quoted != 0 {
            offences.push(format!("{path}: {quoted} quoted status name(s)"));
        }
        for line in code_lines(&text) {
            for needle in [
                "to_lowercase(",
                "fn status_color",
                "fn status_label_key",
                "fn is_terminal_status",
            ] {
                if line.contains(needle) {
                    offences.push(format!("{path}: `{}`", line.trim()));
                }
            }
        }
        if !code_lines(&text).iter().any(|l| l.contains("arm_of(")) {
            offences.push(format!("{path}: never calls arm_of("));
        }
    }
    assert!(
        offences.is_empty(),
        "the order screens still decide the status for themselves:\n  {}",
        offences.join("\n  ")
    );
}

/// Defects 1 and 5. The stepper may only draw: no pipeline of its own, no index
/// fallback, no fill rule of its own, no server token as a label, and a flat bar
/// painted in the arm's colour rather than a hard-coded red.
#[test]
fn the_stepper_draws_nothing_it_cannot_read() {
    let names = vocabulary();
    let text = source(STEPPER_PATH);
    let code = code_lines(&text);
    let mut offences = Vec::new();

    let quoted = quoted_status_names(&text, &names);
    if quoted != 0 {
        offences.push(format!("{quoted} quoted status name(s)"));
    }
    for needle in [
        "const ORDER_PIPELINE",
        "unwrap_or(ORDER_PIPELINE.len())",
        "i <= current",
        "status.to_string()",
        "to_lowercase(",
        "match status",
        "status ==",
        "#ff4757",
    ] {
        for line in &code {
            if line.contains(needle) {
                offences.push(format!("`{}` (holds `{needle}`)", line.trim()));
            }
        }
    }
    for needed in [
        "arm_of(",
        "progress_of(&status, ORDER_PIPELINE)",
        "draws_one_flat_bar(",
        "segment_is_filled(",
    ] {
        if !code.iter().any(|l| l.contains(needed)) {
            offences.push(format!("never calls `{needed}`"));
        }
    }

    // The flat branch: painted in the arm's colour, and no segment is drawn inside it.
    let joined = code.join("\n");
    match joined.find("if flat {") {
        None => offences.push("no `if flat {` branch".to_string()),
        Some(at) => {
            let branch = &joined[at..];
            let branch = &branch[..branch.find("} else {").unwrap_or(branch.len())];
            if !branch.contains("background:{color}") {
                offences.push("the flat bar is not painted in the arm's colour".to_string());
            }
            if branch.contains("PipelineSegment {") {
                offences.push("the flat branch draws pipeline segments".to_string());
            }
        }
    }

    assert!(
        offences.is_empty(),
        "{STEPPER_PATH} still decides what a status means:\n  {}",
        offences.join("\n  ")
    );
}

/// The pipeline the stepper walks must be declared where the host can test its
/// content, and the owner of that content must cite the place it lives.
/// turbobaby/order-status owns WHICH names are in it; the host tests in
/// src/trios/order_status_view.rs pin it to that contract's declarations.
#[test]
fn the_pipeline_the_stepper_draws_is_declared_where_the_host_can_test_it() {
    let stepper = source(STEPPER_PATH);
    assert!(
        !code_lines(&stepper)
            .iter()
            .any(|l| l.contains("const ORDER_PIPELINE")),
        "{STEPPER_PATH} declares its own ORDER_PIPELINE, which no host test can reach \
         (src/lib.rs gates src/ui on wasm32)"
    );
    assert!(
        stepper.contains("crate::trios::order_status_view::"),
        "{STEPPER_PATH} does not take its decisions from {READING_PATH}"
    );

    let cited = spec_values(OWNER_CONTRACT, "DISPLAY_PIPELINE_MODEL_A");
    assert_eq!(cited.len(), 1);
    let (path, _, _) = parse_site(&cited[0]);
    assert_eq!(
        path, READING_PATH,
        "order-status still cites the pipeline at {}, not where it is declared",
        cited[0]
    );
    let lines = cited_lines(&cited[0]);
    assert!(
        lines[0].contains("pub const ORDER_PIPELINE: &[&str] = &["),
        "DISPLAY_PIPELINE_MODEL_A = {} does not open on the pipeline: `{}`",
        cited[0],
        lines[0]
    );
    assert_eq!(
        lines[lines.len() - 1].trim(),
        "];",
        "DISPLAY_PIPELINE_MODEL_A = {} does not close on the pipeline",
        cited[0]
    );
}

/// Every site the contract cites in the four files it owns lands on the code it
/// names. A line citation is otherwise a claim nobody re-reads: two more families
/// edit these same files, and each shifts the lines under the words.
/// Constants holding the record from before the 2026-09-22 repair
/// (`*_BEFORE_REPAIR`) are history at commit 14b01ac and are never read here.
/// Where the pipeline sits is the order-status contract's citation, and the
/// pipeline test above reads it there.
#[test]
fn the_contracts_sites_land_on_their_code() {
    // (constant, the text every cited line -- or, for a range, some cited line -- holds)
    let single: [(&str, &[&str]); 31] = [
        (
            "READING_SITE",
            &["pub fn arm_of(status: &str) -> StatusArm {"],
        ),
        (
            "ALIAS_DECISION_SITE",
            &["\"delivered\" | \"completed\" => StatusArm::DeliveredOrCompleted,"],
        ),
        (
            "REJECTED_FOLD_SITE",
            &["\"cancelled\" | \"rejected\" => StatusArm::CancelledOrRejected,"],
        ),
        ("LABEL_KEY_SITE", &["pub fn label_key(self) -> Key {"]),
        ("CHIP_RULE_SITE", &["pub fn chip(self) -> StatusChip {"]),
        (
            "TERMINALITY_COPY_SITE",
            &["pub fn is_terminal(self) -> bool {"],
        ),
        (
            "CANCEL_PRECONDITION_COPY_SITE",
            &["pub fn customer_may_cancel(self) -> bool {"],
        ),
        (
            "UNREADABLE_PROGRESS_SITE",
            &["StatusArm::Unresolved => Progress::Unreadable,"],
        ),
        (
            "SEGMENT_FILL_RULE_SITE",
            &["Progress::AtStep(step) => index <= step,"],
        ),
        ("FLAT_BAR_SITE", &["background:{color}"]),
        ("CHIP_SITE", &["fn matches(&self, status: &str) -> bool {"]),
        (
            "CANCEL_BUTTON_CONDITION_SITE",
            &["let is_pending = arm.customer_may_cancel();"],
        ),
        (
            "POLLED_STATUS_OVERRIDES_THE_LOADED_ONE_AT",
            &["let display_status"],
        ),
        (
            "DASH_FILTERED_SITE",
            &["fn line_total(item: &ApiOrderItem) -> String {"],
        ),
        ("DEAD_CODE_ANNOTATION_SITE", &["#[allow(dead_code)]"]),
        ("LABELLED_DELIVERY_ROW_SITE", &["T_CART_DELIVERY"]),
        ("ORDER_DATE_SITE", &[".split('T')"]),
        ("POLL_SITE", &["for tick in 0..360u32"]),
        ("LIVE_BADGE_SITE", &["if live_status.is_some()"]),
        // The cancel repair (2026-09-22). The two defect sites it replaced are
        // history under _BEFORE_REPAIR names; these are the sites that replaced them.
        ("CANCEL_SEND_SITE", &["async fn confirm_cancel("]),
        (
            "CANCEL_ANSWER_READ_AT",
            &["let answer = order_cancel_answer(status);"],
        ),
        (
            "CANCEL_DIALOG_CLOSE_SITE",
            &["if order_cancel_dialog_closes(answer) {"],
        ),
        ("CANCEL_REREAD_REQUEST_SITE", &["on_answered.call(());"]),
        (
            "CANCEL_REREAD_HANDLER_SITE",
            &["live_status_res.restart();"],
        ),
        (
            "CANCEL_LINE_SITE",
            &["order_cancel_line(lang, cancel_progress(), since_answer)"],
        ),
        (
            "CANCEL_OFFER_GATE_SITE",
            &["if is_pending && crate::trios::api_errors::order_cancel_offer_shown("],
        ),
        (
            "CANCEL_RESEND_GATE_SITE",
            &["order_cancel_may_send(cancel_progress(), since_answer)"],
        ),
        // What the resend gate and the line are fed: the fresh reading as it
        // landed in the screen's own resource (added after a checker's mutant
        // replaced it with a constant and every other guard stayed green).
        (
            "CANCEL_REREAD_OBSERVED_SITE",
            &["live_status_res.read().as_ref().map(Option::is_some),"],
        ),
        ("FLAT_BAR_RULE_SITE", &["pub fn draws_one_flat_bar("]),
        ("LIST_CHIPS_SITE", &["for filter in [StatusFilter::All"]),
        (
            "READING_CALL_IN_THE_STEPPER_SITE",
            &["let arm = arm_of(&status);"],
        ),
    ];
    let arrays: [(&str, &[&str]); 9] = [
        (
            "REORDER_SITES",
            &["if arm.reorder_offered() {", "if is_terminal {"],
        ),
        (
            "STATUS_ARRIVES_AS_AN_UNCHECKED_STRING_AT",
            &["status: String,", "status: String,", "status: String,"],
        ),
        // The money repair (2026-09-22). The four bare-figure sites it replaced
        // are history under UNFILTERED_SITES_BEFORE_REPAIR; these replaced them.
        (
            "ORDER_FIGURE_RENDER_SITES",
            &[
                "crate::trios::pricing::order_money_text(&order.money, dash)",
                "crate::trios::pricing::order_total_text(o.total,",
            ],
        ),
        (
            "OPTIONAL_FIGURE_SITES",
            &[
                "money: crate::trios::pricing::OrderMoney,",
                "total: Option<f64>,",
            ],
        ),
        (
            "CONDITIONAL_ROW_SITES",
            &[
                "if let Some(bonus) = &money.bonus {",
                "if let Some(stars) = &money.stars {",
            ],
        ),
        (
            "IDENTITY_TERM_SITES",
            &[
                "\"{money.subtotal}\"",
                "\"{bonus}\"",
                "\"{stars}\"",
                "\"{money.total}\"",
            ],
        ),
        (
            "UNLABELLED_ROW_SITES",
            &[
                "{addr.clone()}",
                "{notes.clone()}",
                "{phone.clone()}",
                "{date_str}",
            ],
        ),
        (
            "KEYLESS_CUSTOMER_STRING_SITES",
            &[
                "\"Unknown\".to_string()",
                "unwrap_or(\"TurboBaby\")",
                "\"Error: {e}\"",
                "\"Unknown\".to_string()",
                "unwrap_or(\"TurboBaby\")",
                "\"{e}\"",
            ],
        ),
        (
            "UNRESOLVED_ANSWER_SITES",
            &[
                "StatusArm::Unresolved => \"#8b8b9e\",",
                "StatusArm::Unresolved => T_ORDERS_STATUS_UNKNOWN,",
                "StatusArm::CancelledOrRejected | StatusArm::Unresolved => None,",
                "StatusArm::Unresolved => Progress::Unreadable,",
            ],
        ),
    ];

    let mut wrong = Vec::new();
    let mut checked = 0usize;
    let mut check = |name: &str, site: &str, anchor: &str| {
        let lines = cited_lines(site);
        let (_, first, last) = parse_site(site);
        let hit = if first == last {
            lines[0].contains(anchor)
        } else {
            lines.iter().any(|l| l.contains(anchor))
        };
        if !hit {
            wrong.push(format!("{name} = {site} does not hold `{anchor}`"));
        }
        checked += 1;
    };
    for (name, anchors) in single {
        let values = spec_values(CONTRACT, name);
        assert_eq!(values.len(), 1, "{name} is a single site");
        check(name, &values[0], anchors[0]);
    }
    for (name, anchors) in arrays {
        let values = spec_values(CONTRACT, name);
        assert_eq!(
            values.len(),
            anchors.len(),
            "{name} holds {} sites, this guard expects {}",
            values.len(),
            anchors.len()
        );
        for (site, anchor) in values.iter().zip(anchors.iter()) {
            check(name, site, anchor);
        }
    }
    // D16: the table above is the input; a guard that checked nothing is not a guard.
    assert!(checked >= 50, "only {checked} sites were checked");
    assert!(
        wrong.is_empty(),
        "contract sites no longer land on their code:\n  {}",
        wrong.join("\n  ")
    );
}

/// The premise the exact reading rests on, re-taken rather than trusted: the
/// server reads the column exactly. If it ever folds case, a customer's cancel
/// button (drawn only for the exact name) would be refused less often than it is
/// drawn, and the reason for reading exactly here is gone.
#[test]
fn the_exact_reading_rests_on_the_servers_own_reading() {
    let server = source(SERVER_PATH);
    assert!(
        server.contains("if !VALID_STATUSES.contains(&status) {"),
        "the whitelist no longer compares the requested name exactly"
    );
    assert!(
        server.contains("if o.status != \"pending\" {"),
        "the customer cancellation no longer compares the stored name exactly"
    );
    let validator_at = server
        .find("pub(crate) fn validate_update_order_status(")
        .expect("the validator is gone");
    let validator = &server[validator_at..];
    let validator = &validator[..validator.find("\n}\n").expect("the validator never closes")];
    assert!(
        !validator.contains("to_lowercase") && !validator.contains("to_ascii_lowercase"),
        "the validator folds case now; the exact reading on the screens is no longer the server's"
    );
}
