//! Earliest/latest calendar-day bounds for EDTF expressions.
//!
//! Every EDTF expression denotes a region of the time axis. `bounds()`
//! reports the earliest and latest proleptic-Gregorian calendar day that
//! region touches — the primitive that range queries (`edtf_min`/`edtf_max`
//! in SQL, filtering in an application) are built from.
//!
//! Conventions (documented, spec-derived where the spec speaks):
//! - Bounds are day-granular; time-of-day refines within a day and does not
//!   change them.
//! - Qualification (`?~%`) does not move bounds: `1985~` still bounds to
//!   the calendar year 1985 (ISO 8601-2 §8.4.2 NOTE — approximation widens
//!   confidence, not the written value).
//! - Unspecified digits bound to the full set of matching completions;
//!   `XXXX` is "a four-digit year" (ISO 8601-2 §9.2.1.2 c 4), so it bounds
//!   to 0000-01-01..9999-12-31.
//! - Season/sub-year boundaries use the fixed month table in
//!   `docs/spec-notes.md` D12 (ISO leaves seasons location-dependent).
//! - Open interval ends (`..`) are infinite; unknown ends (empty) are
//!   [`Bound::Unknown`].

use crate::types::*;

/// A concrete proleptic-Gregorian calendar day used as a bound.
///
/// Ordering is calendrical (year, then month, then day). Year is
/// astronomical numbering: 0 is a leap year, -1 precedes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BoundDate {
    /// Astronomical year number.
    pub year: i64,
    /// Calendar month 1–12.
    pub month: u8,
    /// Calendar day of month 1–31.
    pub day: u8,
}

impl core::fmt::Display for BoundDate {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.year < 0 {
            write!(f, "-{:04}-{:02}-{:02}", -self.year, self.month, self.day)
        } else {
            write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
        }
    }
}

/// One side of a [`Bounds`] result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Bound {
    /// Unbounded in the past (open interval start, `..date` endpoints).
    NegativeInfinity,
    /// A concrete calendar day.
    Date(BoundDate),
    /// Unbounded in the future (open interval end, `date..` endpoints).
    PositiveInfinity,
    /// Not determinable: unknown interval endpoints, or year values beyond
    /// the numeric range this library computes with.
    Unknown,
}

/// The earliest and latest calendar day an expression touches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Bounds {
    /// Earliest calendar day (inclusive).
    pub earliest: Bound,
    /// Latest calendar day (inclusive).
    pub latest: Bound,
}

impl Edtf {
    /// Compute the earliest/latest calendar-day bounds of this expression.
    pub fn bounds(&self) -> Bounds {
        match self {
            Edtf::Date(d) => date_bounds(d),
            Edtf::DateTime(dt) => date_bounds(&dt.date),
            Edtf::Interval(iv) => interval_bounds(iv),
            Edtf::Set(s) => set_bounds(s),
        }
    }
}

pub(crate) fn date_bounds(d: &Date) -> Bounds {
    // Y-prefixed / exponential years are year-precision by construction.
    if !matches!(d.year.kind, YearKind::Standard { .. }) {
        let Some(value) = d.year.value() else {
            return Bounds {
                earliest: Bound::Unknown,
                latest: Bound::Unknown,
            };
        };
        let (lo, hi) = significant_range(value, d.year.significant_digits, big_width(&d.year.kind));
        return Bounds {
            earliest: Bound::Date(BoundDate {
                year: lo,
                month: 1,
                day: 1,
            }),
            latest: Bound::Date(BoundDate {
                year: hi,
                month: 12,
                day: 31,
            }),
        };
    }

    // Standard year with significant digits is year-precision only.
    if d.year.significant_digits.is_some() {
        let value = d.year.value().expect("S excludes X digits");
        let (lo, hi) = significant_range(value, d.year.significant_digits, 4);
        return Bounds {
            earliest: Bound::Date(BoundDate {
                year: lo,
                month: 1,
                day: 1,
            }),
            latest: Bound::Date(BoundDate {
                year: hi,
                month: 12,
                day: 31,
            }),
        };
    }

    // Season / sub-year grouping in the month slot.
    if let Some(code) = d.month.as_ref().and_then(DateField::value) {
        if (21..=41).contains(&code) {
            return season_bounds(d, code);
        }
    }

    let earliest = extremum(d, true);
    let latest = extremum(d, false);
    match (earliest, latest) {
        (Some(a), Some(b)) => Bounds {
            earliest: Bound::Date(a),
            latest: Bound::Date(b),
        },
        // Validation guarantees at least one completion, so this is
        // unreachable; stay total anyway.
        _ => Bounds {
            earliest: Bound::Unknown,
            latest: Bound::Unknown,
        },
    }
}

/// Digit width of the year form, for significant-digit ranges.
fn big_width(kind: &YearKind) -> u32 {
    match kind {
        YearKind::Standard { .. } => 4,
        YearKind::Big { value } => decimal_digits(value.unsigned_abs()),
        YearKind::Exponential {
            significand,
            exponent,
        } => decimal_digits(significand.unsigned_abs()) + exponent,
    }
}

fn decimal_digits(mut v: u64) -> u32 {
    let mut n = 1;
    while v >= 10 {
        v /= 10;
        n += 1;
    }
    n
}

/// The year range denoted by a value with `S<precision>` significant digits
/// over a `width`-digit form (ISO 8601-2 §4.4.3): keep the leading
/// `precision` digits, sweep the rest 0..9.
fn significant_range(value: i64, precision: Option<u32>, width: u32) -> (i64, i64) {
    let Some(p) = precision else {
        return (value, value);
    };
    let sweep = width.saturating_sub(p);
    let Some(modulus) = 10i64.checked_pow(sweep) else {
        return (value, value);
    };
    let mag = value.unsigned_abs() as i64;
    let lo_mag = mag - mag.rem_euclid(modulus);
    let hi_mag = lo_mag + (modulus - 1);
    if value < 0 {
        (-hi_mag, -lo_mag)
    } else {
        (lo_mag, hi_mag)
    }
}

/// First and last month of each sub-year grouping code (docs/spec-notes.md
/// D12). `wraps` means the grouping ends in the following year.
fn season_months(code: u8) -> (u8, u8, bool) {
    match code {
        21 | 25 | 31 => (3, 5, false),  // spring (N), autumn (S)
        22 | 26 | 32 => (6, 8, false),  // summer (N), winter (S)
        23 | 27 | 29 => (9, 11, false), // autumn (N), spring (S)
        24 | 28 | 30 => (12, 2, true),  // winter (N), summer (S)
        33 => (1, 3, false),
        34 => (4, 6, false),
        35 => (7, 9, false),
        36 => (10, 12, false),
        37 => (1, 4, false),
        38 => (5, 8, false),
        39 => (9, 12, false),
        40 => (1, 6, false),
        41 => (7, 12, false),
        _ => unreachable!("validated season code"),
    }
}

fn season_bounds(d: &Date, code: u8) -> Bounds {
    let (first, last, wraps) = season_months(code);
    let (y_lo, y_hi) = year_range(d);
    let end_year = if wraps { y_hi + 1 } else { y_hi };
    Bounds {
        earliest: Bound::Date(BoundDate {
            year: y_lo,
            month: first,
            day: 1,
        }),
        latest: Bound::Date(BoundDate {
            year: end_year,
            month: last,
            day: last_day(last, is_leap(end_year)),
        }),
    }
}

/// Min and max year completions of a standard year.
fn year_range(d: &Date) -> (i64, i64) {
    match d.year.kind {
        YearKind::Standard { negative, digits } => {
            if let Some(v) = d.year.value() {
                (v, v)
            } else {
                debug_assert!(!negative);
                let lo = (0..=9999i64)
                    .find(|y| year_matches(&digits, *y))
                    .expect("some year matches any digit pattern");
                let hi = (0..=9999i64)
                    .rev()
                    .find(|y| year_matches(&digits, *y))
                    .expect("some year matches any digit pattern");
                (lo, hi)
            }
        }
        _ => unreachable!("caller checked Standard"),
    }
}

/// Earliest (`ascending`) or latest completion of a masked/plain date.
fn extremum(d: &Date, ascending: bool) -> Option<BoundDate> {
    let (y_lo, y_hi) = year_range(d);
    let years: alloc::vec::Vec<i64> = match d.year.kind {
        YearKind::Standard { digits, .. } if d.year.value().is_none() => {
            let iter = (y_lo..=y_hi).filter(|y| year_matches(&digits, *y));
            if ascending {
                iter.collect()
            } else {
                let mut v: alloc::vec::Vec<i64> = iter.collect();
                v.reverse();
                v
            }
        }
        _ => alloc::vec![y_lo],
    };
    for y in years {
        let Some(month) = &d.month else {
            return Some(if ascending {
                BoundDate {
                    year: y,
                    month: 1,
                    day: 1,
                }
            } else {
                BoundDate {
                    year: y,
                    month: 12,
                    day: 31,
                }
            });
        };
        let mut months = month_candidates_of(month);
        if !ascending {
            months.reverse();
        }
        for m in months {
            let Some(day) = &d.day else {
                return Some(if ascending {
                    BoundDate {
                        year: y,
                        month: m,
                        day: 1,
                    }
                } else {
                    BoundDate {
                        year: y,
                        month: m,
                        day: last_day(m, is_leap(y)),
                    }
                });
            };
            let mut days = day_candidates_of(day);
            if !ascending {
                days.reverse();
            }
            for dd in days {
                if dd <= last_day(m, is_leap(y)) {
                    return Some(BoundDate {
                        year: y,
                        month: m,
                        day: dd,
                    });
                }
            }
        }
    }
    None
}

fn month_candidates_of(f: &DateField) -> alloc::vec::Vec<u8> {
    match f.value() {
        Some(v) => alloc::vec![v],
        None => (1..=12).filter(|v| field_matches(f, *v)).collect(),
    }
}

fn day_candidates_of(f: &DateField) -> alloc::vec::Vec<u8> {
    match f.value() {
        Some(v) => alloc::vec![v],
        None => (1..=31).filter(|v| field_matches(f, *v)).collect(),
    }
}

fn field_matches(f: &DateField, v: u8) -> bool {
    f.digits[0].is_none_or(|p| p == v / 10) && f.digits[1].is_none_or(|p| p == v % 10)
}

fn year_matches(digits: &[Option<u8>; 4], y: i64) -> bool {
    let actual = [(y / 1000) % 10, (y / 100) % 10, (y / 10) % 10, y % 10];
    digits
        .iter()
        .zip(actual)
        .all(|(pat, a)| pat.is_none_or(|p| i64::from(p) == a))
}

/// Proleptic Gregorian leap rule on the astronomical year number.
pub(crate) fn is_leap(y: i64) -> bool {
    y.rem_euclid(4) == 0 && (y.rem_euclid(100) != 0 || y.rem_euclid(400) == 0)
}

pub(crate) fn last_day(month: u8, leap: bool) -> u8 {
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

fn interval_bounds(iv: &Interval) -> Bounds {
    let earliest = match &iv.start {
        IntervalEndpoint::Open | IntervalEndpoint::OnOrBefore(_) => Bound::NegativeInfinity,
        IntervalEndpoint::Unknown => Bound::Unknown,
        IntervalEndpoint::Date(d) | IntervalEndpoint::OnOrAfter(d) => date_bounds(d).earliest,
    };
    let latest = match &iv.end {
        IntervalEndpoint::Open | IntervalEndpoint::OnOrAfter(_) => Bound::PositiveInfinity,
        IntervalEndpoint::Unknown => Bound::Unknown,
        IntervalEndpoint::Date(d) | IntervalEndpoint::OnOrBefore(d) => date_bounds(d).latest,
    };
    Bounds { earliest, latest }
}

fn set_bounds(s: &Set) -> Bounds {
    let mut earliest: Option<Bound> = None;
    let mut latest: Option<Bound> = None;
    for element in &s.elements {
        let (e, l) = match element {
            SetElement::Date(d) => {
                let b = date_bounds(d);
                (b.earliest, b.latest)
            }
            SetElement::OnOrBefore(d) => (Bound::NegativeInfinity, date_bounds(d).latest),
            SetElement::OnOrAfter(d) => (date_bounds(d).earliest, Bound::PositiveInfinity),
            SetElement::Range(a, b) => (date_bounds(a).earliest, date_bounds(b).latest),
        };
        earliest = Some(match earliest {
            None => e,
            Some(cur) => min_bound(cur, e),
        });
        latest = Some(match latest {
            None => l,
            Some(cur) => max_bound(cur, l),
        });
    }
    Bounds {
        earliest: earliest.unwrap_or(Bound::Unknown),
        latest: latest.unwrap_or(Bound::Unknown),
    }
}

fn min_bound(a: Bound, b: Bound) -> Bound {
    match (a, b) {
        (Bound::NegativeInfinity, _) | (_, Bound::NegativeInfinity) => Bound::NegativeInfinity,
        (Bound::Unknown, _) | (_, Bound::Unknown) => Bound::Unknown,
        (Bound::PositiveInfinity, x) | (x, Bound::PositiveInfinity) => x,
        (Bound::Date(x), Bound::Date(y)) => Bound::Date(x.min(y)),
    }
}

fn max_bound(a: Bound, b: Bound) -> Bound {
    match (a, b) {
        (Bound::PositiveInfinity, _) | (_, Bound::PositiveInfinity) => Bound::PositiveInfinity,
        (Bound::Unknown, _) | (_, Bound::Unknown) => Bound::Unknown,
        (Bound::NegativeInfinity, x) | (x, Bound::NegativeInfinity) => x,
        (Bound::Date(x), Bound::Date(y)) => Bound::Date(x.max(y)),
    }
}
