//! Proleptic Julian (Old Style) → Gregorian conversion at the EDTF input
//! boundary.
//!
//! EDTF is proleptic-Gregorian by spec and stays that way — but pre-1918
//! Russian/Imperial sources, Orthodox liturgical dates, and most primary
//! material before a country's adoption date are written Julian (Old
//! Style). This crate converts proleptic Julian calendar dates to proleptic
//! Gregorian and hands the result to [`edtf_core`]'s model, so the
//! conversion is done once, deterministically, instead of hand-rolled per
//! ingest layer. Deterministic integer arithmetic (Julian Day Number
//! route), `#![no_std]`, zero dependencies beyond `edtf-core`.
//!
//! The traps this crate exists to get right, each pinned by tests:
//!
//! - The Julian–Gregorian offset is not constant: 10 days at the 1582 reform,
//!   11 from Gregorian 1700-03-01, 12 from 1800, 13 from 1900 — 1 March 1917
//!   O.S. is 14 March 1917 N.S.
//! - The Julian leap rule is every-4-years with no century exception: Julian
//!   1900-02-29 exists and converts (to Gregorian 1900-03-13), while Gregorian
//!   1900-02-29 stays invalid.
//! - Proleptic in both directions: defined before 1582 too. The offset shrinks
//!   going back, the calendars coincide across the 3rd century, and the Julian
//!   calendar is *ahead* before that (Julian 0001-01-01 is Gregorian
//!   0000-12-30).
//! - Year boundaries shift: Julian late-December dates land in the next
//!   Gregorian year — 25 December 1917 O.S. is 7 January 1918 N.S., the classic
//!   Orthodox-Christmas off-by-one-year error.
//! - Astronomical year numbering throughout (year 0 exists), consistent with
//!   `edtf-core`.
//! - **Precision honesty**: a Julian year or month does not convert to a single
//!   Gregorian year or month (Julian 1917 spans Gregorian
//!   1917-01-14..1918-01-13). [`convert`] returns sub-day-precision inputs as
//!   an explicit earliest/latest [`Converted::Span`] — never a silently
//!   "converted" year.
//!
//! ```
//! use edtf_calendars::{Converted, JulianDate, convert, julian_to_gregorian};
//!
//! // The October Revolution: 25 October 1917 O.S. = 7 November 1917 N.S.
//! let g = julian_to_gregorian(JulianDate {
//!     year: 1917,
//!     month: 10,
//!     day: 25,
//! })
//! .unwrap();
//! assert_eq!(g.to_string(), "1917-11-07");
//!
//! // A Julian *year* is honestly a Gregorian range, EDTF `1917-01-14/1918-01-13`:
//! let Converted::Span { earliest, latest } = convert(1917, None, None).unwrap() else {
//!     unreachable!()
//! };
//! assert_eq!(earliest.to_string(), "1917-01-14");
//! assert_eq!(latest.to_string(), "1918-01-13");
//! ```
//!
//! **Non-goals** (recorded in issue #21): other calendars (Islamic, Hebrew,
//! French Republican) until a real source demands them; England's 25-March
//! year start and dual-dating ("11 February 1731/32") — a known ingest
//! hazard the caller must resolve to an astronomical year *before* calling
//! this crate; and guessing which calendar a source used, which is the
//! application's vocabulary-backed qualifier, not arithmetic.
#![no_std]

use edtf_core::BoundDate;

/// Error returned for invalid calendar dates or unrepresentable results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CalendarError {
    /// The input is not a real calendar day (month outside 1–12, day
    /// beyond the month's length under the calendar's own leap rule, or a
    /// day given without a month).
    InvalidDate,
    /// The converted year does not fit the numeric range this library
    /// computes with.
    OutOfRange,
}

impl core::fmt::Display for CalendarError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::InvalidDate => "not a valid calendar date",
            Self::OutOfRange => "converted year out of range",
        })
    }
}

impl core::error::Error for CalendarError {}

/// A proleptic Julian (Old Style) calendar date.
///
/// Year is astronomical numbering: 0 exists (and is a Julian leap year),
/// -1 precedes it. Ordering is calendrical.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct JulianDate {
    /// Astronomical year number.
    pub year: i64,
    /// Calendar month 1–12.
    pub month: u8,
    /// Calendar day of month 1–31.
    pub day: u8,
}

impl core::fmt::Display for JulianDate {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.year < 0 {
            write!(f, "-{:04}-{:02}-{:02}", -self.year, self.month, self.day)
        } else {
            write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
        }
    }
}

/// A precision-honest conversion result: day-precision input converts to a
/// single day; year- or month-precision input converts to the earliest and
/// latest Gregorian days the Julian span covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Converted {
    /// A complete Julian date: exactly one proleptic-Gregorian day.
    Day(BoundDate),
    /// A Julian year or month: the Gregorian days it spans, inclusive.
    /// Feed these to an EDTF interval (`earliest/latest`) — the span is
    /// never a single Gregorian year or month.
    Span {
        /// Gregorian day of the Julian span's first day.
        earliest: BoundDate,
        /// Gregorian day of the Julian span's last day.
        latest: BoundDate,
    },
}

/// Proleptic Julian leap rule: every fourth year, no century exception,
/// astronomical numbering (year 0 is a leap year, so are -4, -8, …).
#[must_use]
pub const fn is_julian_leap(year: i64) -> bool {
    year.rem_euclid(4) == 0
}

/// Proleptic Gregorian leap rule on the astronomical year number.
const fn is_gregorian_leap(year: i64) -> bool {
    year.rem_euclid(4) == 0 && (year.rem_euclid(100) != 0 || year.rem_euclid(400) == 0)
}

/// Days in `month` (1–12) under the given February length.
fn last_day(month: u8, leap: bool) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if leap {
                29
            } else {
                28
            }
        },
        _ => unreachable!("caller validated month"),
    }
}

fn check(month: u8, day: u8, leap: bool) -> Result<(), CalendarError> {
    if !(1..=12).contains(&month) || day == 0 || day > last_day(month, leap) {
        return Err(CalendarError::InvalidDate);
    }
    Ok(())
}

// Julian Day Number arithmetic (Fliegel–Van Flandern / Richards forms with
// floor division), computed in i128 so any i64 year is safe, valid for all
// proleptic dates in both calendars.

const fn julian_jdn(y: i128, m: i128, d: i128) -> i128 {
    let a = (14 - m).div_euclid(12);
    let y = y + 4800 - a;
    let m = m + 12 * a - 3;
    d + (153 * m + 2).div_euclid(5) + 365 * y + y.div_euclid(4) - 32083
}

const fn gregorian_jdn(y: i128, m: i128, d: i128) -> i128 {
    let a = (14 - m).div_euclid(12);
    let y = y + 4800 - a;
    let m = m + 12 * a - 3;
    d + (153 * m + 2).div_euclid(5) + 365 * y + y.div_euclid(4) - y.div_euclid(100)
        + y.div_euclid(400)
        - 32045
}

/// Split the era-day count `e` (per 4-year cycle) into (year-in-era, month,
/// day) — the shared tail of both JDN inversions.
const fn split(c: i128) -> (i128, u8, u8) {
    let d = (4 * c + 3).div_euclid(1461);
    let e = c - (1461 * d).div_euclid(4);
    let m = (5 * e + 2).div_euclid(153);
    let day = e - (153 * m + 2).div_euclid(5) + 1;
    let month = m + 3 - 12 * m.div_euclid(10);
    // month ∈ 3..=14 → 1..=12 after the +3-12·q fold; day ∈ 1..=31: both
    // are single-byte by construction of the civil-from-days algorithm.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "month in 1..=12 and day in 1..=31 by the algorithm's arithmetic"
    )]
    (d + m.div_euclid(10), month as u8, day as u8)
}

const fn jdn_to_gregorian(jdn: i128) -> (i128, u8, u8) {
    let a = jdn + 32044;
    let b = (4 * a + 3).div_euclid(146_097);
    let c = a - (146_097 * b).div_euclid(4);
    let (y, month, day) = split(c);
    (100 * b + y - 4800, month, day)
}

const fn jdn_to_julian(jdn: i128) -> (i128, u8, u8) {
    let (y, month, day) = split(jdn + 32082);
    (y - 4800, month, day)
}

fn to_bound_date(ymd: (i128, u8, u8)) -> Result<BoundDate, CalendarError> {
    Ok(BoundDate {
        year: i64::try_from(ymd.0).map_err(|_| CalendarError::OutOfRange)?,
        month: ymd.1,
        day: ymd.2,
    })
}

/// Convert a complete proleptic Julian date to its proleptic Gregorian day.
///
/// Rejects inputs that are not real Julian calendar days — but note that
/// Julian February 29 exists in *every* fourth year: `1900-02-29` (O.S.)
/// is valid here and converts to Gregorian `1900-03-13`.
///
/// # Errors
///
/// [`CalendarError::InvalidDate`] when the input is not a real Julian
/// calendar day; [`CalendarError::OutOfRange`] when the converted year
/// exceeds the numeric range this library computes with.
pub fn julian_to_gregorian(date: JulianDate) -> Result<BoundDate, CalendarError> {
    check(date.month, date.day, is_julian_leap(date.year))?;
    let jdn = julian_jdn(
        i128::from(date.year),
        i128::from(date.month),
        i128::from(date.day),
    );
    to_bound_date(jdn_to_gregorian(jdn))
}

/// Convert a complete proleptic Gregorian date to its proleptic Julian
/// (Old Style) date — the round-trip partner of [`julian_to_gregorian`].
///
/// Rejects inputs that are not real Gregorian calendar days: Gregorian
/// `1900-02-29` never existed and stays invalid, exactly as in `edtf-core`.
///
/// # Errors
///
/// [`CalendarError::InvalidDate`] when the input is not a real Gregorian
/// calendar day; [`CalendarError::OutOfRange`] when the converted year
/// exceeds the numeric range this library computes with.
pub fn gregorian_to_julian(date: BoundDate) -> Result<JulianDate, CalendarError> {
    check(date.month, date.day, is_gregorian_leap(date.year))?;
    let jdn = gregorian_jdn(
        i128::from(date.year),
        i128::from(date.month),
        i128::from(date.day),
    );
    let (year, month, day) = jdn_to_julian(jdn);
    Ok(JulianDate {
        year: i64::try_from(year).map_err(|_| CalendarError::OutOfRange)?,
        month,
        day,
    })
}

/// Precision-honest conversion of a possibly partial Julian date.
///
/// - year + month + day → [`Converted::Day`], the exact Gregorian day;
/// - year + month → [`Converted::Span`] over the Julian month;
/// - year only → [`Converted::Span`] over the Julian year;
/// - a day without a month is [`CalendarError::InvalidDate`].
///
/// The span is the whole point: Julian 1917 covers Gregorian
/// 1917-01-14..1918-01-13, and Julian February 1900 covers Gregorian
/// 1900-02-13..1900-03-13 (the offset changes *inside* the month). This
/// function never pretends a Julian year or month is a Gregorian one.
///
/// # Errors
///
/// [`CalendarError::InvalidDate`] when the parts are not a real Julian
/// calendar day; [`CalendarError::OutOfRange`] when the converted year
/// exceeds the numeric range this library computes with.
pub fn convert(year: i64, month: Option<u8>, day: Option<u8>) -> Result<Converted, CalendarError> {
    let leap = is_julian_leap(year);
    match (month, day) {
        (Some(month), Some(day)) => Ok(Converted::Day(julian_to_gregorian(JulianDate {
            year,
            month,
            day,
        })?)),
        (Some(month), None) => {
            if !(1..=12).contains(&month) {
                return Err(CalendarError::InvalidDate);
            }
            Ok(Converted::Span {
                earliest: julian_to_gregorian(JulianDate {
                    year,
                    month,
                    day: 1,
                })?,
                latest: julian_to_gregorian(JulianDate {
                    year,
                    month,
                    day: last_day(month, leap),
                })?,
            })
        },
        (None, None) => Ok(Converted::Span {
            earliest: julian_to_gregorian(JulianDate {
                year,
                month: 1,
                day: 1,
            })?,
            latest: julian_to_gregorian(JulianDate {
                year,
                month: 12,
                day: 31,
            })?,
        }),
        (None, Some(_)) => Err(CalendarError::InvalidDate),
    }
}
