//! The one reading of an order's status that a customer's order screens share.
//!
//! The orders list (`src/ui/screens/orders_screen.rs`), the order detail screen
//! (`src/ui/screens/order_detail_screen.rs`) and the stepper they both draw
//! (`src/ui/components/status_stepper.rs`) take every decision about the status
//! string from here: which arm it falls in, its label key, its colour, its step,
//! its filter chip, whether it is terminal, whether a cancel or a reorder is
//! offered, and how much of the progress bar it fills. Owned by
//! turbobaby/order-presentation (`specs/turbobaby/order_presentation.t27`).
//!
//! Until 2026-09-22 those three files decided for themselves, fifteen times over:
//! six sites lower-cased the string and nine compared it raw, so a status in
//! another case kept its label and lost its chip, its buttons and its place in
//! the pipeline; a status nobody recognised was drawn as a finished delivery; and
//! the server's own word was printed as a step label for every cancelled,
//! rejected or unrecognised order, in both locales (D13: Russian is primary).
//! The screens compile only for wasm32, so none of that could be tested on the
//! host. This module compiles on both targets and is tested below.
//!
//! THE READING IS EXACT, because the server's is. `validate_update_order_status`
//! in `src/api/orders.rs` admits a name only if `VALID_STATUSES.contains` it,
//! the terminal guard in `update_order_status` is a raw `matches!`, and a
//! customer cancellation is refused with 409 whenever `o.status != "pending"`.
//! The order-status contract's `order_status_code_of` says the same: a different
//! case yields UNSET. Folding case here would draw a cancel button the server
//! refuses.
//!
//! WHAT IS NOT OWNED HERE. The vocabulary, the terminal set, the customer-cancel
//! precondition and the CONTENT of the display pipeline belong to
//! turbobaby/order-status (`specs/turbobaby/order_status.t27`: `STATUS_NAMES`,
//! `TERMINAL_NAMES` and `order_status_is_terminal`,
//! `order_status_customer_cancel_admitted`, `DISPLAY_PIPELINE_MISSING_NAMES`).
//! Rust cannot import a contract, so the arms, `StatusArm::is_terminal`,
//! `StatusArm::customer_may_cancel` and `ORDER_PIPELINE` below are named copies,
//! and each carries a tripwire: gate 3 (`scripts/verify_t27_against_source.py`)
//! binds `STATUS_NAMES` and `TERMINAL_NAMES` to the arms of `arm_of`, and the host
//! tests at the bottom of this file read the owner's declarations and the
//! server's checks and fail when a copy here disagrees with either.
//!
//! THREE THINGS ARE THE OWNER'S TO DECIDE, and this module only keeps what
//! shipped (`docs/t27-handover.md`, section 3). `completed` stays an alias of
//! `delivered`: one arm, one label, one chip, one full bar, one reorder button.
//! `rejected` stays folded into the cancelled arm. And a status this module
//! cannot read gets one answer: the unknown label, the grey of no information,
//! one flat grey bar with nothing filled, no step label, the Active chip (so it is
//! never filtered out of sight), and neither a cancel nor a reorder button. Each
//! of the three is a one-line change here when the owner rules.
//!
//! No customer copy lives here -- labels are i18n key NAMES, never their text --
//! and no order id, name, phone, address or note (D14).

use crate::trios::i18n::{
    Key, T_ORDERS_STATUS_CANCELLED, T_ORDERS_STATUS_CONFIRMED, T_ORDERS_STATUS_DELIVERED,
    T_ORDERS_STATUS_OUT_FOR_DELIVERY, T_ORDERS_STATUS_PENDING, T_ORDERS_STATUS_PREPARING,
    T_ORDERS_STATUS_READY, T_ORDERS_STATUS_UNKNOWN, T_ORDERS_STEP_CONFIRMED,
    T_ORDERS_STEP_DELIVERED, T_ORDERS_STEP_ON_THE_WAY, T_ORDERS_STEP_PREPARING,
    T_ORDERS_STEP_READY, T_ORDERS_STEP_RECEIVED,
};

/// The steps the stepper draws, in order. The CONTENT is turbobaby/order-status's
/// (`DISPLAY_PIPELINE_MISSING_NAMES`, `DISPLAY_PIPELINE_VARIANT_COUNT`): the
/// vocabulary minus the three names it leaves out. It moved here from the stepper
/// on 2026-09-22 so a host test can check that content; the stepper, compiled
/// only for wasm32, could not be.
pub const ORDER_PIPELINE: &[&str] = &[
    "pending",
    "confirmed",
    "preparing",
    "ready",
    "out_for_delivery",
    "delivered",
];

/// The arm a status string falls in. These mirror the order-presentation
/// contract's `ARM_*` and are not a second status vocabulary: two arms are named
/// for a pair because the shipped screens fold that pair, and `Unresolved` exists
/// because a reading needs an answer for a string it cannot read, not because the
/// server has a tenth state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusArm {
    Pending,
    Confirmed,
    Preparing,
    Ready,
    OutForDelivery,
    DeliveredOrCompleted,
    CancelledOrRejected,
    Unresolved,
}

/// Which of the three partitioning filter chips an order is listed under. The
/// fourth chip, All, shows everything and is the list's, not a status's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusChip {
    Active,
    Completed,
    Cancelled,
}

/// How far along the pipeline an order is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    /// At this 0-based step of the pipeline; it and every step before it fill.
    AtStep(usize),
    /// A finished order: every step fills. Reached by its own arm, never by a
    /// fallback -- the persisted name is not in the pipeline at all.
    Finished,
    /// A cancelled or rejected order: one flat bar in the arm's colour.
    Stopped,
    /// A string this reading cannot read: one flat bar in the grey of no
    /// information, and nothing filled.
    Unreadable,
}

/// The one reading of the status string. Exact, like the server's: no case
/// folding and no trimming. One arm per line, in the shape gate 3 parses.
pub fn arm_of(status: &str) -> StatusArm {
    match status {
        "pending" => StatusArm::Pending,
        "confirmed" => StatusArm::Confirmed,
        "preparing" => StatusArm::Preparing,
        "ready" => StatusArm::Ready,
        "out_for_delivery" => StatusArm::OutForDelivery,
        "delivered" | "completed" => StatusArm::DeliveredOrCompleted,
        "cancelled" | "rejected" => StatusArm::CancelledOrRejected,
        _ => StatusArm::Unresolved,
    }
}

impl StatusArm {
    /// Every arm, the unresolved one last.
    pub const ALL: [StatusArm; 8] = [
        StatusArm::Pending,
        StatusArm::Confirmed,
        StatusArm::Preparing,
        StatusArm::Ready,
        StatusArm::OutForDelivery,
        StatusArm::DeliveredOrCompleted,
        StatusArm::CancelledOrRejected,
        StatusArm::Unresolved,
    ];

    /// The label key, in the order of the contract's `LABEL_KEY_NAMES`.
    pub fn label_key(self) -> Key {
        match self {
            StatusArm::Pending => T_ORDERS_STATUS_PENDING,
            StatusArm::Confirmed => T_ORDERS_STATUS_CONFIRMED,
            StatusArm::Preparing => T_ORDERS_STATUS_PREPARING,
            StatusArm::Ready => T_ORDERS_STATUS_READY,
            StatusArm::OutForDelivery => T_ORDERS_STATUS_OUT_FOR_DELIVERY,
            StatusArm::DeliveredOrCompleted => T_ORDERS_STATUS_DELIVERED,
            StatusArm::CancelledOrRejected => T_ORDERS_STATUS_CANCELLED,
            StatusArm::Unresolved => T_ORDERS_STATUS_UNKNOWN,
        }
    }

    /// The colour the label, the card border and a flat bar are drawn in.
    pub fn color(self) -> &'static str {
        match self {
            StatusArm::Pending => "#ffe600",
            StatusArm::Confirmed => "#00e5ff",
            StatusArm::Preparing => "#ff9d00",
            StatusArm::Ready => "#39ff14",
            StatusArm::OutForDelivery => "#00e5ff",
            StatusArm::DeliveredOrCompleted => "#39ff14",
            StatusArm::CancelledOrRejected => "#ff4757",
            StatusArm::Unresolved => "#8b8b9e",
        }
    }

    /// The step label key and its marker, or nothing. Never the server's string:
    /// an arm with no step shows no step.
    pub fn step(self) -> Option<(Key, &'static str)> {
        match self {
            StatusArm::Pending => Some((T_ORDERS_STEP_RECEIVED, "📥")),
            StatusArm::Confirmed => Some((T_ORDERS_STEP_CONFIRMED, "✅")),
            StatusArm::Preparing => Some((T_ORDERS_STEP_PREPARING, "🔥")),
            StatusArm::Ready => Some((T_ORDERS_STEP_READY, "📦")),
            StatusArm::OutForDelivery => Some((T_ORDERS_STEP_ON_THE_WAY, "🚗")),
            StatusArm::DeliveredOrCompleted => Some((T_ORDERS_STEP_DELIVERED, "🎉")),
            StatusArm::CancelledOrRejected | StatusArm::Unresolved => None,
        }
    }

    /// A named copy of turbobaby/order-status's terminal set
    /// (`TERMINAL_NAMES`, `order_status_is_terminal`), checked against it below.
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            StatusArm::DeliveredOrCompleted | StatusArm::CancelledOrRejected
        )
    }

    /// The chip an order is listed under. Derived from the arm, so the three
    /// partitioning chips cover every arm exactly once by construction.
    pub fn chip(self) -> StatusChip {
        match self {
            StatusArm::DeliveredOrCompleted => StatusChip::Completed,
            StatusArm::CancelledOrRejected => StatusChip::Cancelled,
            _ => StatusChip::Active,
        }
    }

    /// A named copy of turbobaby/order-status's
    /// `order_status_customer_cancel_admitted`, checked against it and against
    /// the server's own check below.
    pub fn customer_may_cancel(self) -> bool {
        self == StatusArm::Pending
    }

    /// The reorder button follows terminality, as it shipped: it is offered
    /// after a cancellation or a rejection too.
    pub fn reorder_offered(self) -> bool {
        self.is_terminal()
    }
}

/// How far along `pipeline` a status is drawn. The pipeline is a parameter so its
/// content stays turbobaby/order-status's; the stepper passes [`ORDER_PIPELINE`].
/// The string is read once, by [`arm_of`], and a step is found by the arm of each
/// pipeline entry rather than by a second comparison of the string.
pub fn progress_of(status: &str, pipeline: &[&str]) -> Progress {
    let arm = arm_of(status);
    match arm {
        StatusArm::Unresolved => Progress::Unreadable,
        StatusArm::CancelledOrRejected => Progress::Stopped,
        StatusArm::DeliveredOrCompleted => Progress::Finished,
        _ => pipeline
            .iter()
            .position(|&step| arm_of(step) == arm)
            .map_or(Progress::Unreadable, Progress::AtStep),
    }
}

/// Whether the segment at `index` is drawn filled.
pub fn segment_is_filled(progress: Progress, index: usize) -> bool {
    match progress {
        Progress::AtStep(step) => index <= step,
        Progress::Finished => true,
        Progress::Stopped | Progress::Unreadable => false,
    }
}

/// Whether the stepper draws one flat bar in the arm's colour instead of the
/// pipeline's segments.
pub fn draws_one_flat_bar(progress: Progress) -> bool {
    matches!(progress, Progress::Stopped | Progress::Unreadable)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The server's validator. The nine names are read out of it, never written in
    /// this file: a hand-written list would be one more enumeration of the column.
    const SERVER: &str = include_str!("../api/orders.rs");
    /// The owner of the vocabulary, the terminal set, the cancel precondition and
    /// the pipeline's content.
    const OWNER: &str = include_str!("../../specs/turbobaby/order_status.t27");

    const STEP_KEYS: [Key; 6] = [
        T_ORDERS_STEP_RECEIVED,
        T_ORDERS_STEP_CONFIRMED,
        T_ORDERS_STEP_PREPARING,
        T_ORDERS_STEP_READY,
        T_ORDERS_STEP_ON_THE_WAY,
        T_ORDERS_STEP_DELIVERED,
    ];

    /// Strings this reading must not read: two names from another model of the
    /// column (the UI types' extras), two accepted names in another case, the empty
    /// string, and an accepted name with a trailing space.
    const UNREADABLE: [&str; 6] = [
        "shipped",
        "processing",
        "Delivered",
        "CANCELLED",
        "",
        "pending ",
    ];

    fn lf(text: &str) -> String {
        text.replace("\r\n", "\n")
    }

    /// The quoted strings of one line.
    fn quoted(line: &str) -> Vec<String> {
        line.split('"')
            .skip(1)
            .step_by(2)
            .map(str::to_string)
            .collect()
    }

    /// The names in the server's `VALID_STATUSES`, in its own order.
    fn vocabulary() -> Vec<String> {
        let server = lf(SERVER);
        let open = "const VALID_STATUSES: &[&str] = &[";
        let start = server
            .find(open)
            .expect("VALID_STATUSES is gone from src/api/orders.rs");
        let rest = &server[start + open.len()..];
        let body = &rest[..rest.find("];").expect("VALID_STATUSES never closes")];
        let names: Vec<String> = body
            .lines()
            .map(|l| l.split("//").next().unwrap_or(""))
            .flat_map(quoted)
            .collect();
        // D16: the floor is that the scan saw something. The count itself is the
        // owner's, and is compared with the owner's STATUS_COUNT below.
        assert!(!names.is_empty(), "read no names out of VALID_STATUSES");
        names
    }

    /// Every string of one `str` or `[N]str` constant of the order-status contract.
    fn owner_strs(name: &str) -> Vec<String> {
        let owner = lf(OWNER);
        let head = format!("pub const {name} : ");
        let mut lines = owner.lines().skip_while(|l| !l.starts_with(&head));
        let first = lines
            .next()
            .unwrap_or_else(|| panic!("{name} is no longer declared by turbobaby/order-status"));
        let mut text = first[first.find('=').unwrap_or(0)..].to_string();
        if first.contains("= [") && !first.trim_end().ends_with("];") {
            for line in lines {
                text.push_str(line);
                if line.trim_start().starts_with("];") {
                    break;
                }
            }
        }
        let values = quoted(&text);
        assert!(!values.is_empty(), "{name} holds no string");
        values
    }

    /// One `u8` constant of the order-status contract.
    fn owner_u8(name: &str) -> u8 {
        let owner = lf(OWNER);
        let head = format!("pub const {name} : u8 = ");
        let line = owner
            .lines()
            .find(|l| l.starts_with(&head))
            .unwrap_or_else(|| panic!("{name} is no longer declared by turbobaby/order-status"));
        line[head.len()..]
            .trim_end_matches(';')
            .trim()
            .parse()
            .unwrap_or_else(|_| panic!("{name} is not a u8"))
    }

    #[test]
    fn the_server_and_the_owner_name_the_same_vocabulary() {
        let server = vocabulary();
        let owner = owner_strs("STATUS_NAMES");
        assert_eq!(
            server, owner,
            "the server's whitelist and STATUS_NAMES disagree"
        );
        assert_eq!(server.len(), usize::from(owner_u8("STATUS_COUNT")));
    }

    /// Defect 1: absence was drawn as the most reassuring thing on the screen.
    #[test]
    fn an_unreadable_status_is_never_drawn_as_progress() {
        let everything = vocabulary();
        let nine: Vec<&str> = everything.iter().map(String::as_str).collect();
        let pipelines: [&[&str]; 3] = [&ORDER_PIPELINE[..4], ORDER_PIPELINE, &nine];
        for status in UNREADABLE {
            for pipeline in pipelines {
                let progress = progress_of(status, pipeline);
                assert_eq!(
                    progress,
                    Progress::Unreadable,
                    "{status:?} over {pipeline:?}"
                );
                assert!(draws_one_flat_bar(progress), "{status:?} draws segments");
                for index in 0..pipeline.len() {
                    assert!(
                        !segment_is_filled(progress, index),
                        "{status:?} fills segment {index} of {}",
                        pipeline.len()
                    );
                }
            }
        }
        let stopped: Vec<&String> = everything
            .iter()
            .filter(|n| arm_of(n) == StatusArm::CancelledOrRejected)
            .collect();
        assert!(!stopped.is_empty(), "no accepted name reads as stopped");
        for status in stopped {
            let progress = progress_of(status, ORDER_PIPELINE);
            assert_eq!(progress, Progress::Stopped, "{status}");
            assert!(draws_one_flat_bar(progress));
            assert!((0..ORDER_PIPELINE.len()).all(|i| !segment_is_filled(progress, i)));
        }
    }

    /// The alias as shipped, reached by its own arm. The write path stores only
    /// the persisted name, so a fix that emptied the bar for a name off the
    /// pipeline would empty it for every finished order.
    #[test]
    fn completed_reaches_the_finished_picture_by_its_own_arm() {
        let accepted = owner_strs("ALIAS_ACCEPTED_NAME");
        let persisted = owner_strs("ALIAS_PERSISTED_NAME");
        let missing = owner_strs("DISPLAY_PIPELINE_MISSING_NAMES");
        assert!(
            missing.contains(&persisted[0]),
            "the persisted name joined the pipeline; this arm's reason is gone"
        );
        for name in [&accepted[0], &persisted[0]] {
            let arm = arm_of(name);
            assert_eq!(arm, StatusArm::DeliveredOrCompleted, "{name}");
            let progress = progress_of(name, ORDER_PIPELINE);
            assert_eq!(progress, Progress::Finished, "{name}");
            assert!((0..ORDER_PIPELINE.len()).all(|i| segment_is_filled(progress, i)));
            assert!(!draws_one_flat_bar(progress));
            assert_eq!(arm.label_key(), T_ORDERS_STATUS_DELIVERED);
            assert_eq!(arm.step(), Some((T_ORDERS_STEP_DELIVERED, "🎉")));
            assert_eq!(arm.chip(), StatusChip::Completed);
            assert!(arm.reorder_offered());
        }
    }

    /// Defect 4: one reading, exact like the server's, and every answer follows it.
    #[test]
    fn the_reading_is_exact_like_the_server_and_every_answer_follows_it() {
        for name in vocabulary() {
            assert_ne!(arm_of(&name), StatusArm::Unresolved, "{name}");
            let mut capital = name.clone();
            capital[..1].make_ascii_uppercase();
            for spelled in [
                name.to_uppercase(),
                capital,
                format!(" {name}"),
                format!("{name} "),
            ] {
                let arm = arm_of(&spelled);
                assert_eq!(arm, StatusArm::Unresolved, "{spelled:?} was read as {name}");
                assert_eq!(arm.label_key(), T_ORDERS_STATUS_UNKNOWN, "{spelled:?}");
                assert_eq!(arm.color(), "#8b8b9e", "{spelled:?}");
                assert_eq!(arm.chip(), StatusChip::Active, "{spelled:?}");
                assert!(!arm.is_terminal(), "{spelled:?}");
                assert!(!arm.customer_may_cancel(), "{spelled:?}");
                assert!(!arm.reorder_offered(), "{spelled:?}");
                assert_eq!(arm.step(), None, "{spelled:?}");
                assert_eq!(progress_of(&spelled, ORDER_PIPELINE), Progress::Unreadable);
            }
        }
    }

    /// Defect 5: the server's word never reaches the customer as a label.
    #[test]
    fn no_step_label_is_ever_the_status_string() {
        for arm in StatusArm::ALL {
            if let Some((key, marker)) = arm.step() {
                assert!(STEP_KEYS.contains(&key), "{arm:?} steps to {key:?}");
                assert!(!marker.is_empty(), "{arm:?} has no marker");
            }
        }
        for name in vocabulary() {
            let arm = arm_of(&name);
            if let Some((key, _)) = arm.step() {
                assert_ne!(key, name.as_str());
            }
            if arm == StatusArm::CancelledOrRejected {
                assert_eq!(arm.step(), None, "{name} shows a step");
            }
        }
        for status in UNREADABLE {
            assert_eq!(arm_of(status).step(), None, "{status:?} shows a step");
        }
    }

    #[test]
    fn every_accepted_name_has_a_deliberate_arm() {
        let names = vocabulary();
        let mut may_cancel = 0;
        for name in &names {
            let arm = arm_of(name);
            assert_ne!(arm, StatusArm::Unresolved, "{name}");
            assert_ne!(
                progress_of(name, ORDER_PIPELINE),
                Progress::Unreadable,
                "{name} is drawn as unreadable"
            );
            assert_eq!(
                arm.chip() == StatusChip::Active,
                !arm.is_terminal(),
                "{name}"
            );
            assert_eq!(
                arm.step().is_some(),
                arm != StatusArm::CancelledOrRejected,
                "{name}"
            );
            if arm.customer_may_cancel() {
                may_cancel += 1;
            }
        }
        assert_eq!(
            may_cancel, 1,
            "a cancel button is offered on {may_cancel} names"
        );
    }

    #[test]
    fn the_chips_partition_every_arm_by_construction() {
        for arm in StatusArm::ALL {
            let chip = arm.chip();
            let under = [
                StatusChip::Active,
                StatusChip::Completed,
                StatusChip::Cancelled,
            ]
            .iter()
            .filter(|&&c| c == chip)
            .count();
            assert_eq!(under, 1, "{arm:?}");
            assert_eq!(chip == StatusChip::Active, !arm.is_terminal(), "{arm:?}");
        }
        assert_eq!(StatusArm::Unresolved.chip(), StatusChip::Active);
    }

    /// Every arm keeps the colour and the label key it shipped with, so moving the
    /// decision here changed nothing a customer sees for a name read correctly.
    /// Keyed by arm, not by server name, so this is no enumeration of the column.
    #[test]
    fn every_arm_keeps_the_colour_and_label_it_shipped_with() {
        let shipped: [(StatusArm, &str, Key); 8] = [
            (StatusArm::Pending, "#ffe600", T_ORDERS_STATUS_PENDING),
            (StatusArm::Confirmed, "#00e5ff", T_ORDERS_STATUS_CONFIRMED),
            (StatusArm::Preparing, "#ff9d00", T_ORDERS_STATUS_PREPARING),
            (StatusArm::Ready, "#39ff14", T_ORDERS_STATUS_READY),
            (
                StatusArm::OutForDelivery,
                "#00e5ff",
                T_ORDERS_STATUS_OUT_FOR_DELIVERY,
            ),
            (
                StatusArm::DeliveredOrCompleted,
                "#39ff14",
                T_ORDERS_STATUS_DELIVERED,
            ),
            (
                StatusArm::CancelledOrRejected,
                "#ff4757",
                T_ORDERS_STATUS_CANCELLED,
            ),
            (StatusArm::Unresolved, "#8b8b9e", T_ORDERS_STATUS_UNKNOWN),
        ];
        for (arm, colour, key) in shipped {
            assert_eq!(arm.color(), colour, "{arm:?}");
            assert_eq!(arm.label_key(), key, "{arm:?}");
        }
        assert_eq!(shipped.len(), StatusArm::ALL.len());
    }

    /// Tripwire for the terminal copy: turbobaby/order-status owns the set, and
    /// gate 3 binds that contract's TERMINAL_NAMES to the server's guard.
    #[test]
    fn terminality_is_the_order_status_contracts_and_this_copy_agrees() {
        let terminal = owner_strs("TERMINAL_NAMES");
        assert_eq!(terminal.len(), usize::from(owner_u8("TERMINAL_COUNT")));
        for name in vocabulary() {
            let arm = arm_of(&name);
            assert_eq!(
                arm.is_terminal(),
                terminal.contains(&name),
                "{name}: this copy and order-status disagree on terminality"
            );
            assert_eq!(arm.reorder_offered(), arm.is_terminal(), "{name}");
        }
    }

    /// Tripwire for the cancel copy: the owner's predicate names one code, that
    /// code names one status, and the server's own check compares the stored
    /// status with exactly that name.
    #[test]
    fn the_cancel_button_follows_the_owners_precondition_and_the_servers_check() {
        let owner = lf(OWNER);
        let opener = "pub fn order_status_customer_cancel_admitted(current: u8) bool {";
        let body = owner
            .split(opener)
            .nth(1)
            .expect("order_status_customer_cancel_admitted is gone from the owner");
        let returned = body
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .and_then(|l| l.strip_prefix("return current == "))
            .and_then(|l| l.strip_suffix(';'))
            .expect("the owner's cancel precondition is no longer one comparison");
        let code = usize::from(owner_u8(returned));
        let names = owner_strs("STATUS_NAMES");
        let admitted = &names[code - 1];

        let server = lf(SERVER);
        let needle = "if o.status != \"";
        assert_eq!(
            server.matches(needle).count(),
            1,
            "the server's cancel check moved"
        );
        let at = server.find(needle).map(|i| i + needle.len()).unwrap_or(0);
        let checked = &server[at..at + server[at..].find('"').unwrap_or(0)];
        assert_eq!(
            checked, admitted,
            "the server and the owner name different statuses"
        );

        for name in vocabulary() {
            assert_eq!(
                arm_of(&name).customer_may_cancel(),
                &name == admitted,
                "{name}: this copy and order-status disagree on the cancel button"
            );
        }
    }

    /// The pipeline's content, checked on the host: the owner's vocabulary minus
    /// the names the owner says it leaves out, in the owner's order.
    #[test]
    fn the_pipeline_is_the_vocabulary_minus_the_names_order_status_leaves_out() {
        let names = owner_strs("STATUS_NAMES");
        let missing = owner_strs("DISPLAY_PIPELINE_MISSING_NAMES");
        let expected: Vec<&str> = names
            .iter()
            .filter(|n| !missing.contains(n))
            .map(String::as_str)
            .collect();
        assert_eq!(ORDER_PIPELINE, expected.as_slice());
        assert_eq!(
            ORDER_PIPELINE.len(),
            usize::from(owner_u8("DISPLAY_PIPELINE_VARIANT_COUNT"))
        );
        for step in ORDER_PIPELINE {
            let arm = arm_of(step);
            assert_ne!(arm, StatusArm::Unresolved, "{step}");
            assert_ne!(arm, StatusArm::CancelledOrRejected, "{step}");
        }
        for name in vocabulary() {
            let arm = arm_of(&name);
            if arm.is_terminal() {
                continue;
            }
            let at = ORDER_PIPELINE.iter().position(|s| *s == name);
            assert_eq!(
                progress_of(&name, ORDER_PIPELINE),
                at.map_or(Progress::Unreadable, Progress::AtStep),
                "{name}"
            );
            assert!(at.is_some(), "{name} is in flight and has no step");
        }
    }
}
