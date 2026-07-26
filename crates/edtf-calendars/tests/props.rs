//! Generative invariants (issue #21's test ceremony): round-trip identity
//! in both directions, offset monotonicity, and span lengths that match
//! the Julian calendar's own day counts.
//!
//! The Gregorian JDN formula below is a test-local oracle, implemented
//! independently of the crate (same spirit as `edtf-core/tests/props.rs`
//! re-deriving the leap rule): day counting happens on the Gregorian day
//! scale only, so a bug in the crate's Julian arithmetic cannot cancel
//! itself out.

use edtf_calendars::{
    convert, gregorian_to_julian, is_julian_leap, julian_to_gregorian, Converted, JulianDate,
};
use edtf_core::BoundDate;
use proptest::prelude::*;

fn julian_last_day(month: u8, leap: bool) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        _ => unreachable!("month is 1-12"),
    }
}

/// Test-local oracle: JDN of a proleptic Gregorian date (floor division).
fn gregorian_jdn(d: BoundDate) -> i128 {
    let (y, m, dd) = (i128::from(d.year), i128::from(d.month), i128::from(d.day));
    let a = (14 - m).div_euclid(12);
    let y = y + 4800 - a;
    let m = m + 12 * a - 3;
    dd + (153 * m + 2).div_euclid(5) + 365 * y + y.div_euclid(4) - y.div_euclid(100)
        + y.div_euclid(400)
        - 32045
}

/// The written-date offset: how many days later the Julian nominal date
/// `y-m-d` falls than the Gregorian nominal date `y-m-d`.
fn offset(y: i64, m: u8, d: u8) -> i128 {
    let g = julian_to_gregorian(JulianDate {
        year: y,
        month: m,
        day: d,
    })
    .expect("valid by construction");
    gregorian_jdn(g)
        - gregorian_jdn(BoundDate {
            year: y,
            month: m,
            day: d,
        })
}

/// A valid proleptic Julian date across a wide year range.
fn julian_date() -> impl Strategy<Value = JulianDate> {
    (-1_000_000i64..=1_000_000, 1u8..=12).prop_flat_map(|(year, month)| {
        (1u8..=julian_last_day(month, is_julian_leap(year))).prop_map(move |day| JulianDate {
            year,
            month,
            day,
        })
    })
}

fn is_gregorian_leap(y: i64) -> bool {
    y.rem_euclid(4) == 0 && (y.rem_euclid(100) != 0 || y.rem_euclid(400) == 0)
}

/// A valid proleptic Gregorian date across a wide year range.
fn gregorian_date() -> impl Strategy<Value = BoundDate> {
    (-1_000_000i64..=1_000_000, 1u8..=12).prop_flat_map(|(year, month)| {
        (1u8..=julian_last_day(month, is_gregorian_leap(year))).prop_map(move |day| BoundDate {
            year,
            month,
            day,
        })
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    /// Julian → Gregorian → Julian is the identity.
    #[test]
    fn round_trips_from_julian(d in julian_date()) {
        let g = julian_to_gregorian(d).expect("valid Julian date converts");
        prop_assert_eq!(gregorian_to_julian(g), Ok(d), "via {}", g);
    }

    /// Gregorian → Julian → Gregorian is the identity.
    #[test]
    fn round_trips_from_gregorian(d in gregorian_date()) {
        let j = gregorian_to_julian(d).expect("valid Gregorian date converts");
        prop_assert_eq!(julian_to_gregorian(j), Ok(d), "via {}", j);
    }

    /// The Julian–Gregorian offset never shrinks as time advances: for the
    /// same nominal month/day, a later year's offset is >= an earlier
    /// year's (day <= 28 so the nominal date exists in every year).
    #[test]
    fn offset_is_monotonic(
        y1 in -100_000i64..=100_000, span in 0i64..=100_000,
        m in 1u8..=12, d in 1u8..=28
    ) {
        let y2 = y1 + span;
        prop_assert!(offset(y1, m, d) <= offset(y2, m, d));
    }

    /// A year span covers exactly the Julian year's own day count, and its
    /// endpoints are the converted first and last days.
    #[test]
    fn year_span_has_julian_length(y in -100_000i64..=100_000) {
        let Ok(Converted::Span { earliest, latest }) = convert(y, None, None) else {
            return Err(TestCaseError::fail("year must convert to a span"));
        };
        let days = gregorian_jdn(latest) - gregorian_jdn(earliest) + 1;
        let expected = if is_julian_leap(y) { 366 } else { 365 };
        prop_assert_eq!(days, expected, "Julian {} spans {}..{}", y, earliest, latest);
    }

    /// A month span covers exactly the Julian month's own day count.
    #[test]
    fn month_span_has_julian_length(y in -100_000i64..=100_000, m in 1u8..=12) {
        let Ok(Converted::Span { earliest, latest }) = convert(y, Some(m), None) else {
            return Err(TestCaseError::fail("month must convert to a span"));
        };
        let days = gregorian_jdn(latest) - gregorian_jdn(earliest) + 1;
        prop_assert_eq!(days, i128::from(julian_last_day(m, is_julian_leap(y))));
    }

    /// Day-precision conversion agrees with the span endpoints: converting
    /// the first/last Julian day of a month lands on its span's bounds.
    #[test]
    fn day_conversion_agrees_with_spans(y in -100_000i64..=100_000, m in 1u8..=12) {
        let Ok(Converted::Span { earliest, latest }) = convert(y, Some(m), None) else {
            return Err(TestCaseError::fail("month must convert to a span"));
        };
        prop_assert_eq!(convert(y, Some(m), Some(1)), Ok(Converted::Day(earliest)));
        let last = julian_last_day(m, is_julian_leap(y));
        prop_assert_eq!(convert(y, Some(m), Some(last)), Ok(Converted::Day(latest)));
    }
}
