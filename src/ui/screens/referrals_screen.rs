// Referrals screen wrapper
use dioxus::prelude::*;
use crate::ui::pages::referrals::Referrals;

#[component]
pub fn ReferralsScreen() -> Element {
    rsx! { Referrals {} }
}
