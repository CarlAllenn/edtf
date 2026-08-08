// SPDX-FileCopyrightText: Copyright (c) the edtf contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Data model for parsed EDTF expressions.
//!
//! The model preserves everything the string encodes beyond a yes/no answer:
//! per-component qualification, unspecified-digit masks, endpoint kinds, and
//! set semantics (see docs/spec-notes.md §6).

use alloc::vec::Vec;

/// Error returned when an input is not valid EDTF.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ParseError {
    /// Human-readable reason the input was rejected.
    pub message: &'static str,
    /// Byte offset into the original input where the problem was detected.
    pub offset: usize,
}

impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "invalid EDTF at offset {}: {}",
            self.offset, self.message
        )
    }
}

impl core::error::Error for ParseError {}

/// Uncertainty (`?`), approximation (`~`), or both (`%`), per ISO 8601-2
/// §3.2.6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Qualifier {
    /// The value's source is considered dubious (`?`).
    pub uncertain: bool,
    /// The value is an estimate (`~`).
    pub approximate: bool,
}

impl Qualifier {
    /// True if either flag is set.
    #[must_use]
    pub const fn is_qualified(self) -> bool {
        self.uncertain || self.approximate
    }

    pub(crate) fn merge(&mut self, other: Self) {
        self.uncertain |= other.uncertain;
        self.approximate |= other.approximate;
    }
}

/// The three syntactic forms a calendar year can take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum YearKind {
    /// Four-digit year, possibly negative, possibly with `X` digits
    /// (`None` = unspecified digit).
    Standard {
        /// True for years before year 0000 (written with a leading `-`).
        negative: bool,
        /// The four digits, most significant first; `None` is an `X`.
        digits: [Option<u8>; 4],
    },
    /// `Y`-prefixed year beyond ±9999 (ISO 8601-2 §4.7.2).
    Big {
        /// The signed year value.
        value: i64,
    },
    /// `Y…E…` exponential year (ISO 8601-2 §4.7.3).
    Exponential {
        /// Signed significand; the year is `significand × 10^exponent`.
        significand: i64,
        /// Power-of-ten exponent.
        exponent: u32,
    },
}

/// A calendar year with optional significant-digit precision and qualification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Year {
    /// Which syntactic form the year uses, and its digits.
    pub kind: YearKind,
    /// `S`-suffix significant digits (ISO 8601-2 §4.4.3, §4.7.4).
    pub significant_digits: Option<u32>,
    /// Qualification applying to the year component.
    pub qualifier: Qualifier,
}

impl Year {
    /// The numeric year value, when every digit is specified and the value is
    /// representable. `None` for masked years and overflowing exponentials.
    #[must_use]
    pub fn value(&self) -> Option<i64> {
        match self.kind {
            YearKind::Standard { negative, digits } => {
                let mut v: i64 = 0;
                for d in digits {
                    v = v * 10 + i64::from(d?);
                }
                Some(if negative { -v } else { v })
            },
            YearKind::Big { value } => Some(value),
            YearKind::Exponential {
                significand,
                exponent,
            } => significand.checked_mul(10i64.checked_pow(exponent)?),
        }
    }

    /// True if any digit is unspecified (`X`).
    #[must_use]
    pub fn has_unspecified(&self) -> bool {
        matches!(self.kind, YearKind::Standard { digits, .. } if digits.iter().any(Option::is_none))
    }
}

/// A two-digit month or day slot; `None` digits are unspecified (`X`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DateField {
    /// Tens digit then ones digit; `None` is an `X`.
    pub digits: [Option<u8>; 2],
    /// Qualification applying to this component.
    pub qualifier: Qualifier,
}

impl DateField {
    /// The numeric value when both digits are specified.
    #[must_use]
    pub fn value(self) -> Option<u8> {
        Some(self.digits[0]? * 10 + self.digits[1]?)
    }

    /// True if either digit is unspecified (`X`).
    #[must_use]
    pub fn has_unspecified(self) -> bool {
        self.digits.iter().any(Option::is_none)
    }
}

/// Precision of a date expression (its lowest specified component).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Precision {
    /// Year only (`1985`, `Y17E7`).
    Year,
    /// Year and calendar month (`1985-04`).
    Month,
    /// Month slot holds a sub-year grouping code 21–41 (ISO 8601-2 §4.8).
    Season,
    /// Complete calendar date (`1985-04-12`).
    Day,
}

/// A (possibly qualified, possibly partially unspecified) EDTF date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Date {
    /// The year component (always present).
    pub year: Year,
    /// Month 01–12 or sub-year grouping code 21–41.
    pub month: Option<DateField>,
    /// Calendar day of month.
    pub day: Option<DateField>,
}

impl Date {
    /// The precision of this date (its lowest specified component).
    #[must_use]
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

    fn any_component<F: Fn(Qualifier) -> bool>(&self, f: F) -> bool {
        f(self.year.qualifier)
            || self.month.is_some_and(|m| f(m.qualifier))
            || self.day.is_some_and(|d| f(d.qualifier))
    }

    /// True if any component is marked uncertain (`?` or `%`).
    #[must_use]
    pub fn is_uncertain(&self) -> bool {
        self.any_component(|q| q.uncertain)
    }

    /// True if any component is marked approximate (`~` or `%`).
    #[must_use]
    pub fn is_approximate(&self) -> bool {
        self.any_component(|q| q.approximate)
    }

    /// True if any component contains an unspecified digit (`X`).
    #[must_use]
    pub fn has_unspecified(&self) -> bool {
        self.year.has_unspecified()
            || self.month.is_some_and(DateField::has_unspecified)
            || self.day.is_some_and(DateField::has_unspecified)
    }
}

/// UTC designator or a numeric shift from UTC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TimeShift {
    /// `Z`
    Utc,
    /// `±hh` or `±hh:mm`.
    Offset {
        /// Signed total offset from UTC in minutes.
        minutes: i16,
        /// True when written in the hours-only form `±hh`.
        hours_only: bool,
    },
}

/// A complete time of day `hh:mm:ss` with optional shift (EDTF level 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Time {
    /// Clock hour 00–23.
    pub hour: u8,
    /// Clock minute 00–59.
    pub minute: u8,
    /// Clock second 00–60 (60 admits a leap second).
    pub second: u8,
    /// Optional UTC designator or shift.
    pub shift: Option<TimeShift>,
}

/// A complete date and time of day (EDTF level 0 only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DateTime {
    /// The complete calendar date.
    pub date: Date,
    /// The complete time of day.
    pub time: Time,
}

/// One side of a time interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum IntervalEndpoint {
    /// A concrete (possibly qualified/unspecified) date.
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
    /// True unless the endpoint is open (`..`) or unknown (empty).
    #[must_use]
    pub const fn is_dated(&self) -> bool {
        !matches!(self, Self::Open | Self::Unknown)
    }

    pub(crate) const fn date(&self) -> Option<&Date> {
        match self {
            Self::Date(d) | Self::OnOrBefore(d) | Self::OnOrAfter(d) => Some(d),
            _ => None,
        }
    }
}

/// A time interval `start/end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Interval {
    /// The start endpoint.
    pub start: IntervalEndpoint,
    /// The end endpoint.
    pub end: IntervalEndpoint,
}

/// `{…}` (all members) vs `[…]` (one member) — ISO 8601-2 §6.1–6.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SetKind {
    /// `{…}` — every member applies.
    AllMembers,
    /// `[…]` — exactly one (unknown) member applies.
    OneMember,
}

/// An element of a set, including `..` range notation (ISO 8601-2 §6.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SetElement {
    /// A single date.
    Date(Date),
    /// `..date` — this date or any earlier one.
    OnOrBefore(Date),
    /// `date..` — this date or any later one.
    OnOrAfter(Date),
    /// `a..b` — every expression between the two, inclusive.
    Range(Date, Date),
}

/// A set expression (EDTF level 2).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Set {
    /// All-members (`{}`) or one-member (`[]`) semantics.
    pub kind: SetKind,
    /// The elements, in written order.
    pub elements: Vec<SetElement>,
}

/// Any parsed EDTF expression.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Edtf {
    /// A single date.
    Date(Date),
    /// A date with time of day (level 0 only).
    DateTime(DateTime),
    /// A time interval.
    Interval(Interval),
    /// A set of dates (level 2).
    Set(Set),
}

impl Edtf {
    /// Parse an EDTF string (levels 0–2, ISO 8601-2:2019 Annex A).
    ///
    /// # Errors
    ///
    /// [`ParseError`] with a byte offset and message pointing at the first
    /// place the input stops being valid EDTF.
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        crate::parser::parse(input)
    }

    /// The minimum EDTF conformance level (0, 1 or 2) able to express this
    /// value. Semantically equivalent spellings classify identically: e.g.
    /// `?1985-?04-?12` reports level 1 because `1985-04-12?` means the same.
    #[must_use]
    pub fn level(&self) -> u8 {
        match self {
            Self::Date(d) => date_level(d, false),
            Self::DateTime(_) => 0,
            Self::Interval(iv) => interval_level(iv),
            Self::Set(_) => 2,
        }
    }

    fn any_date<F: Fn(&Date) -> bool>(&self, f: F) -> bool {
        match self {
            Self::Date(d) => f(d),
            Self::DateTime(dt) => f(&dt.date),
            Self::Interval(iv) => iv.start.date().is_some_and(&f) || iv.end.date().is_some_and(&f),
            Self::Set(s) => s.elements.iter().any(|e| match e {
                SetElement::Date(d) | SetElement::OnOrBefore(d) | SetElement::OnOrAfter(d) => f(d),
                // Range endpoints parse unqualified and fully specified, so
                // today every predicate is false here; stay structural.
                SetElement::Range(a, b) => f(a) || f(b),
            }),
        }
    }

    /// True if any component anywhere is marked uncertain (`?` or `%`).
    pub fn is_uncertain(&self) -> bool {
        self.any_date(Date::is_uncertain)
    }

    /// True if any component anywhere is marked approximate (`~` or `%`).
    pub fn is_approximate(&self) -> bool {
        self.any_date(Date::is_approximate)
    }

    /// True if any component anywhere contains an unspecified digit (`X`).
    pub fn has_unspecified(&self) -> bool {
        self.any_date(Date::has_unspecified)
    }
}

impl core::str::FromStr for Edtf {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

fn interval_level(iv: &Interval) -> u8 {
    let mut level = 0;
    for endpoint in [&iv.start, &iv.end] {
        level = level.max(match endpoint {
            IntervalEndpoint::Open | IntervalEndpoint::Unknown => 1,
            IntervalEndpoint::OnOrBefore(d) | IntervalEndpoint::OnOrAfter(d) => {
                2u8.max(date_level(d, true))
            },
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
        },
        YearKind::Big { .. } => level = level.max(1),
        YearKind::Exponential { .. } => level = level.max(2),
    }
    if d.year.significant_digits.is_some() {
        level = level.max(2);
    }
    if let Some(v) = d.month.and_then(DateField::value) {
        if (21..=24).contains(&v) {
            level = level.max(1);
        } else if (25..=41).contains(&v) {
            level = level.max(2);
        }
    }
    let masks = mask_level(d);
    if masks > 0 {
        // Unspecified digits inside interval endpoints are a level 2 feature
        // (ISO 8601-2 §10.4 is absent from the level 1 half of Annex A).
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
    let month_all_x = d.month.map(|m| m.digits.iter().all(Option::is_none));
    let day_all_x = d.day.map(|f| f.digits.iter().all(Option::is_none));
    let day_any_x = d.day.map(DateField::has_unspecified);
    let any_x =
        !year_full || d.month.is_some_and(DateField::has_unspecified) || day_any_x.unwrap_or(false);
    if !any_x {
        return 0;
    }
    // Level 1 shapes: `1985-04-XX`, `1985-XX-XX`, `2004-XX`, `201X`, `20XX`.
    let level1 = (year_full && month_full == Some(true) && day_all_x == Some(true))
        || (year_full && month_all_x == Some(true) && day_all_x == Some(true))
        || (year_full && month_all_x == Some(true) && d.day.is_none())
        || (d.month.is_none() && matches!(year_x, [false, false, false | true, true]));
    if level1 { 1 } else { 2 }
}
