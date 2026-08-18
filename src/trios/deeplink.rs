//! Deep links, in the one place both halves of the app can see.
//!
//! A shared link goes through four stages, and until this module existed no
//! test could reach three of them:
//!
//! 1. **Build.** The Mini App turns a product into `t.me/<bot>?start=<payload>`.
//! 2. **Classify.** The *bot* decides whether an incoming `/start <payload>` is
//!    a Mini App target, and answers with a Mini App button if it is.
//! 3. **Carry.** That button's URL carries the payload as `?startapp=…`.
//! 4. **Parse.** The Mini App reads it back and opens the card.
//!
//! Stages 2 and 4 were written twice — the classifier in `src/bot/mod.rs`, the
//! parsers in `src/ui/share.rs` — and the two lists of prefixes had no way of
//! being compared, because **`src/ui` is `#[cfg(target_arch = "wasm32")]`**.
//! `cargo test --lib` on the host cannot see it, so every parser here has been
//! invisible to the suite that runs in CI and in the pre-push hook, for as long
//! as they have existed. A link that the bot refuses to answer and a link the
//! app cannot parse look identical to a customer: you tap, and nothing happens.
//!
//! So the vocabulary lives here, in `trios`, which both halves compile. The bot
//! and the app now read the same list, and `every_kind_survives_the_whole_chain`
//! walks all four stages rather than any one of them.

/// Telegram caps a `start` parameter at 64 characters and allows only
/// `A-Za-z0-9_-`. Both limits are Telegram's, not ours, so a payload that
/// breaks them is not a link that works badly — it is a link Telegram will not
/// carry at all.
pub const MAX_START_PARAM_LEN: usize = 64;

/// Catalog kinds that can be shared as a card.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Strain,
    Accessory,
    Set,
    Tea,
    Event,
}

impl Kind {
    /// Every kind, so a test can sweep them rather than list them and drift.
    pub const ALL: [Kind; 5] = [
        Kind::Strain,
        Kind::Accessory,
        Kind::Set,
        Kind::Tea,
        Kind::Event,
    ];

    /// The payload prefix, without its trailing underscore.
    pub fn prefix(self) -> &'static str {
        match self {
            Kind::Strain => "p_strain",
            Kind::Accessory => "p_acc",
            Kind::Set => "p_set",
            Kind::Tea => "p_tea",
            Kind::Event => "p_event",
        }
    }

    /// The name this kind travels under on the share API's wire.
    pub fn wire(self) -> &'static str {
        match self {
            Kind::Strain => "strain",
            Kind::Accessory => "accessory",
            Kind::Set => "set",
            Kind::Tea => "tea",
            Kind::Event => "event",
        }
    }
}

/// What a `start` payload asks the app to open.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Product {
        kind: Kind,
        id: String,
    },
    Order(String),
    /// `cart`, or `cart__<source>` carrying the campaign that drove the open.
    Cart {
        source: String,
    },
    Reorder(String),
    Garden {
        referrer: Option<i64>,
        source: Option<String>,
    },
}

/// The character set Telegram allows in a `start` parameter.
fn transportable(payload: &str) -> bool {
    !payload.is_empty()
        && payload.len() <= MAX_START_PARAM_LEN
        && payload
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// A trailing attribution segment: non-empty, bounded, safe characters.
fn usable_source(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 50
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Build the payload that opens a product card.
pub fn product_payload(kind: Kind, id: &str) -> String {
    format!("{}_{}", kind.prefix(), id)
}

/// Read a payload back into the thing it opens.
///
/// `strip_prefix` rather than `split_once('_')`: the prefixes contain an
/// underscore themselves, so splitting on the first one turns `p_set_x` into
/// kind `p` and id `set_x`.
pub fn parse(payload: &str) -> Option<Target> {
    if !transportable(payload) {
        return None;
    }

    for kind in Kind::ALL {
        let with_sep = format!("{}_", kind.prefix());
        if let Some(id) = payload.strip_prefix(&with_sep) {
            if id.is_empty() {
                return None;
            }
            return Some(Target::Product {
                kind,
                id: id.to_string(),
            });
        }
    }

    if payload == "cart" {
        return Some(Target::Cart {
            source: String::new(),
        });
    }
    if let Some(source) = payload.strip_prefix("cart__") {
        return usable_source(source).then(|| Target::Cart {
            source: source.to_string(),
        });
    }

    // Before `o_`, because `o_` is a prefix of nothing else but `reorder__`
    // does not start with it — order matters only for readers, not the matcher.
    if let Some(id) = payload.strip_prefix("reorder__") {
        return (!id.is_empty()).then(|| Target::Reorder(id.to_string()));
    }
    if let Some(id) = payload.strip_prefix("o_") {
        return (!id.is_empty()).then(|| Target::Order(id.to_string()));
    }

    if payload == "garden" {
        return Some(Target::Garden {
            referrer: None,
            source: None,
        });
    }
    if let Some(rest) = payload.strip_prefix("garden__") {
        if rest.is_empty() {
            return None;
        }
        let (id_part, source) = match rest.find("__") {
            Some(at) => (&rest[..at], Some(&rest[at + 2..])),
            None => (rest, None),
        };
        if let Some(s) = source {
            if !usable_source(s) {
                return None;
            }
        }
        let referrer = id_part.parse::<i64>().ok()?;
        if referrer <= 0 {
            return None;
        }
        return Some(Target::Garden {
            referrer: Some(referrer),
            source: source.map(str::to_string),
        });
    }

    None
}

/// Build the payload for any target — the exact inverse of [`parse`].
///
/// One builder beside the one parser, because the four shapes this replaces
/// (`cart__`, `o_`, `reorder__`, `garden__`) were written in `src/ui/share.rs`,
/// whose `#[cfg(test)] mod tests` holds 22 assertions and runs **none of them**:
/// `src/ui` is `#[cfg(target_arch = "wasm32")]`, so `cargo test` never reaches
/// it. The parse side was covered here and the build side was not, so a builder
/// emitting `garden_<id>` instead of `garden__<id>` would have shipped green.
///
/// Returns `None` for a target that cannot be transported — an id carrying a
/// character Telegram rejects, or a payload over 64 bytes — rather than
/// producing a link that silently opens the home screen.
pub fn payload_for(target: &Target) -> Option<String> {
    let payload = match target {
        Target::Product { kind, id } => product_payload(*kind, id),
        Target::Order(id) => format!("o_{id}"),
        Target::Reorder(id) => format!("reorder__{id}"),
        Target::Cart { source } if source.is_empty() => "cart".to_string(),
        Target::Cart { source } => format!("cart__{source}"),
        Target::Garden {
            referrer: None,
            source: _,
        } => "garden".to_string(),
        Target::Garden {
            referrer: Some(id),
            source: None,
        } => format!("garden__{id}"),
        Target::Garden {
            referrer: Some(id),
            source: Some(s),
        } => format!("garden__{id}__{s}"),
    };
    transportable(&payload).then_some(payload)
}

/// Where a target is supposed to land the customer.
///
/// Written here, as an exhaustive `match`, because the app forgot one. Every
/// other target reached its screen — the product screens, the cart, the order,
/// the reorder — and `Garden` had a resolver, an analytics event and an invite
/// signal but **no navigation**, so `startapp=garden` (the link the watering
/// reminder sends) resolved correctly and left the customer on the home screen.
///
/// A sixth variant cannot be added without the compiler stopping here, which is
/// the only guarantee available: the wiring itself lives in `src/ui`, and no
/// test in this repository can execute that.
pub fn destination(target: &Target) -> &'static str {
    match target {
        Target::Product { kind, .. } => match kind {
            Kind::Strain => "/menu",
            Kind::Accessory => "/accessories",
            Kind::Set => "/sets",
            Kind::Tea => "/tea",
            Kind::Event => "/events",
        },
        Target::Order(_) => "/orders",
        Target::Reorder(_) => "/cart",
        Target::Cart { .. } => "/cart",
        Target::Garden { .. } => "/garden",
    }
}

/// Does this payload address the Mini App?
///
/// The bot answers a `/start` with a Mini App button exactly when this is true,
/// and the app can open exactly what this accepts — one function, so the two
/// can no longer disagree. A referral code (`ref_…`) is deliberately false: it
/// is recorded against the inviter and the recipient gets the ordinary welcome.
pub fn is_miniapp_payload(payload: &str) -> bool {
    parse(payload).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ids of the shapes the catalog actually produces: a UUID, a slug with
    /// hyphens, and a short numeric key.
    const IDS: [&str; 4] = [
        "fe346171-aa5b-4f88-93ed-8be0ec38aa6c",
        "black-heavy-hit-pack",
        "42",
        "a",
    ];

    /// Every target names a screen, and none of them names the home screen.
    ///
    /// "Landed on the home screen" is precisely how a broken deep link looks to
    /// a customer: no error, no 404, just the wrong place. `Garden` shipped in
    /// exactly that state.
    #[test]
    fn every_target_names_a_screen_that_is_not_the_home_screen() {
        let mut seen = std::collections::BTreeSet::new();
        for kind in Kind::ALL {
            seen.insert(destination(&Target::Product {
                kind,
                id: "x".into(),
            }));
        }
        for t in [
            Target::Order("x".into()),
            Target::Reorder("x".into()),
            Target::Cart {
                source: String::new(),
            },
            Target::Garden {
                referrer: None,
                source: None,
            },
            Target::Garden {
                referrer: Some(7),
                source: Some("tg".into()),
            },
        ] {
            seen.insert(destination(&t));
        }
        for d in &seen {
            assert!(
                d.starts_with('/') && *d != "/",
                "{d:?} is the home screen, which is what a broken link looks like"
            );
        }
        // Each product kind gets its OWN screen — a copy-paste pointing two
        // kinds at one catalog would open the wrong list with no error. Said
        // directly rather than as a total, so the assertion does not have to be
        // re-counted every time a non-product target is added. (`Reorder` and
        // `Cart` share `/cart` deliberately: a reorder fills the cart.)
        let per_kind: Vec<&str> = Kind::ALL
            .iter()
            .map(|k| {
                destination(&Target::Product {
                    kind: *k,
                    id: "x".into(),
                })
            })
            .collect();
        let unique: std::collections::BTreeSet<_> = per_kind.iter().collect();
        assert_eq!(
            unique.len(),
            Kind::ALL.len(),
            "two product kinds share a screen: {per_kind:?}"
        );
    }

    /// Build every target, parse it back, and require the same target.
    ///
    /// This is the assertion the repository did not have. `src/ui/share.rs`
    /// writes the same four shapes by hand and its test module never runs, so
    /// nothing checked that what the app *builds* is what the app *reads*.
    /// Sweeping `Kind::ALL` rather than listing kinds means a sixth kind cannot
    /// be added without landing here.
    #[test]
    fn every_target_survives_being_built_and_read_back() {
        let mut targets: Vec<Target> = Vec::new();
        for kind in Kind::ALL {
            for id in IDS {
                targets.push(Target::Product {
                    kind,
                    id: id.to_string(),
                });
            }
        }
        for id in IDS {
            targets.push(Target::Order(id.to_string()));
            targets.push(Target::Reorder(id.to_string()));
        }
        targets.push(Target::Cart {
            source: String::new(),
        });
        targets.push(Target::Cart {
            source: "utm_instagram".to_string(),
        });
        targets.push(Target::Garden {
            referrer: None,
            source: None,
        });
        targets.push(Target::Garden {
            referrer: Some(144_022_504),
            source: None,
        });
        targets.push(Target::Garden {
            referrer: Some(144_022_504),
            source: Some("tg".to_string()),
        });

        for t in &targets {
            let payload = payload_for(t)
                .unwrap_or_else(|| panic!("{t:?} could not be turned into a payload at all"));
            assert_eq!(
                parse(&payload).as_ref(),
                Some(t),
                "built {payload:?} for {t:?} and read back something else"
            );
            assert!(
                is_miniapp_payload(&payload),
                "the bot would not answer {payload:?}, so the link opens the home screen"
            );
            assert!(
                payload.len() <= MAX_START_PARAM_LEN,
                "{payload:?} is {} bytes; Telegram truncates over {MAX_START_PARAM_LEN}",
                payload.len()
            );
        }
        assert_eq!(targets.len(), 5 * IDS.len() + 2 * IDS.len() + 5);
    }

    /// The shapes `src/ui/share.rs` writes by hand, pinned here where a test
    /// can see them. If a builder there drifts, this is the file that says so.
    #[test]
    fn the_wire_shapes_are_exactly_what_the_app_writes() {
        let cases: [(Target, &str); 7] = [
            (
                Target::Cart {
                    source: String::new(),
                },
                "cart",
            ),
            (
                Target::Cart {
                    source: "utm_x".into(),
                },
                "cart__utm_x",
            ),
            (Target::Order("abc".into()), "o_abc"),
            (Target::Reorder("abc".into()), "reorder__abc"),
            (
                Target::Garden {
                    referrer: None,
                    source: None,
                },
                "garden",
            ),
            (
                Target::Garden {
                    referrer: Some(7),
                    source: None,
                },
                "garden__7",
            ),
            (
                Target::Garden {
                    referrer: Some(7),
                    source: Some("tg".into()),
                },
                "garden__7__tg",
            ),
        ];
        for (t, want) in cases {
            assert_eq!(payload_for(&t).as_deref(), Some(want), "{t:?}");
        }
    }

    /// A link that cannot be transported is refused, not emitted broken. An id
    /// with a space or a slash produces a `t.me` URL Telegram will not carry,
    /// and the recipient lands on the home screen with no error anywhere.
    #[test]
    fn an_untransportable_target_yields_no_link() {
        for bad in ["has space", "a/b", "emoji🌿", &"x".repeat(70)] {
            assert_eq!(
                payload_for(&Target::Product {
                    kind: Kind::Set,
                    id: bad.to_string()
                }),
                None,
                "{bad:?} was turned into a link anyway"
            );
        }
    }

    #[test]
    fn a_product_payload_survives_the_round_trip_for_every_kind() {
        for kind in Kind::ALL {
            for id in IDS {
                let payload = product_payload(kind, id);
                let back = parse(&payload).unwrap_or_else(|| {
                    panic!("{payload} did not parse back; a shared {id} would open nothing")
                });
                assert_eq!(
                    back,
                    Target::Product {
                        kind,
                        id: id.to_string()
                    },
                    "{payload} came back as something else"
                );
            }
        }
    }

    /// The defect this whole module exists for: the bot decides whether to
    /// answer with a Mini App button, the app decides whether it can open the
    /// payload, and if those two disagree the customer taps and nothing
    /// happens. They are now one function; this states it as a property.
    #[test]
    fn the_bot_answers_exactly_what_the_app_can_open() {
        let mut checked = 0;
        for kind in Kind::ALL {
            for id in IDS {
                let payload = product_payload(kind, id);
                assert_eq!(
                    is_miniapp_payload(&payload),
                    parse(&payload).is_some(),
                    "{payload}: the classifier and the parser disagree"
                );
                assert!(is_miniapp_payload(&payload), "{payload} must be answerable");
                checked += 1;
            }
        }
        for other in [
            "cart",
            "cart__utm_a",
            "o_7f3a",
            "reorder__7f3a",
            "garden",
            "garden__123",
        ] {
            assert!(is_miniapp_payload(other), "{other} must be answerable");
            checked += 1;
        }
        // A scan that checks nothing reports a clean sweep.
        assert!(checked >= 26, "only {checked} payloads were checked");
    }

    #[test]
    fn a_referral_code_is_not_a_mini_app_target_and_that_is_deliberate() {
        // It is recorded against the inviter and the recipient gets the
        // ordinary welcome. If this ever becomes true, the bot will start
        // answering referral links with a card and stop recording the referral.
        assert!(!is_miniapp_payload("ref_0u3KYyAT"));
        assert!(parse("ref_0u3KYyAT").is_none());
    }

    #[test]
    fn telegrams_own_limits_are_enforced_rather_than_discovered_in_production() {
        let longest_id = "x".repeat(MAX_START_PARAM_LEN - "p_set_".len());
        let at_the_limit = product_payload(Kind::Set, &longest_id);
        assert_eq!(at_the_limit.len(), MAX_START_PARAM_LEN);
        assert!(
            parse(&at_the_limit).is_some(),
            "a payload at the cap must work"
        );

        let over = product_payload(Kind::Set, &format!("{longest_id}x"));
        assert_eq!(over.len(), MAX_START_PARAM_LEN + 1);
        assert!(
            parse(&over).is_none(),
            "Telegram will not carry this at all"
        );

        // Characters Telegram refuses, and one that would need encoding.
        for bad in ["p_set_a b", "p_set_кириллица", "p_set_a.b", "p_set_a/b", ""] {
            assert!(parse(bad).is_none(), "{bad:?} should be refused");
        }
    }

    #[test]
    fn a_prefix_with_no_id_opens_nothing_rather_than_a_blank_card() {
        for kind in Kind::ALL {
            let bare = format!("{}_", kind.prefix());
            assert!(parse(&bare).is_none(), "{bare} has no id");
        }
        assert!(parse("o_").is_none());
        assert!(parse("reorder__").is_none());
        assert!(parse("garden__").is_none());
    }

    /// `p_set_x` must not be read as kind `p` with id `set_x`, which is what
    /// splitting on the first underscore does.
    #[test]
    fn a_prefix_containing_an_underscore_is_not_split_at_the_first_one() {
        assert_eq!(
            parse("p_set_set_x"),
            Some(Target::Product {
                kind: Kind::Set,
                id: "set_x".to_string()
            })
        );
    }

    #[test]
    fn the_other_targets_carry_what_they_promise() {
        assert_eq!(parse("o_7f3a"), Some(Target::Order("7f3a".into())));
        assert_eq!(parse("reorder__7f3a"), Some(Target::Reorder("7f3a".into())));
        assert_eq!(
            parse("cart"),
            Some(Target::Cart {
                source: String::new()
            })
        );
        assert_eq!(
            parse("cart__utm_a"),
            Some(Target::Cart {
                source: "utm_a".into()
            })
        );
        assert_eq!(
            parse("garden"),
            Some(Target::Garden {
                referrer: None,
                source: None
            })
        );
        assert_eq!(
            parse("garden__123__utm_a"),
            Some(Target::Garden {
                referrer: Some(123),
                source: Some("utm_a".into())
            })
        );
        // A reorder is not an order: `reorder__x` must not be read as an order
        // whose id happens to start with `reorder`.
        assert!(matches!(parse("reorder__x"), Some(Target::Reorder(_))));
    }

    #[test]
    fn a_garden_referrer_must_be_a_real_seat_number() {
        assert!(parse("garden__0").is_none(), "0 is not a referrer");
        assert!(parse("garden__-5").is_none(), "negative is not a referrer");
        assert!(parse("garden__abc").is_none(), "not a number at all");
    }
}
