//! Source-level guard for the public TurboBaby navigation cutover.
//!
//! Legacy screens still compile while their data paths are retired. These
//! checks make the customer-facing entry points a much narrower contract: the
//! ordinary root route is the bike catalog, and the tab bar advertises only
//! fleet, Ride, orders, cart, and profile.

const ROUTES: &str = include_str!("../src/ui/routes.rs");
const BOTTOM_NAV: &str = include_str!("../src/ui/components/bottom_nav.rs");
const I18N: &str = include_str!("../src/trios/i18n.rs");

#[test]
fn ordinary_home_visit_renders_the_bike_catalog() {
    let start = ROUTES.find("fn Home() -> Element").expect("Home route");
    let end = ROUTES[start..]
        .find("fn Menu() -> Element")
        .map(|offset| start + offset)
        .expect("Menu route after Home");
    let home = &ROUTES[start..end];

    assert!(home.contains("if has_compatibility_redirect"));
    assert!(home.contains("HomeScreen {}"));
    assert!(home.contains("else {\n                CatalogScreen {}"));
}

#[test]
fn primary_navigation_advertises_only_live_bike_surfaces() {
    for destination in ["Home", "Ride", "Orders", "Cart", "Profile"] {
        assert!(
            BOTTOM_NAV.contains(&format!("Link {{ to: Route::{destination} {{}}")),
            "missing {destination} from primary navigation"
        );
    }
    assert_eq!(BOTTOM_NAV.matches("Link { to: Route::").count(), 5);

    for retired in ["Garden", "Sets", "Accessories", "Tea", "Events"] {
        assert!(
            !BOTTOM_NAV.contains(&format!("Link {{ to: Route::{retired} {{}}")),
            "legacy {retired} route is still advertised"
        );
    }
}

#[test]
fn bike_navigation_labels_exist_in_both_customer_locales() {
    for declaration in ["T_NAV_FLEET", "T_NAV_RIDE", "T_NAV_ORDERS"] {
        assert!(I18N.contains(&format!("pub const {declaration}: Key")));
    }
    for russian in ["Байки", "Заезд", "Заказы"] {
        assert!(I18N.contains(russian));
    }
    for english in ["Bikes", "Ride", "Orders"] {
        assert!(I18N.contains(english));
    }
}
