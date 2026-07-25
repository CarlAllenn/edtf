//! Data model for parsed EDTF expressions.
//!
//! The model preserves everything the string encodes beyond a yes/no answer:
//! per-component qualification, unspecified-digit masks, endpoint kinds, and
//! set semantics (see docs/spec-notes.md §6).

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Error returned when an input is not valid EDTF.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ParseError {
    pub message: &'static str,
}

impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "invalid EDTF: {}", self.message)
    }
}

impl std::error::Error for ParseError {}

/// Uncertainty (`?`), approximation (`~`), or both (`%`), per ISO 8601-2 §3.2.6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Qualifier {
    pub uncertain: bool,
    pub approximate: bool,
}

impl Qualifier {
    pub fn is_qualified(self) -> bool {
        self.uncertain || self.approximate
    }

    pub(crate) fn merge(&mut self, other: Qualifier) {
        self.uncertain |= other.uncertain;
        self.approximate |= other.approximate;
    }
}

/// The three syntactic forms a calendar year can take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum YearKind {
    /// Four-digit year, possibly negative, possibly with `X` digits
    /// (`None` = unspecified digit).
    Standard {
        negative: bool,
        digits: [Option<u8>; 4],
    },
    /// `Y`-prefixed year beyond ±9999 (ISO 8601-2 §4.7.2).
    Big { value: i64 },
    /// `Y…E…` exponential year (ISO 8601-2 §4.7.3).
    Exponential { significand: i64, exponent: u32 },
}

/// A calendar year with optional significant-digit precision and qualification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Year {
    pub kind: YearKind,
    /// `S`-suffix significant digits (ISO 8601-2 §4.4.3, §4.7.4).
    pub significant_digits: Option<u32>,
    pub qualifier: Qualifier,
}

impl Year {
    /// The numeric year value, when every digit is specified and the value is
    /// representable. `None` for masked years and overflowing exponentials.
    pub fn value(&self) -> Option<i64> {
        match self.kind {
            YearKind::Standard { negative, digits } => {
                let mut v: i64 = 0;
                for d in digits {
                    v = v * 10 + i64::from(d?);
                }
                Some(if negative { -v } else { v })
            }
            YearKind::Big { value } => Some(value),
            YearKind::Exponential {
                significand,
                exponent,
            } => significand.checked_mul(10i64.checked_pow(exponent)?),
        }
    }

    pub fn has_unspecified(&self) -> bool {
        matches!(self.kind, YearKind::Standard { digits, .. } if digits.iter().any(|d| d.is_none()))
    }
}

/// A two-digit month or day slot; `None` digits are unspecified (`X`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct DateField {
    pub digits: [Option<u8>; 2],
    pub qualifier: Qualifier,
}

impl DateField {
    /// The numeric value when both digits are specified.
    pub fn value(&self) -> Option<u8> {
        Some(self.digits[0]? * 10 + self.digits[1]?)
    }

    pub fn has_unspecified(&self) -> bool {
        self.digits.iter().any(|d| d.is_none())
    }
}

/// Precision of a date expression (its lowest specified component).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Precision {
    Year,
    Month,
    /// Month slot holds a sub-year grouping code 21–41 (ISO 8601-2 §4.8).
    Season,
    Day,
}

/// A (possibly qualified, possibly partially unspecified) EDTF date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Date {
    pub year: Year,
    /// Month 01–12 or sub-year grouping code 21–41.
    pub month: Option<DateField>,
    pub day: Option<DateField>,
}

impl Date {
    pub fn precision(&self) -> Precision {
        if self.day.is_some() {
            Precision::Day
        } else if let Some(m) = &self.month {
            match m.value() {
                Some(v) if v >= 21 => Precision::Season,
                _ => Precision::Month,
            }
        } else {
            Precision::Year
        }
    }
}

/// UTC designator or a numeric shift from UTC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum TimeShift {
    /// `Z`
    Utc,
    /// `±hh` or `±hh:mm`; `minutes` is the signed total offset.
    Offset { minutes: i16, hours_only: bool },
}

/// A complete time of day `hh:mm:ss` with optional shift (EDTF level 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Time {
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub shift: Option<TimeShift>,
}

/// A complete date and time of day (EDTF level 0 only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct DateTime {
    pub date: Date,
    pub time: Time,
}

/// One side of a time interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum IntervalEndpoint {
    Date(Date),
    /// `..` — open (unbounded).
    Open,
    /// Empty — unknown.
    Unknown,
    /// `..date` at the start: begins on or before this date (ISO 8601-2 §10.6).
    OnOrBefore(Date),
    /// `date..` at the end: ends on or after this date (ISO 8601-2 §10.6).
    OnOrAfter(Date),
}

impl IntervalEndpoint {
    pub fn is_dated(&self) -> bool {
        !matches!(self, IntervalEndpoint::Open | IntervalEndpoint::Unknown)
    }
}

/// A time interval `start/end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Interval {
    pub start: IntervalEndpoint,
    pub end: IntervalEndpoint,
}

/// `{…}` (all members) vs `[…]` (one member) — ISO 8601-2 §6.1–6.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum SetKind {
    AllMembers,
    OneMember,
}

/// An element of a set, including `..` range notation (ISO 8601-2 §6.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum SetElement {
    Date(Date),
    /// `..date`
    OnOrBefore(Date),
    /// `date..`
    OnOrAfter(Date),
    /// `a..b` inclusive range.
    Range(Date, Date),
}

/// A set expression (EDTF level 2).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Set {
    pub kind: SetKind,
    pub elements: Vec<SetElement>,
}

/// Any parsed EDTF expression.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Edtf {
    Date(Date),
    DateTime(DateTime),
    Interval(Interval),
    Set(Set),
}

impl Edtf {
    /// Parse an EDTF string (levels 0–2, ISO 8601-2:2019 Annex A).
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        crate::parser::parse(input)
    }

    /// The minimum EDTF conformance level (0, 1 or 2) able to express this
    /// value. Semantically equivalent spellings classify identically: e.g.
    /// `?1985-?04-?12` reports level 1 because `1985-04-12?` means the same.
    pub fn level(&self) -> u8 {
        match self {
            Edtf::Date(d) => date_level(d, false),
            Edtf::DateTime(_) => 0,
            Edtf::Interval(iv) => interval_level(iv),
            Edtf::Set(_) => 2,
        }
    }
}

impl core::str::FromStr for Edtf {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Edtf::parse(s)
    }
}

fn interval_level(iv: &Interval) -> u8 {
    let mut level = 0;
    for endpoint in [&iv.start, &iv.end] {
        level = level.max(match endpoint {
            IntervalEndpoint::Open | IntervalEndpoint::Unknown => 1,
            IntervalEndpoint::OnOrBefore(d) | IntervalEndpoint::OnOrAfter(d) => {
                2u8.max(date_level(d, true))
            }
            IntervalEndpoint::Date(d) => date_level(d, true),
        });
    }
    level
}

fn date_level(d: &Date, in_interval: bool) -> u8 {
    let mut level = 0u8;
    match d.year.kind {
        YearKind::Standard { negative, .. } => {
            if negative {
                level = level.max(1);
            }
        }
        YearKind::Big { .. } => level = level.max(1),
        YearKind::Exponential { .. } => level = level.max(2),
    }
    if d.year.significant_digits.is_some() {
        level = level.max(2);
    }
    if let Some(v) = d.month.as_ref().and_then(DateField::value) {
        if (21..=24).contains(&v) {
            level = level.max(1);
        } else if (25..=41).contains(&v) {
            level = level.max(2);
        }
    }
    let masks = mask_level(d);
    if masks > 0 {
        // Unspecified digits inside interval endpoints are a level 2 feature
        // (ISO 8601-2 §10.4 appears only in the level 2 half of Annex A usage).
        level = level.max(if in_interval { 2 } else { masks });
    }
    level.max(qualification_level(d))
}

/// 0 = unqualified; 1 = uniform ("complete") qualification; 2 = mixed.
fn qualification_level(d: &Date) -> u8 {
    let mut quals = [Some(d.year.qualifier), None, None];
    quals[1] = d.month.map(|m| m.qualifier);
    quals[2] = d.day.map(|f| f.qualifier);
    let present: Vec<Qualifier> = quals.iter().flatten().copied().collect();
    if present.iter().all(|q| !q.is_qualified()) {
        return 0;
    }
    if present.iter().all(|q| *q == present[0]) {
        1
    } else {
        2
    }
}

/// 0 = no X digits; 1 = one of the level 1 shapes (Annex A.5.5); 2 = any other.
fn mask_level(d: &Date) -> u8 {
    let year_x: [bool; 4] = match d.year.kind {
        YearKind::Standard { digits, .. } => [
            digits[0].is_none(),
            digits[1].is_none(),
            digits[2].is_none(),
            digits[3].is_none(),
        ],
        _ => [false; 4],
    };
    let year_full = !year_x.iter().any(|&x| x);
    let month_full = d.month.map(|m| !m.has_unspecified());
    let month_all_x = d.month.map(|m| m.digits.iter().all(|x| x.is_none()));
    let day_all_x = d.day.map(|f| f.digits.iter().all(|x| x.is_none()));
    let day_any_x = d.day.map(|f| f.has_unspecified());
    let any_x = !year_full
        || d.month.map(|m| m.has_unspecified()).unwrap_or(false)
        || day_any_x.unwrap_or(false);
    if !any_x {
        return 0;
    }
    // Level 1 shapes: `1985-04-XX`, `1985-XX-XX`, `2004-XX`, `201X`, `20XX`.
    let level1 = (year_full && month_full == Some(true) && day_all_x == Some(true))
        || (year_full && month_all_x == Some(true) && day_all_x == Some(true))
        || (year_full && month_all_x == Some(true) && d.day.is_none())
        || (d.month.is_none()
            && matches!(
                year_x,
                [false, false, false, true] | [false, false, true, true]
            ));
    if level1 {
        1
    } else {
        2
    }
}
