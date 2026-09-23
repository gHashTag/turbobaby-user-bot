// Shared order-status stepper used by the orders list and order detail screen.
//
// It only draws. Every decision about the status string -- the label, the
// colour, the step shown on the left and how much of the pipeline fills -- comes
// from crate::trios::order_status_view, the one exact reading both screens
// share, which the host compiles and tests. A stopped or unreadable status is
// drawn as one flat bar in its own colour, with no step on the left and never
// the server's own word. The pipeline the segments walk is declared in that
// module as well; turbobaby/order-status owns what it contains.
// Contract: specs/turbobaby/order_presentation.t27.

use crate::trios::i18n::t;
use crate::trios::order_status_view::{
    arm_of, draws_one_flat_bar, progress_of, segment_is_filled, ORDER_PIPELINE,
};
use dioxus::prelude::*;

#[component]
fn PipelineSegment(filled: bool, last: bool) -> Element {
    let bg = if filled { "#39ff14" } else { "#2a2a4a" };
    let flex = if last { "0 0 8px" } else { "1" };
    let shape = if last {
        "border-radius: 50%;"
    } else {
        "border-radius: 2px;"
    };
    rsx! {
        div { style: "{shape} height: 4px; background: {bg}; flex: {flex}; min-width: 8px;" }
        if !last {
            div { style: "width: 4px; height: 4px; background: {bg};" }
        }
    }
}

/// Compact horizontal order-status tracker.
///
/// `margin_bottom` lets callers tune spacing for the list vs detail contexts.
#[component]
pub fn StatusStepper(status: String, #[props(default = 8)] margin_bottom: u32) -> Element {
    let lang = crate::ui::lang::current_lang();
    let arm = arm_of(&status);
    let progress = progress_of(&status, ORDER_PIPELINE);
    let flat = draws_one_flat_bar(progress);
    let step = arm
        .step()
        .map(|(key, marker)| format!("{marker} {}", t(lang, key)))
        .unwrap_or_default();
    let color = arm.color();
    let status_label = t(lang, arm.label_key());

    rsx! {
        div { style: "margin-bottom: {margin_bottom}px;",
            div { style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 6px;",
                span { style: "font-size: 13px; color: #8b8b9e;",
                    "{step}"
                }
                span { style: "font-size: 12px; color: {color};",
                    "{status_label}"
                }
            }
            div { style: "display: flex; align-items: center; gap: 4px;",
                if flat {
                    div { style: "flex:1;height:4px;background:{color};border-radius:2px;" }
                } else {
                    for i in 0..ORDER_PIPELINE.len() {
                        PipelineSegment { filled: segment_is_filled(progress, i), last: i == ORDER_PIPELINE.len() - 1 }
                    }
                }
            }
        }
    }
}
