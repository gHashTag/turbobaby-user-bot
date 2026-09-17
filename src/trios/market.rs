//! The deployment market's clock, as a declared profile rather than an ambient
//! fact (D18).
//!
//! [`MarketMoneyFormat`](crate::trios::pricing::MarketMoneyFormat) already gives
//! the money half of the profile a Rust home. The clock had none: measured
//! 2026-09-14, seven sites across seven files each restated "the shop is at
//! UTC+7" in its own words, and no file said *market*. `api/orders.rs` documents
//! the duplication outright — "which is also how `trios::promo` and
//! `trios::happy_hour` read the shop's clock" — which is the shape D15 removed
//! for price arithmetic and D18 extends to the market.
//!
//! D18 records five of those seven. The two it missed (`api/events.rs` and
//! `ui/screens/admin_screen.rs`) are exactly what an unwired constant costs:
//! nobody can enumerate a rule that is spelled a different way in every file.
//!
//! # The assumption none of the seven stated
//!
//! Every one of them reduces the market's clock to a [`FixedOffset`]. That is
//! correct for Thailand and wrong in general, and *the difference was invisible*
//! because no value in the program recorded which case it was in. This module
//! makes it a field. `dst_observed` comes from the same `market` block in
//! `data/fleet_seed.json` that `scripts/verify_fleet_seed.py` pins against
//! `specs/turbobaby/market_profile.t27`, whose proof profile is deliberately
//! DST-true so a contract shaped like one market cannot pass as a contract for
//! all of them.
//!
//! So [`MarketClock::fixed_offset`] returns `None` for a DST market instead of
//! quietly picking one of its two offsets. Half the year that guess is an hour
//! wrong, and an hour-wrong pickup time is invention in the same sense D11
//! forbids for a price: a number with no authority behind it, presented as
//! though it had one. `None` is the refusal; every call site already had a
//! non-panicking arm for it, because `FixedOffset::east_opt` is fallible too.

use chrono::{DateTime, FixedOffset, Utc};

/// The clock half of the market profile (D18). Field names mirror the `market`
/// block in `data/fleet_seed.json` and the `TZ_*` consts in
/// `specs/turbobaby/market_profile.t27`.
///
/// A second market is a second instance of this struct, not a second way of
/// spelling `7 * 3600`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarketClock {
    /// IANA zone name, e.g. `Asia/Bangkok`. Carried for provenance and for
    /// anything that must *name* the zone; the arithmetic uses the offset.
    pub timezone_name: &'static str,
    /// Standard-time offset from UTC in whole hours. Whole hours because the
    /// seed carries `utc_offset_hours`; a market on a :30 or :45 offset (India,
    /// Nepal, Chatham) needs the contract widened before it can be served here,
    /// which is a decision to record rather than a rounding to perform.
    pub utc_offset_hours: i32,
    /// Whether the market moves its clocks. `true` makes [`Self::fixed_offset`]
    /// refuse — see the module docs.
    pub dst_observed: bool,
}

/// The deployment's clock, mirroring the `market` block in
/// `data/fleet_seed.json`: `Asia/Bangkok`, UTC+7, no DST.
///
/// Thailand last observed DST in 1976, which is why every site this replaces
/// could get away with a fixed offset. The fact is now written down once and
/// carried, instead of being re-asserted in seven comments.
pub const TH_CLOCK: MarketClock = MarketClock {
    timezone_name: "Asia/Bangkok",
    utc_offset_hours: 7,
    dst_observed: false,
};

/// The clock this deployment runs on. Call sites name `MARKET`, never a
/// country: swapping the deployment to another market is an edit here and in
/// the seed, not a sweep through `src/`.
pub const MARKET: MarketClock = TH_CLOCK;

/// The dialling half of the market profile (D18).
///
/// Separate from [`MarketClock`] because it is not a clock fact, and separate
/// from `MarketMoneyFormat` because it is not a money fact. Three structs
/// rather than one bag: a call site that needs the calling code should not
/// have to name a type that also carries the DST flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarketDialing {
    /// E.164 country calling code, written the way the seed writes it —
    /// `+66`, with the plus. Carrying it without the plus would make this
    /// value a fragment that only one caller's `format!` knows how to
    /// reassemble, and the seed is the authority on the spelling.
    pub default_calling_code: &'static str,
}

/// Thailand's dialling profile, mirroring `default_calling_code` in the
/// `market` block of `data/fleet_seed.json`.
pub const TH_DIALING: MarketDialing = MarketDialing {
    default_calling_code: "+66",
};

/// The dialling profile this deployment runs on.
///
/// `src/trios/store.rs` held this as `DEFAULT_COUNTRY_CODE = "66"` under a
/// comment that placed the shop on Koh Phangan — an island the shop has never
/// been on; it is in Kamala, Phuket. The value was right and the reason given
/// for it was wrong, which is the worse of the two failures: the next person
/// to ask "is 66 still correct?" would have checked a false premise.
pub const MARKET_DIALING: MarketDialing = TH_DIALING;

impl MarketClock {
    /// The standard-time offset in seconds east of UTC.
    ///
    /// `saturating_mul` rather than `*`: the multiply is the only arithmetic in
    /// this module that can overflow, and a profile is data — the seed is
    /// verified, but this type is public and a caller may construct one. A
    /// saturated offset is then rejected by `east_opt`'s ±24h bound below,
    /// which turns a nonsense profile into a refusal instead of a panic in
    /// release or a wrapped, plausible-looking offset.
    pub const fn offset_secs(&self) -> i32 {
        self.utc_offset_hours.saturating_mul(3600)
    }

    /// The market as a fixed offset, or `None` when it cannot honestly be
    /// reduced to one.
    ///
    /// Two ways to get `None`, and they are different in kind:
    /// - `dst_observed` — a refusal. The market has two offsets and this type
    ///   holds one; see the module docs.
    /// - an out-of-range offset — a malformed profile, caught by `east_opt`.
    ///
    /// Both are `None` because both mean the same thing to a caller: this
    /// program cannot say what the local time is, so it must not print one.
    pub fn fixed_offset(&self) -> Option<FixedOffset> {
        if self.dst_observed {
            return None;
        }
        FixedOffset::east_opt(self.offset_secs())
    }

    /// A UTC instant as seen on the market's wall clock.
    ///
    /// This is the honest replacement for `utc + Duration::hours(7)`, which two
    /// sites used. That trick yields the right digits from `.hour()` and
    /// `.format("%H:%M")` — and a value still *typed* `DateTime<Utc>` while
    /// holding Bangkok wall-clock time. It prints `+0000` under `%z`, and any
    /// later code that treats it as a real instant is seven hours out. The
    /// digits being right today is what kept it alive.
    pub fn at(&self, utc: DateTime<Utc>) -> Option<DateTime<FixedOffset>> {
        self.fixed_offset().map(|tz| utc.with_timezone(&tz))
    }

    /// Now, on the market's wall clock.
    pub fn now(&self) -> Option<DateTime<FixedOffset>> {
        self.at(Utc::now())
    }

    /// The market's numeric offset in RFC 3339 form, e.g. `+07:00`.
    ///
    /// Built from the profile rather than written as a literal, so the one
    /// remaining `"+07:00"` string in the tree (the admin event form) moves
    /// with the market like everything else. Returns `None` on the same terms
    /// as [`Self::fixed_offset`]: a DST market has no single such string.
    pub fn rfc3339_offset(&self) -> Option<String> {
        if self.dst_observed {
            return None;
        }
        let secs = self.offset_secs();
        // `east_opt` bounds the offset to ±24h; reuse it rather than re-deriving
        // the range, so the two functions cannot disagree about what is valid.
        FixedOffset::east_opt(secs)?;
        let sign = if secs < 0 { '-' } else { '+' };
        let abs = secs.unsigned_abs();
        Some(format!("{sign}{:02}:{:02}", abs / 3600, (abs % 3600) / 60))
    }

    /// [`Self::rfc3339_offset`] with UTC as the fallback, for the one caller
    /// that must produce a timestamp rather than decline to.
    ///
    /// The fallback lives here rather than at the call site on purpose. Written
    /// as `.unwrap_or_else(|| "+00:00".to_string())` in the admin screen it was
    /// an offset literal outside this module — which is exactly what
    /// `tests/market_clock_wiring.rs` forbids, and the guard caught it. A
    /// default is a policy about the market, so it belongs with the market.
    ///
    /// Stamping the admin's entry as UTC is wrong in a way the timestamp
    /// declares: the server reads `+00:00` and stores the instant that string
    /// actually denotes. Guessing one of a DST market's two offsets would be
    /// wrong while looking right, which is the failure mode D11 exists to
    /// refuse.
    pub fn rfc3339_offset_or_utc(&self) -> String {
        self.rfc3339_offset()
            .unwrap_or_else(|| String::from("+00:00"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The profile is internally consistent — the derived values agree with
    /// the declared fields.
    ///
    /// This used to also assert the declared fields themselves, against
    /// hand-typed literals, under a doc comment claiming they were "pinned to
    /// the seed's `market` block". Nothing read the seed. Editing
    /// `data/fleet_seed.json` left these four `assert_eq!`s green on the old
    /// numbers, which is the restated-list defect wearing the costume of the
    /// guard against it. The real pin is
    /// `tests/market_clock_wiring.rs::the_rust_market_profile_matches_the_seed`,
    /// which reads the JSON; what is left here is arithmetic, which the seed
    /// has no opinion about.
    #[test]
    fn the_deployment_clock_is_internally_consistent() {
        assert_eq!(
            MARKET.offset_secs(),
            MARKET.utc_offset_hours * 3600,
            "offset_secs() must be the declared hours, not a second literal"
        );
        assert_eq!(
            MARKET.rfc3339_offset().as_deref(),
            Some(format!("+{:02}:00", MARKET.utc_offset_hours).as_str()),
            "rfc3339_offset() must be derived from the declared offset"
        );
    }

    /// The behaviour the seven replaced sites had, restated once.
    #[test]
    fn a_utc_instant_reads_as_local_wall_clock() {
        let utc = "2026-09-14T17:30:00Z"
            .parse::<DateTime<Utc>>()
            .expect("a literal RFC 3339 instant parses");
        let local = MARKET.at(utc).expect("Thailand has a fixed offset");
        // 17:30 UTC is 00:30 the next day in Bangkok — the date rolls, which is
        // the case a naive `.hour()` comparison would miss.
        assert_eq!(
            local.format("%d.%m.%Y %H:%M").to_string(),
            "15.09.2026 00:30"
        );
        // And unlike `utc + Duration::hours(7)`, it says so under `%z`.
        assert_eq!(local.format("%z").to_string(), "+0700");
    }

    /// The distinction the old code could not express. A market that moves its
    /// clocks has two offsets; this type holds one, so it declines.
    #[test]
    fn a_dst_market_has_no_single_offset_and_says_so() {
        let berlin = MarketClock {
            timezone_name: "Europe/Berlin",
            utc_offset_hours: 1,
            dst_observed: true,
        };
        assert_eq!(berlin.fixed_offset(), None);
        assert_eq!(berlin.rfc3339_offset(), None);
        assert_eq!(berlin.at(Utc::now()), None);
        assert_eq!(berlin.now(), None);
        // The refusal is about DST, not about the offset being unusable: the
        // same offset serves fine once the market stops claiming to move it.
        let without_dst = MarketClock {
            dst_observed: false,
            ..berlin
        };
        assert_eq!(without_dst.rfc3339_offset().as_deref(), Some("+01:00"));
        // The caller that cannot decline gets UTC, not one of the two guesses.
        assert_eq!(berlin.rfc3339_offset_or_utc(), "+00:00");
        assert_eq!(MARKET.rfc3339_offset_or_utc(), "+07:00");
    }

    /// A market west of Greenwich, to prove the sign is handled rather than
    /// assumed away by a deployment that happens to sit east of it.
    #[test]
    fn a_western_market_keeps_its_sign() {
        let lima = MarketClock {
            timezone_name: "America/Lima",
            utc_offset_hours: -5,
            dst_observed: false,
        };
        assert_eq!(lima.offset_secs(), -5 * 3600);
        assert_eq!(lima.rfc3339_offset().as_deref(), Some("-05:00"));
        let utc = "2026-09-14T02:00:00Z"
            .parse::<DateTime<Utc>>()
            .expect("a literal RFC 3339 instant parses");
        assert_eq!(
            lima.at(utc)
                .expect("Peru has a fixed offset")
                .format("%d.%m %H:%M")
                .to_string(),
            "13.09 21:00"
        );
    }

    /// A malformed profile is refused, not wrapped into a plausible offset.
    /// `east_opt` bounds the offset to ±24h and the saturating multiply keeps
    /// the overflow from becoming a small, believable number.
    #[test]
    fn an_impossible_profile_is_refused() {
        let nonsense = MarketClock {
            timezone_name: "Nowhere/Nowhere",
            utc_offset_hours: i32::MAX,
            dst_observed: false,
        };
        assert_eq!(nonsense.offset_secs(), i32::MAX);
        assert_eq!(nonsense.fixed_offset(), None);
        assert_eq!(nonsense.rfc3339_offset(), None);
    }
}
