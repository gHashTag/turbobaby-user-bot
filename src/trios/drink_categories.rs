//! Pure, host-testable core for the Drinks section's category model.
//!
//! Drink categories are defined *by the data*: a category exists iff at least
//! one drink (`tea_products` row) carries it. A drink carries a bilingual label
//! pair — `subcategory` (RU) + `subcategory_en` (EN), both free-text. This
//! module turns those raw label pairs into the ordered category lists rendered
//! as menu filter-chips (customer) and the admin create/edit dropdown.
//!
//! Two rules (owner-confirmed):
//! - **Bilingual:** a new category is entered as RU + EN; chips/cards localize.
//! - **Legacy fold:** historical fine-grained tea subcats (green/black/herbal/
//!   oolong/pu-erh/…) collapse into ONE canonical `tea` category ("Чай / Tea").
//!
//! Identity is the **canonical key**, computed from the EN label (the stable
//! slug) so old rows (`subcategory="milkshake"`, no `_en`) and new rows
//! (`subcategory="Милкшейк"`, `subcategory_en="Milkshake"`) group together.
//!
//! Lives in `trios` (the functional core shared by backend + WASM) following
//! the same pattern as `packs` and `pricing`. The exemplar named here used to
//! be `garden::first_seedable_item`, deleted with the garden (D5).

/// Lowercased legacy tea labels / keys that all fold into the single canonical
/// `tea` category. Covers the seed-018 keys and their EN display labels.
const LEGACY_TEA: &[&str] = &[
    "tea",
    "green",
    "black",
    "herbal",
    "oolong",
    "puer",
    "pu-erh",
    "pu-er",
    "white",
    "cbd tea",
    "green tea",
    "black tea",
    "herbal tea",
    "oolong tea",
    "pu-er tea",
    "pu-erh tea",
    "white tea",
    "flower",
    "teaware",
    "dessert",
    "fruit",
];

/// Built-in starter categories `(key, ru, en)`, always offered in the admin
/// dropdown (so the very first drink of a type can be created) and used as the
/// preferred display order. `key` is the canonical key these labels resolve to.
pub const STARTER: &[(&str, &str, &str)] = &[
    ("tea", "Чай", "Tea"),
    ("coffee", "Кофе", "Coffee"),
    ("lemonade", "Лимонад", "Lemonade"),
    ("milkshake", "Милкшейк", "Milkshake"),
    ("soda", "Газировка", "Soda"),
    ("juice", "Сок", "Juice"),
];

/// One drink category, ready for display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrinkCategory {
    /// Canonical join key (lowercased, legacy-folded).
    pub key: String,
    /// Russian display label.
    pub ru: String,
    /// English display label.
    pub en: String,
}

/// Compute the canonical category key for a drink's `(subcategory,
/// subcategory_en)` pair. Joins on the EN label when present (the stable slug),
/// folds legacy tea variants → `tea`, and lowercases/trims. Empty → `tea`.
pub fn canonical_key(subcategory: &str, subcategory_en: Option<&str>) -> String {
    let basis = subcategory_en
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| subcategory.trim());
    let s = basis.trim().to_lowercase();
    if s.is_empty() || LEGACY_TEA.contains(&s.as_str()) {
        "tea".to_string()
    } else {
        s
    }
}

/// Built-in bilingual labels for a known canonical key. `None` for data-defined
/// (admin-created) categories — the caller falls back to the row's own labels.
pub fn label(key: &str) -> Option<(&'static str, &'static str)> {
    STARTER
        .iter()
        .find(|(k, _, _)| *k == key)
        .map(|(_, ru, en)| (*ru, *en))
}

/// Emoji for a canonical category key (visual chip / dropdown decoration).
pub fn emoji(key: &str) -> &'static str {
    match key {
        "coffee" => "☕",
        "lemonade" => "🍋",
        "milkshake" => "🥛",
        "soda" => "🥤",
        "juice" => "🧃",
        "smoothie" => "🥤",
        "tea" => "🍵",
        _ => "🥤",
    }
}

/// Index of `key` in [`STARTER`], or `usize::MAX` for non-starter categories
/// (so they sort after the starters). Used as the primary ordering key.
fn starter_rank(key: &str) -> usize {
    STARTER
        .iter()
        .position(|(k, _, _)| *k == key)
        .unwrap_or(usize::MAX)
}

/// Resolve a `(key, raw_ru, raw_en)` triple into a [`DrinkCategory`], preferring
/// the built-in label for known keys and the row's own labels otherwise.
fn make_category(key: String, raw_ru: &str, raw_en: Option<&str>) -> DrinkCategory {
    if let Some((ru, en)) = label(&key) {
        DrinkCategory {
            key,
            ru: ru.to_string(),
            en: en.to_string(),
        }
    } else {
        let ru = raw_ru.trim();
        let en = raw_en
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(ru);
        DrinkCategory {
            key,
            ru: ru.to_string(),
            en: en.to_string(),
        }
    }
}

/// Distinct categories **present in the data**, deduped by canonical key and
/// ordered (STARTER order first, then extras alphabetically by RU label).
/// A row whose `_en` is set is preferred as the label source for its key.
/// → customer menu chips.
pub fn present_categories<'a, I>(items: I) -> Vec<DrinkCategory>
where
    I: IntoIterator<Item = (&'a str, Option<&'a str>)>,
{
    let mut out: Vec<DrinkCategory> = Vec::new();
    for (sub, sub_en) in items {
        let key = canonical_key(sub, sub_en);
        match out.iter().position(|c| c.key == key) {
            Some(idx) => {
                // Upgrade the label if this row carries an EN label and the
                // stored one was a bare fallback (non-starter keys only).
                if label(&key).is_none() {
                    if let Some(en) = sub_en.map(str::trim).filter(|s| !s.is_empty()) {
                        out[idx].en = en.to_string();
                        if !sub.trim().is_empty() {
                            out[idx].ru = sub.trim().to_string();
                        }
                    }
                }
            }
            None => out.push(make_category(key, sub, sub_en)),
        }
    }
    sort_categories(&mut out);
    out
}

/// STARTER ∪ present, deduped by key, same ordering. → admin dropdown (lets the
/// admin pick a built-in category even before any drink uses it).
pub fn admin_categories<'a, I>(items: I) -> Vec<DrinkCategory>
where
    I: IntoIterator<Item = (&'a str, Option<&'a str>)>,
{
    let mut out = present_categories(items);
    for (key, ru, en) in STARTER {
        if !out.iter().any(|c| &c.key == key) {
            out.push(DrinkCategory {
                key: key.to_string(),
                ru: ru.to_string(),
                en: en.to_string(),
            });
        }
    }
    sort_categories(&mut out);
    out
}

fn sort_categories(cats: &mut [DrinkCategory]) {
    cats.sort_by(|a, b| {
        starter_rank(&a.key)
            .cmp(&starter_rank(&b.key))
            .then_with(|| a.ru.to_lowercase().cmp(&b.ru.to_lowercase()))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_tea_variants_fold_into_tea() {
        for s in [
            "green",
            "Black",
            "HERBAL",
            "oolong",
            "pu-erh",
            "Green Tea",
            "tea",
            "",
        ] {
            assert_eq!(canonical_key(s, None), "tea", "{s} should fold to tea");
        }
    }

    #[test]
    fn old_and_new_milkshake_unify() {
        // Old A2 row: english key in `subcategory`, no `_en`.
        let old = canonical_key("milkshake", None);
        // New bilingual row: RU in `subcategory`, EN in `subcategory_en`.
        let new = canonical_key("Милкшейк", Some("Milkshake"));
        assert_eq!(old, "milkshake");
        assert_eq!(new, "milkshake");
        assert_eq!(old, new);
    }

    #[test]
    fn new_category_passes_through_lowercased() {
        assert_eq!(canonical_key("Смузи", Some("Smoothie")), "smoothie");
    }

    #[test]
    fn present_categories_dedup_fold_and_order() {
        let items = vec![
            ("green", Some("Green Tea")),
            ("black", Some("Black Tea")),
            ("Милкшейк", Some("Milkshake")),
            ("Смузи", Some("Smoothie")),
            ("milkshake", None), // old row, same key as Милкшейк
        ];
        let cats = present_categories(items);
        let keys: Vec<&str> = cats.iter().map(|c| c.key.as_str()).collect();
        // tea (starter rank 0) before milkshake (rank 3) before smoothie (extra).
        assert_eq!(keys, vec!["tea", "milkshake", "smoothie"]);
        // Built-in labels win for known keys; data labels for new ones.
        let tea = &cats[0];
        assert_eq!((tea.ru.as_str(), tea.en.as_str()), ("Чай", "Tea"));
        let smoothie = &cats[2];
        assert_eq!(
            (smoothie.ru.as_str(), smoothie.en.as_str()),
            ("Смузи", "Smoothie")
        );
    }

    #[test]
    fn admin_categories_includes_all_starters() {
        let cats = admin_categories(vec![("Смузи", Some("Smoothie"))]);
        for (key, _, _) in STARTER {
            assert!(cats.iter().any(|c| &c.key == key), "missing starter {key}");
        }
        assert!(cats.iter().any(|c| c.key == "smoothie"));
        // Starters first, in STARTER order.
        assert_eq!(cats[0].key, "tea");
    }

    #[test]
    fn present_categories_empty_when_no_drinks() {
        assert!(present_categories(Vec::<(&str, Option<&str>)>::new()).is_empty());
    }
}
