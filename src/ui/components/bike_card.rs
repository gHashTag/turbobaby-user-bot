// Bike Card Component for the Catalog
use crate::trios::i18n::{t, T_ADD_TO_CART, T_MENU_SOLD_OUT};
use crate::ui::components::{Button, ButtonVariant};
use crate::ui::lang;
use dioxus::prelude::*;

/// What an absent money field renders as.
///
/// An em dash, not `0`, not an average, not a "from" price (D9).
pub const DASH: &str = "—";

/// The one money renderer in the UI: a published number, or a dash.
///
/// This is the single rendering helper `#7` asks for. It is reached from the
/// catalog, the bike detail screen and the admin read-only views under the
/// names `money_thb` / `finite_money` / `MONEY_DASH`, which are re-exports of
/// these three items from `screens::catalog_screen` — that module used to
/// carry a second, identical implementation, and both copies carried a doc
/// comment calling itself the only one.
///
/// It lives here, in the component layer, because a component must not depend
/// on a screen.
///
/// Two properties matter and are deliberate:
///
/// 1. `None` and any non-finite value return **before**
///    `pricing::format_baht` is reached, so that function's internal
///    `sanitize_money` zero-fallback (`NaN -> 0.0`, which reads as FREE)
///    cannot be hit by an absent price.
/// 2. A non-positive rate is treated as absent too. A rented motorbike at
///    `฿0/day` is not a price, it is corrupt input, and `฿0` is the single
///    most misleading string this card could print.
pub fn thb_or_dash(amount: Option<f64>) -> String {
    match published(amount) {
        Some(v) => crate::trios::pricing::format_baht(v),
        None => DASH.to_string(),
    }
}

/// The filter behind [`thb_or_dash`], exposed so callers can branch on
/// "is there a price at all" without parsing the rendered string.
///
/// The rule itself moved to `crate::trios::pricing::published_money` on
/// 2026-09-21 and this is now a delegation, because the server, the cart wire
/// and `cargo test` all need the same boundary and none of them can link this
/// module -- `src/lib.rs` gates `pub mod ui;` on `wasm32` (D15). The NAME stays
/// here: `tests/money_is_never_invented.rs` pins it as the component layer's
/// canonical helper, and the copy it was pinned against has grown back once
/// already.
pub fn published(amount: Option<f64>) -> Option<f64> {
    crate::trios::pricing::published_money(amount)
}

/// Scooter or motorcycle — the only two classes the fleet has.
///
/// The class is display and filtering only. The class discount is applied by
/// whoever resolved the rate (D11: the door decides the number), never here.
#[derive(PartialEq, Clone, Copy, Default)]
pub enum BikeClass {
    #[default]
    Scooter,
    Motorcycle,
}

impl BikeClass {
    pub fn css_class(&self) -> &'static str {
        match self {
            Self::Scooter => "scooter",
            Self::Motorcycle => "motorcycle",
        }
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            Self::Scooter => "🛵",
            Self::Motorcycle => "🏍️",
        }
    }

    pub fn label(&self) -> String {
        match self {
            Self::Scooter => lang::localized("Скутер", Some("Scooter")),
            Self::Motorcycle => lang::localized("Мотоцикл", Some("Motorcycle")),
        }
    }
}

/// Public, serializable data needed to render a bike card.
///
/// Declared here rather than reusing a wire type so this component compiles
/// against nothing but itself; the same arrangement `SetCardData` uses.
///
/// Every money field is `Option<f64>` and every one of them means what the
/// fleet source means by `null`: nothing is published. The card renders that
/// as a dash and says a human quotes the price — it never computes one (D11).
#[derive(PartialEq, Clone, Default)]
pub struct BikeCardData {
    /// Family key, e.g. `nmax-155`. A customer books a family; the shop
    /// assigns the unit (D8).
    pub id: String,
    pub brand: String,
    pub model: String,
    /// Trim / year / colour line, when the family has one.
    pub variant_label: Option<String>,
    pub displacement_cc: Option<u32>,
    pub class: BikeClass,
    /// Units of this family free to rent right now.
    pub units_available: u32,
    /// Client-facing day rate, already resolved by the caller.
    ///
    /// This is the number the door returned, post class-discount. The card
    /// does not discount, average or interpolate it, and a `None` here is not
    /// a hole to fill — it is the answer (D11).
    pub rate_thb_day: Option<f64>,
    pub deposit_thb: Option<f64>,
    pub monthly_low_season_thb: Option<f64>,
    pub image_url: Option<String>,
    pub description: Option<String>,
    /// `false` closes the family to new rentals even while units exist —
    /// CLICK 125 is the live instance (D12).
    pub offered: bool,
}

impl BikeCardData {
    /// `brand` + `model`, with neither doubled if the model already carries
    /// the brand.
    pub fn title(&self) -> String {
        let brand = self.brand.trim();
        let model = self.model.trim();
        if brand.is_empty() {
            return model.to_string();
        }
        if model.is_empty() {
            return brand.to_string();
        }
        if model.to_lowercase().starts_with(&brand.to_lowercase()) {
            model.to_string()
        } else {
            format!("{brand} {model}")
        }
    }

    /// The day rate if one is published, `None` otherwise.
    pub fn published_rate(&self) -> Option<f64> {
        published(self.rate_thb_day)
    }

    /// Rentable now: offered, and at least one unit free.
    pub fn is_available(&self) -> bool {
        self.offered && self.units_available > 0
    }

    /// Addable to a cart.
    ///
    /// A family with no published rate is deliberately **not** addable: the
    /// cart's `unit_price` column rejects NULL and only accepts `>= 0`, so the
    /// sole value that would get past it is `0`, which lands in a real cart as
    /// a real free rental (D9, fourth construct). Refusing the add is the
    /// honest outcome — a machine whose price nobody has set is a machine we
    /// cannot yet take money for.
    pub fn can_add_to_cart(&self) -> bool {
        self.is_available() && self.published_rate().is_some()
    }
}

#[derive(Props, PartialEq, Clone)]
pub struct BikeCardProps {
    bike: BikeCardData,
    /// Marks the family as the catalog's highlighted pick.
    #[props(default = false)]
    featured: bool,
    #[props(default)]
    on_add_to_cart: EventHandler<BikeCardData>,
}

#[component]
pub fn BikeCard(props: BikeCardProps) -> Element {
    let lang = lang::current_lang();
    let bike = props.bike.clone();
    let bike_for_callback = props.bike.clone();
    let featured = props.featured;
    // `EventHandler` is `Copy`, so lifting it out of `props` here keeps the
    // closure below from capturing `props` itself.
    let on_add_to_cart = props.on_add_to_cart;

    let is_available = bike.is_available();
    let can_add = bike.can_add_to_cart();
    let title = bike.title();
    let add_label = t(lang, T_ADD_TO_CART);
    let sold_label = t(lang, T_MENU_SOLD_OUT);

    // Price. `rate_line` is a dash whenever the day rate is absent, and the
    // quote note replaces the add-to-cart button in exactly that case, so the
    // card never shows a number the door did not give us.
    let rate_line = thb_or_dash(bike.rate_thb_day);
    let has_rate = bike.published_rate().is_some();
    let per_day = lang::localized("/сутки", Some("/day"));
    let quote_note = lang::localized(
        "Цену уточнит менеджер",
        Some("A manager will quote this price"),
    );
    let deposit_label = lang::localized("Залог", Some("Deposit"));
    let deposit_line = thb_or_dash(bike.deposit_thb);
    let monthly_label = lang::localized("В месяц", Some("Per month"));
    let monthly_line = thb_or_dash(bike.monthly_low_season_thb);
    let has_monthly = published(bike.monthly_low_season_thb).is_some();

    let displacement_line = bike
        .displacement_cc
        .map(|cc| format!("{cc} {}", lang::localized("см³", Some("cc"))));
    let availability_line = if is_available {
        format!(
            "{} {}",
            lang::localized("Свободно:", Some("Available:")),
            bike.units_available
        )
    } else if bike.offered {
        lang::localized("Все в аренде", Some("All rented out"))
    } else {
        lang::localized("Сейчас не сдаём", Some("Not rented at the moment"))
    };

    let class_css = bike.class.css_class();
    let class_emoji = bike.class.emoji();
    let class_label = bike.class.label();
    let featured_badge = lang::localized("ВЫБОР", Some("PICK"));

    rsx! {
        div {
            class: "bike-card",
            class: if featured { "bike-featured" } else { "" },
            class: if !is_available { "unavailable" } else { "" },

            if featured {
                div { class: "featured-badge", "{featured_badge}" }
            }

            // Image
            div { class: "bike-image",
                {
                    let url = bike.image_url.clone().unwrap_or_default();
                    if !url.is_empty() && (url.starts_with("http://") || url.starts_with("https://") || (url.starts_with("/") && !url.starts_with("//"))) {
                        rsx! {
                            img {
                                src: "{url}?v=2",
                                alt: "{title}",
                                loading: "lazy"
                            }
                        }
                    } else {
                        rsx! { div { class: "bike-placeholder", "{class_emoji}" } }
                    }
                }
            }

            // Content
            div { class: "bike-content",
                h3 { class: "bike-name", "{title}" }
                if let Some(variant) = &bike.variant_label {
                    div { class: "bike-variant", "{variant}" }
                }
                div { class: "bike-class",
                    span { class: "class-badge {class_css}",
                        "{class_emoji} {class_label}"
                    }
                    if let Some(cc) = &displacement_line {
                        span { class: "bike-displacement", "{cc}" }
                    }
                }
                div { class: "bike-availability", "{availability_line}" }
                if let Some(description) = &bike.description {
                    p { class: "bike-description", "{description}" }
                }
                div { class: "bike-price",
                    if has_rate {
                        span { class: "price", "{rate_line}" }
                        span { class: "price-unit", "{per_day}" }
                    } else {
                        // No number at all — not exact, not "from", not a
                        // range. Invention and silence are forbidden
                        // equally, so the dash is paired with the note
                        // below saying who to ask (D11).
                        span { class: "price price-absent", "{rate_line}" }
                    }
                }
                div { class: "bike-terms",
                    span { class: "bike-deposit", "{deposit_label}: {deposit_line}" }
                    if has_monthly {
                        span { class: "bike-monthly", "{monthly_label}: {monthly_line}" }
                    }
                }
                if !has_rate {
                    p { class: "bike-quote-note", "{quote_note}" }
                }
            }

            // Add to cart button
            div { class: "bike-actions",
                Button {
                    variant: if can_add { ButtonVariant::Primary } else { ButtonVariant::Ghost },
                    disabled: !can_add,
                    onclick: move |_| {
                        on_add_to_cart.call(bike_for_callback.clone());
                    },
                    if can_add {
                        "{add_label} 🛒"
                    } else if !has_rate {
                        "{quote_note}"
                    } else {
                        "{sold_label}"
                    }
                }
            }
        }
    }
}
