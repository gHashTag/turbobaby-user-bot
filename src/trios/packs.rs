//! Pure, host-testable core for the "Наборы" (Packs) section.
//!
//! A pack (a row in the `sets` table — a strain bundle) carries an optional
//! promo `badge` and a `total_weight_grams`. This module owns the badge enum +
//! its bilingual labels/colours, the "is this pack promotional" rule that drives
//! the carousel-first ordering, and the weight/strain-count display string.
//!
//! Lives in `trios` (shared backend + WASM) following the functional-core
//! pattern of `garden::first_seedable_item` and `drink_categories`.

/// A pack's promo badge. One per pack (owner decision). The auto `% OFF`
/// discount badge is separate and rendered independently of this.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackBadge {
    None,
    Sale,
    Special,
    Limited,
}

impl std::str::FromStr for PackBadge {
    type Err = std::convert::Infallible;

    /// Parse the DB / API string value. Unknown / empty → `None` (fail safe).
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.trim().to_lowercase().as_str() {
            "sale" => PackBadge::Sale,
            "special" => PackBadge::Special,
            "limited" => PackBadge::Limited,
            _ => PackBadge::None,
        })
    }
}

impl PackBadge {
    /// Parse wrapper for call sites that expect the badge directly.
    pub fn parse(s: &str) -> Self {
        std::str::FromStr::from_str(s).unwrap_or(PackBadge::None)
    }

    /// Canonical DB value.
    pub fn as_str(self) -> &'static str {
        match self {
            PackBadge::None => "none",
            PackBadge::Sale => "sale",
            PackBadge::Special => "special",
            PackBadge::Limited => "limited",
        }
    }

    /// Whether `s` is an accepted badge value (server-side validation).
    pub fn is_valid(s: &str) -> bool {
        matches!(
            s.trim().to_lowercase().as_str(),
            "none" | "sale" | "special" | "limited"
        )
    }

    /// Bilingual display label `(ru, en)`, or `None` for no badge.
    pub fn label(self) -> Option<(&'static str, &'static str)> {
        match self {
            PackBadge::None => None,
            PackBadge::Sale => Some(("СКИДКА", "SALE")),
            PackBadge::Special => Some(("СПЕЦПРЕДЛОЖЕНИЕ", "SPECIAL OFFER")),
            PackBadge::Limited => Some(("ЛИМИТКА", "LIMITED EDITION")),
        }
    }

    /// Chip background colour (hex). `None` has no chip so any value is fine.
    pub fn color(self) -> &'static str {
        match self {
            PackBadge::None => "#2a2a4a",
            PackBadge::Sale => "#ff4757",    // red
            PackBadge::Special => "#ffb300", // amber
            PackBadge::Limited => "#b388ff", // violet (sets accent)
        }
    }
}

/// A pack is "promotional" (sorts into the carousel, shown first) if it carries
/// a badge OR a positive discount.
pub fn is_promo(badge: &str, discount_percent: f64) -> bool {
    PackBadge::parse(badge) != PackBadge::None
        || (discount_percent.is_finite() && discount_percent > 0.0)
}

/// Grams per strain = total ÷ count. `None` when the pack has no strains (avoids
/// a divide-by-zero and lets the caller omit the "по Xг" suffix).
pub fn per_strain_grams(total: f64, strain_count: usize) -> Option<f64> {
    if strain_count == 0 || !total.is_finite() || total <= 0.0 {
        None
    } else {
        Some(total / strain_count as f64)
    }
}

/// Format a number without a trailing ".0" (2.0 → "2", 2.5 → "2.5").
fn fmt_g(v: f64) -> String {
    if (v.fract()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        // One decimal is plenty for grams.
        let s = format!("{:.1}", v);
        s
    }
}

/// Card meta line: `"10G Total · 5 сортов по 2г"`. Falls back gracefully:
/// - count == 0 or weight <= 0 → just `"{w}G Total"` (or empty if no weight);
/// - otherwise append the per-strain breakdown.
///
/// `strain_word` is the (already pluralised/localised) word for "strains" so the
/// caller controls RU/EN ("сортов" / "strains").
pub fn weight_line(total: f64, strain_count: usize, strain_word: &str) -> String {
    let total_ok = total.is_finite() && total > 0.0;
    let head = if total_ok {
        format!("{}G Total", fmt_g(total))
    } else {
        String::new()
    };
    match (total_ok, per_strain_grams(total, strain_count)) {
        (true, Some(per)) => format!("{head} · {strain_count} {strain_word} по {}г", fmt_g(per)),
        (true, None) => head,
        (false, _) if strain_count > 0 => format!("{strain_count} {strain_word}"),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn badge_parse_roundtrip_and_unknown_is_none() {
        for b in [
            PackBadge::None,
            PackBadge::Sale,
            PackBadge::Special,
            PackBadge::Limited,
        ] {
            assert_eq!(PackBadge::parse(b.as_str()), b);
        }
        assert_eq!(PackBadge::parse("SALE"), PackBadge::Sale);
        assert_eq!(PackBadge::parse(" limited "), PackBadge::Limited);
        assert_eq!(PackBadge::parse("bogus"), PackBadge::None);
        assert_eq!(PackBadge::parse(""), PackBadge::None);
    }

    #[test]
    fn badge_validation() {
        assert!(PackBadge::is_valid("none"));
        assert!(PackBadge::is_valid("SALE"));
        assert!(PackBadge::is_valid("special"));
        assert!(!PackBadge::is_valid("bogus"));
        assert!(!PackBadge::is_valid(""));
    }

    #[test]
    fn label_present_only_for_real_badges() {
        assert_eq!(PackBadge::None.label(), None);
        assert_eq!(PackBadge::Sale.label(), Some(("СКИДКА", "SALE")));
        assert_eq!(PackBadge::Limited.label().unwrap().1, "LIMITED EDITION");
    }

    #[test]
    fn promo_rule() {
        assert!(is_promo("sale", 0.0));
        assert!(is_promo("none", 15.0));
        assert!(is_promo("limited", 0.0));
        assert!(!is_promo("none", 0.0));
        assert!(!is_promo("none", f64::NAN));
        assert!(!is_promo("", -5.0));
    }

    #[test]
    fn per_strain_guards_div_by_zero() {
        assert_eq!(per_strain_grams(10.0, 0), None);
        assert_eq!(per_strain_grams(0.0, 5), None);
        assert_eq!(per_strain_grams(10.0, 5), Some(2.0));
    }

    #[test]
    fn weight_line_formats() {
        assert_eq!(weight_line(10.0, 5, "сортов"), "10G Total · 5 сортов по 2г");
        assert_eq!(weight_line(7.0, 0, "сортов"), "7G Total");
        assert_eq!(weight_line(0.0, 5, "strains"), "5 strains");
        assert_eq!(weight_line(0.0, 0, "сортов"), "");
        // Fractional per-strain keeps one decimal; integers drop ".0".
        assert_eq!(
            weight_line(10.0, 4, "strains"),
            "10G Total · 4 strains по 2.5г"
        );
    }
}
