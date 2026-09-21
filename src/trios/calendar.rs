//! Month arithmetic for the events calendar.
//!
//! Lives here rather than next to the screen because `src/ui` is
//! `#[cfg(target_arch = "wasm32")]`-gated — tests written there never run on
//! the host target. Date maths is exactly the kind of code that needs tests
//! (month lengths, leap years, year boundaries), so it belongs on this side.

use chrono::{Datelike, NaiveDate};

/// First day of the month containing `date`.
pub fn first_of_month(date: NaiveDate) -> NaiveDate {
    NaiveDate::from_ymd_opt(date.year(), date.month(), 1).unwrap_or(date)
}

/// Number of days in the month containing `date`.
///
/// Derived by stepping to the first of the next month and back one day, so
/// leap years need no special case.
pub fn days_in_month(date: NaiveDate) -> u32 {
    let (next_year, next_month) = if date.month() == 12 {
        (date.year() + 1, 1)
    } else {
        (date.year(), date.month() + 1)
    };
    NaiveDate::from_ymd_opt(next_year, next_month, 1)
        .and_then(|d| d.pred_opt())
        .map(|d| d.day())
        .unwrap_or(28)
}

/// Every day of the month containing `date`, in order.
pub fn month_days(date: NaiveDate) -> Vec<NaiveDate> {
    let first = first_of_month(date);
    (0..days_in_month(date))
        .filter_map(|i| first.with_day(i + 1))
        .collect()
}

/// Move `date` by whole months, landing on the first of the target month.
///
/// Clamping to the first sidesteps the classic "31 January + 1 month" trap:
/// the picker only ever needs the month, and a silent shift to 3 March would
/// skip February entirely.
pub fn shift_month(date: NaiveDate, delta: i32) -> NaiveDate {
    let zero_based = date.year() * 12 + (date.month() as i32 - 1) + delta;
    let year = zero_based.div_euclid(12);
    let month = zero_based.rem_euclid(12) as u32 + 1;
    NaiveDate::from_ymd_opt(year, month, 1).unwrap_or(date)
}

/// Days of `anchor`'s month that are worth showing, given today's date.
///
/// The picker exists to schedule things, so days already past are dead space:
/// on the 10th, a strip starting at the 1st wastes a third of its width before
/// the first tappable day. In the current month it starts at `today`; in any
/// other month the whole month is shown, so stepping back to review an earlier
/// month still works.
pub fn visible_month_days(anchor: NaiveDate, today: NaiveDate) -> Vec<NaiveDate> {
    month_days(anchor)
        .into_iter()
        .filter(|d| first_of_month(*d) != first_of_month(today) || *d >= today)
        .collect()
}

/// Turn an `<input type="datetime-local">` value into an RFC 3339 timestamp.
///
/// The admin form used to build this by hand — `format!("{value}:00+07:00")` —
/// which assumes the widget always yields `YYYY-MM-DDTHH:MM`. The HTML spec
/// does not promise that: a value whose seconds are non-zero (or an input with
/// `step` under 60) is `YYYY-MM-DDTHH:MM:SS`, and iOS pickers do emit it.
/// Concatenating then produced `…T19:30:00:00+07:00`, which the server rejects
/// with a 400 that logged nothing and surfaced as a bare "Ошибка сохранения".
///
/// Both shapes are accepted here, and the result is parsed back before being
/// returned, so a malformed value fails as `None` at the call site instead of
/// travelling to the server as garbage. `offset` is the suffix to attach
/// (e.g. `"+07:00"` for Asia/Bangkok, `"Z"` for UTC).
pub fn datetime_local_to_rfc3339(value: &str, offset: &str) -> Option<String> {
    let v = value.trim();
    if v.is_empty() {
        return None;
    }
    // Count the colons in the time part to tell HH:MM from HH:MM:SS.
    let time_part = v.split('T').nth(1)?;
    let candidate = match time_part.matches(':').count() {
        1 => format!("{v}:00{offset}"),
        2 => format!("{v}{offset}"),
        _ => return None,
    };
    // Only hand back something that actually parses as the instant we claim.
    candidate
        .parse::<chrono::DateTime<chrono::FixedOffset>>()
        .ok()
        .map(|_| candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn datetime_local_without_seconds_is_padded() {
        assert_eq!(
            datetime_local_to_rfc3339("2026-09-05T19:30", "+07:00").as_deref(),
            Some("2026-09-05T19:30:00+07:00")
        );
    }

    /// The regression: an iOS picker returning seconds used to be concatenated
    /// into `…T19:30:00:00+07:00` and rejected by the server.
    #[test]
    fn datetime_local_with_seconds_is_not_double_suffixed() {
        assert_eq!(
            datetime_local_to_rfc3339("2026-09-05T19:30:00", "+07:00").as_deref(),
            Some("2026-09-05T19:30:00+07:00")
        );
        assert_eq!(
            datetime_local_to_rfc3339("2026-09-05T19:30:45", "+07:00").as_deref(),
            Some("2026-09-05T19:30:45+07:00")
        );
    }

    #[test]
    fn utc_offset_suffix_is_honoured() {
        assert_eq!(
            datetime_local_to_rfc3339("2026-09-05T19:30", "Z").as_deref(),
            Some("2026-09-05T19:30:00Z")
        );
    }

    #[test]
    fn empty_and_malformed_values_are_rejected_not_forwarded() {
        assert_eq!(datetime_local_to_rfc3339("", "+07:00"), None);
        assert_eq!(datetime_local_to_rfc3339("   ", "+07:00"), None);
        assert_eq!(datetime_local_to_rfc3339("2026-09-05", "+07:00"), None);
        assert_eq!(datetime_local_to_rfc3339("not a date", "+07:00"), None);
        // Shape is right, instant is not — must not reach the server.
        assert_eq!(
            datetime_local_to_rfc3339("2026-13-45T99:99", "+07:00"),
            None
        );
    }

    /// Whatever comes back must be parseable by the same parser the server
    /// uses (`str::parse::<DateTime<Utc>>` in `api::events`).
    #[test]
    fn output_parses_the_way_the_server_parses_it() {
        for raw in [
            "2026-09-05T19:30",
            "2026-09-05T19:30:00",
            "2026-01-01T00:00",
        ] {
            let s = datetime_local_to_rfc3339(raw, "+07:00").expect("valid");
            assert!(
                s.parse::<chrono::DateTime<chrono::Utc>>().is_ok(),
                "server would reject {s} (from {raw})"
            );
        }
    }

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).expect("valid date")
    }

    #[test]
    fn first_of_month_snaps_to_day_one() {
        assert_eq!(first_of_month(d(2026, 8, 9)), d(2026, 8, 1));
        assert_eq!(first_of_month(d(2026, 8, 1)), d(2026, 8, 1));
        assert_eq!(first_of_month(d(2026, 12, 31)), d(2026, 12, 1));
    }

    #[test]
    fn days_in_month_covers_every_month_length() {
        for (month, expected) in [
            (1, 31),
            (3, 31),
            (4, 30),
            (6, 30),
            (9, 30),
            (11, 30),
            (12, 31),
        ] {
            assert_eq!(
                days_in_month(d(2026, month, 1)),
                expected,
                "month {month} length"
            );
        }
    }

    #[test]
    fn days_in_month_handles_february_and_leap_years() {
        assert_eq!(days_in_month(d(2026, 2, 1)), 28, "2026 is not a leap year");
        assert_eq!(days_in_month(d(2028, 2, 1)), 29, "2028 is a leap year");
        // Centuries: divisible by 100 but not 400 is NOT a leap year.
        assert_eq!(days_in_month(d(1900, 2, 1)), 28);
        assert_eq!(days_in_month(d(2000, 2, 1)), 29);
    }

    #[test]
    fn month_days_returns_the_whole_month_in_order() {
        let days = month_days(d(2026, 8, 9));
        assert_eq!(days.len(), 31, "August has 31 days");
        assert_eq!(days[0], d(2026, 8, 1), "must start at the 1st");
        assert_eq!(
            *days.last().expect("non-empty"),
            d(2026, 8, 31),
            "must reach the last day — this is what 'fill to the end of the \
             month' needs and the old 7-day strip could not show"
        );
        assert!(
            days.windows(2).all(|w| w[1] == w[0].succ_opt().unwrap()),
            "days must be consecutive with no gaps"
        );
    }

    #[test]
    fn month_days_is_correct_for_a_short_month() {
        let days = month_days(d(2026, 2, 14));
        assert_eq!(days.len(), 28);
        assert_eq!(*days.last().expect("non-empty"), d(2026, 2, 28));
    }

    #[test]
    fn the_current_month_starts_at_today_not_the_first() {
        // Dead space: on the 10th a strip beginning at the 1st burns a third
        // of its width on days nothing can be scheduled into.
        let today = d(2026, 8, 10);
        let days = visible_month_days(today, today);
        assert_eq!(days[0], today, "must start at today");
        assert_eq!(*days.last().expect("non-empty"), d(2026, 8, 31));
        assert_eq!(days.len(), 22, "10th..31st inclusive");
    }

    #[test]
    fn today_itself_is_still_selectable() {
        let today = d(2026, 8, 10);
        assert!(
            visible_month_days(today, today).contains(&today),
            "an event can be added for today"
        );
    }

    #[test]
    fn a_future_month_shows_from_its_first_day() {
        let today = d(2026, 8, 10);
        let days = visible_month_days(d(2026, 9, 1), today);
        assert_eq!(days[0], d(2026, 9, 1));
        assert_eq!(days.len(), 30);
    }

    #[test]
    fn a_past_month_still_shows_in_full() {
        // Stepping back to review what already happened must not show an
        // empty strip.
        let today = d(2026, 8, 10);
        let days = visible_month_days(d(2026, 7, 1), today);
        assert_eq!(days.len(), 31, "July is fully visible when looking back");
        assert_eq!(days[0], d(2026, 7, 1));
    }

    #[test]
    fn the_last_day_of_a_month_leaves_exactly_one_day() {
        let today = d(2026, 8, 31);
        assert_eq!(visible_month_days(today, today), vec![today]);
    }

    #[test]
    fn the_same_day_number_in_another_year_does_not_truncate() {
        // Filtering on day-of-month alone would wrongly trim August 2027.
        let today = d(2026, 8, 10);
        let days = visible_month_days(d(2027, 8, 1), today);
        assert_eq!(days.len(), 31, "a different year is not the current month");
    }

    #[test]
    fn shift_month_moves_forward_and_back() {
        assert_eq!(shift_month(d(2026, 8, 9), 1), d(2026, 9, 1));
        assert_eq!(shift_month(d(2026, 8, 9), -1), d(2026, 7, 1));
        assert_eq!(shift_month(d(2026, 8, 9), 0), d(2026, 8, 1));
    }

    #[test]
    fn shift_month_crosses_year_boundaries() {
        assert_eq!(shift_month(d(2026, 12, 15), 1), d(2027, 1, 1));
        assert_eq!(shift_month(d(2026, 1, 15), -1), d(2025, 12, 1));
        assert_eq!(shift_month(d(2026, 1, 1), -13), d(2024, 12, 1));
        assert_eq!(shift_month(d(2026, 12, 1), 13), d(2028, 1, 1));
    }

    #[test]
    fn shift_month_never_skips_a_month_from_a_long_month_end() {
        // Naive "add 30 days" arithmetic from 31 January lands in March.
        assert_eq!(shift_month(d(2026, 1, 31), 1), d(2026, 2, 1));
        assert_eq!(shift_month(d(2026, 3, 31), -1), d(2026, 2, 1));
    }

    #[test]
    fn shifting_forward_then_back_returns_to_the_same_month() {
        for delta in 1..=24 {
            let start = d(2026, 8, 1);
            assert_eq!(
                shift_month(shift_month(start, delta), -delta),
                start,
                "round trip failed for delta {delta}"
            );
        }
    }
}

/// The time a single event occupies, as one label.
///
/// The day list printed only the start — `🕒 09:00` — for every event on a
/// date. With one event per day that reads as "the event is at nine". With
/// four, a customer choosing between 09:00, 15:00, 18:00 and 21:00 cannot tell
/// which of them is still running when they arrive, and the end time was
/// recorded, stored and sent the whole time.
///
/// `None` for the end is a real state, not a defect: `ends_at` is optional in
/// the schema and the admin form labels it «Окончание (необязательно)». An
/// event with no end prints its start alone rather than an empty dash.
///
/// An en dash with thin spaces, because `09:00-12:00` on a phone reads as one
/// token and wraps badly.
pub fn time_range(start: &str, end: Option<&str>) -> String {
    match end {
        Some(e) if !e.trim().is_empty() && e != start => format!("{start} – {e}"),
        _ => start.to_string(),
    }
}

/// How many events a day holds, as the calendar marks it.
///
/// The day strip gave no sign which dates had anything at all: every chip was
/// a weekday and a number, so finding an event meant tapping all thirty-one.
/// That is also why one date looked like it could only hold one event — there
/// was nothing on the strip that could have said otherwise.
///
/// Zero is its own case and gets no marker: a dot on every day would say as
/// little as a dot on none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DayMark {
    /// Nothing scheduled.
    Empty,
    /// Exactly one event — a dot is enough.
    One,
    /// Several; the count is worth printing, because it is the thing the
    /// customer cannot otherwise discover without opening the day.
    Several(usize),
}

pub fn day_mark(count: usize) -> DayMark {
    match count {
        0 => DayMark::Empty,
        1 => DayMark::One,
        n => DayMark::Several(n),
    }
}

/// Seats still free on an event with a declared capacity.
///
/// One implementation, shared by the two ends that used to disagree (D15).
/// The API clamped at zero — `(cap - seats_taken).max(0)`, the
/// `seats_available` it publishes — and the calendar card recomputed the same
/// quantity with `i32::saturating_sub`, which saturates at `i32::MIN` and not
/// at zero. An event can be oversubscribed: `validate_update_request`
/// (`src/api/events.rs:298`) accepts any `max_seats` in `0..=1_000_000` and
/// never compares it against the seats already sold, so lowering the capacity
/// under the guest list is one PUT away. The card then printed a negative seat
/// count for an event the API was reporting as full.
///
/// `taken` is `i64` because it arrives as `SUM(seats)` off the bookings
/// table, and the subtraction is done in `i64` so a capacity near the `i32`
/// edge cannot wrap into a large positive count before the clamp.
pub fn seats_free(cap: i32, taken: i64) -> i32 {
    (i64::from(cap) - taken).clamp(0, i64::from(i32::MAX)) as i32
}

#[cfg(test)]
mod schedule_tests {
    use super::*;

    #[test]
    fn a_range_needs_both_ends() {
        assert_eq!(time_range("09:00", Some("12:00")), "09:00 – 12:00");
        assert_eq!(time_range("09:00", None), "09:00");
        // An optional end left blank in the admin form arrives as an empty
        // string, not as `None`.
        assert_eq!(time_range("09:00", Some("")), "09:00");
        assert_eq!(time_range("09:00", Some("   ")), "09:00");
        // An end equal to the start is not a range worth printing.
        assert_eq!(time_range("09:00", Some("09:00")), "09:00");
    }

    /// The four events from the report, each keeping its own hours.
    #[test]
    fn a_days_schedule_reads_as_distinct_slots() {
        let day = [
            ("09:00", Some("12:00")),
            ("15:00", Some("17:00")),
            ("18:00", Some("19:30")),
            ("21:00", None),
        ];
        let labels: Vec<String> = day.iter().map(|(s, e)| time_range(s, *e)).collect();
        assert_eq!(
            labels,
            vec!["09:00 – 12:00", "15:00 – 17:00", "18:00 – 19:30", "21:00"]
        );
        let unique: std::collections::BTreeSet<&String> = labels.iter().collect();
        assert_eq!(
            unique.len(),
            labels.len(),
            "two slots printed the same: {labels:?}"
        );
    }

    #[test]
    fn only_a_day_with_something_is_marked() {
        assert_eq!(day_mark(0), DayMark::Empty);
        assert_eq!(day_mark(1), DayMark::One);
        assert_eq!(day_mark(2), DayMark::Several(2));
        assert_eq!(day_mark(9), DayMark::Several(9));
    }

    /// Both ends, spelled out as the two expressions that used to disagree.
    ///
    /// The three easy rows are not evidence and this test said only those
    /// until 2026-09-21: `saturating_sub`, the expression the card carried,
    /// returns 10, 7 and 0 for them as well, so the test passed with the fix
    /// reverted. The row that tells the two apart is the oversubscribed one --
    /// the only case where an `i32` saturation floor (`i32::MIN`) and a clamp
    /// at zero can differ at all -- and it is asserted here rather than left
    /// to the neighbouring test, because agreement is this test's own claim.
    #[test]
    fn free_seats_are_counted_the_same_at_both_ends() {
        // The API's published rule: `seats_available`, clamped at zero.
        let api = |cap: i32, taken: i64| (i64::from(cap) - taken).max(0);
        // The card's rule before D15: `cap.saturating_sub(taken as i32)`.
        let card_before_d15 = |cap: i32, taken: i64| i64::from(cap.saturating_sub(taken as i32));

        for (cap, taken) in [(10i32, 0i64), (10, 3), (10, 10)] {
            assert_eq!(i64::from(seats_free(cap, taken)), api(cap, taken));
            assert_eq!(
                card_before_d15(cap, taken),
                api(cap, taken),
                "cap={cap} taken={taken} agrees under both rules, so it cannot \
                 tell them apart"
            );
        }

        // 8 seats sold against a capacity an admin lowered to 5. The API
        // published 0 and the card printed -3 for the same row.
        assert_eq!(i64::from(seats_free(5, 8)), api(5, 8));
        assert_eq!(seats_free(5, 8), 0);
        assert_ne!(
            card_before_d15(5, 8),
            api(5, 8),
            "the two ends now agree on the oversubscribed row as well, so this \
             test has stopped being able to fail"
        );
    }

    /// The case the card and the API disagreed on. An admin lowers `max_seats`
    /// to 5 on an event that already sold 8 seats: the API publishes
    /// `seats_available: 0` and the card printed the seats-left label with -3 in it.
    #[test]
    fn an_oversubscribed_event_has_no_free_seats_rather_than_negative_ones() {
        assert_eq!(seats_free(5, 8), 0, "a full event has 0 free seats, not -3");
        assert_eq!(seats_free(0, 5), 0, "capacity lowered to zero is still 0");
        assert_eq!(seats_free(1, 1_000_000), 0);
    }

    /// `taken` is a `SUM` and arrives as `i64`. Narrowing it to `i32` before
    /// the subtraction turns a huge number of seats into a negative one, and a
    /// negative subtrahend into a seat count of `i32::MAX` — "free seats" on
    /// an event nobody can book.
    #[test]
    fn a_seat_count_past_the_i32_edge_does_not_wrap_into_free_seats() {
        assert_eq!(seats_free(10, i64::from(i32::MAX) + 11), 0);
        assert_eq!(seats_free(i32::MAX, 0), i32::MAX);
    }
}
