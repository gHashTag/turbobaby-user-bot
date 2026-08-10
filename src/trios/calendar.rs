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

#[cfg(test)]
mod tests {
    use super::*;

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
