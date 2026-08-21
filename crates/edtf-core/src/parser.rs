// SPDX-FileCopyrightText: Copyright (c) the edtf contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Hand-written recursive-descent parser and validator for EDTF levels 0–2.
//!
//! Grammar and validation rules follow docs/spec-notes.md, which cites the
//! ISO 8601-1/-2:2019 sections for every production. The D-decisions are
//! implemented as resolved there.
//!
//! Every error carries the byte offset (into the original input) where the
//! problem was detected; sub-parsers receive a `base` offset so positions
//! stay absolute inside intervals, sets and date-times.

#![expect(
    clippy::min_ident_chars,
    reason = "a byte scanner's b, c, s and i, alongside a date's y, m and d, are this grammar's notation; spelling them out makes the productions harder to follow, not easier"
)]
#![expect(
    clippy::arithmetic_side_effects,
    reason = "every flagged operation is bounded where it stands: slice indices by a length guard on the line above, digit values to 0-9 by the match arm that binds them, and the JDN forms by being computed in i128 so any i64 year fits. The operations that genuinely could leave range already use checked_/saturating_ and return an error rather than wrapping"
)]
#![expect(
    clippy::indexing_slicing,
    reason = "each index is preceded by the bounds check that justifies it, or indexes a fixed-size array whose length is in its type"
)]
#![expect(
    clippy::single_call_fn,
    reason = "a named helper used once is extraction for readability, which is the opposite of a defect; several are also the named steps the module docs describe"
)]
#![expect(
    clippy::integer_division_remainder_used,
    reason = "calendar arithmetic is integer division by definition — the leap rules are /4, /100 and /400, and a float would be wrong"
)]
#![expect(
    clippy::as_conversions,
    reason = "the operands are proven in range by the guard or type immediately above each cast, and try_from at these sites would add an unreachable error path"
)]
#![expect(
    clippy::integer_division,
    reason = "calendar arithmetic is integer division by definition — the leap rules are /4, /100 and /400, and a float would be wrong"
)]
#![expect(
    clippy::modulo_arithmetic,
    reason = "digit and calendar-cycle extraction is modulo by definition"
)]
#![expect(
    clippy::string_slice,
    reason = "the slice boundaries are byte offsets the parser itself produced and has already proven to be char boundaries"
)]
#![expect(
    clippy::missing_asserts_for_indexing,
    reason = "the length guard immediately above the index is the assertion"
)]
#![expect(
    clippy::pattern_type_mismatch,
    reason = "matching through a reference without restating & at every level is the idiomatic form the rest of this crate uses"
)]
#![expect(
    clippy::redundant_pub_crate,
    reason = "pub(crate) states the intended visibility even where the module tree makes it redundant today"
)]
#![expect(
    clippy::shadow_unrelated,
    reason = "short-lived rebinding inside one expression chain, where a second name would say nothing"
)]
#![expect(
    clippy::single_char_lifetime_names,
    reason = "'a is the conventional name for the single borrowed input's lifetime"
)]
#![expect(
    clippy::unreachable,
    reason = "an unreachable! whose comment names the caller-side check that makes it unreachable — a deliberate assertion of an invariant, not an unhandled case"
)]
#![expect(
    clippy::missing_errors_doc,
    reason = "edtf-core declares every module private and exports only named types, so nothing flagged here is reachable from outside the crate and there is no published error contract to document"
)]
#![expect(
    clippy::too_long_first_doc_paragraph,
    reason = "the items are crate-private, so these paragraphs render in no rustdoc summary; they are written to be read next to the code they describe"
)]

use alloc::{vec, vec::Vec};

use crate::{
    bounds::{Bound, date_bounds, is_leap},
    types::{
        Date, DateField, DateTime, Edtf, Interval, IntervalEndpoint, ParseError, Precision,
        Qualifier, Set, SetElement, SetKind, Time, TimeShift, Year, YearKind,
    },
};

/// Parse a whole EDTF expression, or report where it stopped being one.
pub(crate) fn parse(input: &str) -> Result<Edtf, ParseError> {
    if input.is_empty() {
        return Err(err(0, "empty input"));
    }
    if let Some(pos) = input.bytes().position(|b| !b.is_ascii()) {
        return Err(err(pos, "EDTF permits ASCII characters only"));
    }
    if let Some(pos) = input
        .bytes()
        .position(|b| b == b' ' || b.is_ascii_control())
    {
        return Err(err(pos, "whitespace is not allowed"));
    }
    match input.as_bytes()[0] {
        b'{' | b'[' => parse_set(input),
        _ if input.contains('/') => parse_interval(input),
        _ if input.contains('T') => parse_datetime(input),
        _ => parse_date_at(input, 0).map(Edtf::Date),
    }
}

/// A parse error at `offset` with a fixed message.
const fn err(offset: usize, message: &'static str) -> ParseError {
    ParseError { message, offset }
}

/// Byte offset of `part` (a slice borrowed from `whole`) within `whole`.
fn offset_in(whole: &str, part: &str) -> usize {
    (part.as_ptr() as usize).saturating_sub(whole.as_ptr() as usize)
}

// ---------------------------------------------------------------- cursor

/// A cursor over the input's bytes, carrying its offset in the original.
struct Cur<'a> {
    /// The bytes still being scanned.
    b: &'a [u8],
    /// Index of the next byte in `b`.
    i: usize,
    /// Offset of `b[0]` within the original input.
    base: usize,
}

impl<'a> Cur<'a> {
    /// A cursor over `s`, whose first byte sits at `base` in the original.
    const fn new(s: &'a str, base: usize) -> Self {
        Self {
            b: s.as_bytes(),
            i: 0,
            base,
        }
    }

    /// The current position, in the original input's coordinates.
    const fn pos(&self) -> usize {
        self.base + self.i
    }

    /// A parse error at the current position.
    const fn fail(&self, message: &'static str) -> ParseError {
        err(self.pos(), message)
    }

    /// The next byte without consuming it.
    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }

    /// Consume and return the next byte.
    fn bump(&mut self) -> Option<u8> {
        let c = self.peek();
        if c.is_some() {
            self.i += 1;
        }
        c
    }

    /// Consume the next byte if it is `c`; report whether it was.
    fn eat(&mut self, c: u8) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    /// Whether the cursor has reached the end of its input.
    const fn eof(&self) -> bool {
        self.i >= self.b.len()
    }

    /// Consume a trailing qualifier character, if one is present.
    fn take_qualifier(&mut self) -> Option<Qualifier> {
        let q = match self.peek()? {
            b'?' => Qualifier {
                uncertain: true,
                approximate: false,
            },
            b'~' => Qualifier {
                uncertain: false,
                approximate: true,
            },
            b'%' => Qualifier {
                uncertain: true,
                approximate: true,
            },
            _ => return None,
        };
        self.i += 1;
        Some(q)
    }

    /// Consume consecutive digits into an i64; returns (value, digit count).
    fn take_number(&mut self) -> Result<(i64, usize), ParseError> {
        let start = self.i;
        let mut v: i64 = 0;
        while let Some(d @ b'0'..=b'9') = self.peek() {
            self.i += 1;
            v = v
                .checked_mul(10)
                .and_then(|x| x.checked_add(i64::from(d - b'0')))
                .ok_or_else(|| err(self.base + start, "number out of supported range"))?;
        }
        Ok((v, self.i - start))
    }

    /// Two characters, each a digit or `X`.
    fn take_two(&mut self, what: &'static str) -> Result<[Option<u8>; 2], ParseError> {
        let mut out = [None; 2];
        for slot in &mut out {
            match self.bump() {
                Some(d @ b'0'..=b'9') => *slot = Some(d - b'0'),
                Some(b'X') => *slot = None,
                _ => {
                    return Err(ParseError {
                        message: match what {
                            "month" => "month must be two digits (or X)",
                            _ => "day must be two digits (or X)",
                        },
                        offset: self.base + self.i.saturating_sub(1),
                    });
                }
            }
        }
        Ok(out)
    }
}

// ---------------------------------------------------------------- dates

#[expect(
    clippy::too_many_lines,
    reason = "one linear scan; splitting scatters the offset bookkeeping"
)]
/// Parse a date whose first byte sits at `base` in the original input.
pub(crate) fn parse_date_at(s: &str, base: usize) -> Result<Date, ParseError> {
    // Every caller (dispatcher, interval endpoints, set elements) rejects
    // empty slices with its own message first; a graceful error beats a
    // panic if that invariant ever breaks.
    if s.is_empty() {
        return Err(err(base, "empty date"));
    }
    if s.as_bytes()[0] == b'Y' {
        return parse_prefixed_year(s, base);
    }
    let mut c = Cur::new(s, base);

    // Year: optional individual qualifier, optional '-', exactly four digit/X.
    let mut year_qual = Qualifier::default();
    if let Some(q) = c.take_qualifier() {
        year_qual.merge(q);
    }
    let negative = c.eat(b'-');
    let year_off = c.pos();
    let mut ydigits = [None; 4];
    for slot in &mut ydigits {
        match c.bump() {
            Some(d @ b'0'..=b'9') => *slot = Some(d - b'0'),
            Some(b'X') => *slot = None,
            _ => {
                return Err(err(
                    c.pos().saturating_sub(1),
                    "year must have exactly four digits (or X)",
                ));
            }
        }
    }
    let year_has_x = ydigits.iter().any(Option::is_none);
    if negative && year_has_x {
        return Err(err(
            year_off,
            "negative years cannot contain unspecified digits",
        ));
    }
    if negative && ydigits == [Some(0); 4] {
        return Err(err(year_off - 1, "-0000 is not a valid year"));
    }

    // Optional S<digits> significant-digit suffix (year-precision only).
    let significant = if c.eat(b'S') {
        if year_has_x {
            return Err(err(
                c.pos() - 1,
                "significant digits cannot combine with unspecified digits",
            ));
        }
        let sig_off = c.pos();
        let (n, len) = c.take_number()?;
        let sig = u32::try_from(n).ok().filter(|s| (1..=4).contains(s));
        if len == 0 || sig.is_none() {
            return Err(err(
                sig_off,
                "significant digits must be 1-4 for a four-digit year",
            ));
        }
        sig
    } else {
        None
    };

    // Optional trailing qualifier (group: year, or complete if year-only).
    if let Some(q) = c.take_qualifier() {
        year_qual.merge(q);
    }

    if c.eof() {
        return finish_date(
            negative,
            ydigits,
            significant,
            year_qual,
            None,
            None,
            year_off,
            year_off,
        );
    }
    if significant.is_some() {
        return Err(c.fail("significant-digit years are year-precision only"));
    }
    if !c.eat(b'-') {
        return Err(c.fail("expected '-' before month"));
    }

    // Month.
    let mut month_qual = Qualifier::default();
    if let Some(q) = c.take_qualifier() {
        month_qual.merge(q);
    }
    let month_off = c.pos();
    let month_digits = c.take_two("month")?;
    if let Some(q) = c.take_qualifier() {
        // Group qualification: applies to this component and everything left.
        month_qual.merge(q);
        year_qual.merge(q);
    }
    let mut month = DateField {
        digits: month_digits,
        qualifier: month_qual,
    };

    if c.eof() {
        return finish_date(
            negative,
            ydigits,
            None,
            year_qual,
            Some(month),
            None,
            month_off,
            month_off,
        );
    }
    if !c.eat(b'-') {
        return Err(c.fail("expected '-' before day"));
    }

    // Day.
    let mut day_qual = Qualifier::default();
    if let Some(q) = c.take_qualifier() {
        day_qual.merge(q);
    }
    let day_off = c.pos();
    let day_digits = c.take_two("day")?;
    if let Some(q) = c.take_qualifier() {
        // Trailing qualifier after the last component = complete qualification.
        day_qual.merge(q);
        month.qualifier.merge(q);
        year_qual.merge(q);
    }
    if !c.eof() {
        return Err(c.fail("unexpected characters after day"));
    }
    let day = DateField {
        digits: day_digits,
        qualifier: day_qual,
    };
    finish_date(
        negative,
        ydigits,
        None,
        year_qual,
        Some(month),
        Some(day),
        month_off,
        day_off,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "finisher taking every parsed component; it has one caller"
)]
/// Assemble a validated date from its parsed parts.
fn finish_date(
    negative: bool,
    ydigits: [Option<u8>; 4],
    significant: Option<u32>,
    year_qual: Qualifier,
    month: Option<DateField>,
    day: Option<DateField>,
    month_off: usize,
    day_off: usize,
) -> Result<Date, ParseError> {
    let year = Year {
        kind: YearKind::Standard {
            negative,
            digits: ydigits,
        },
        significant_digits: significant,
        qualifier: year_qual,
    };
    validate_month_day(&year.kind, month.as_ref(), day.as_ref(), month_off, day_off)?;
    Ok(Date { year, month, day })
}

/// Reject month/day combinations no completion of the year admits.
fn validate_month_day(
    year: &YearKind,
    month: Option<&DateField>,
    day: Option<&DateField>,
    month_off: usize,
    day_off: usize,
) -> Result<(), ParseError> {
    let Some(m) = month else {
        return Ok(());
    };
    match m.value() {
        Some(v) => {
            if !((1..=12).contains(&v) || (21..=41).contains(&v)) {
                return Err(err(
                    month_off,
                    "month must be 01-12 or a sub-year code 21-41",
                ));
            }
            if (21..=41).contains(&v) && day.is_some() {
                return Err(err(day_off, "sub-year groupings cannot carry a day"));
            }
        }
        None => {
            // Masked months match calendar months 01-12 only (decision D14);
            // sub-year codes must be written explicitly.
            if month_candidates(*m).is_empty() {
                return Err(err(
                    month_off,
                    "no calendar month matches the unspecified digits",
                ));
            }
        }
    }
    if let Some(d) = day {
        if !day_has_valid_completion(year, *m, *d) {
            return Err(err(day_off, "day is out of range for the month"));
        }
    }
    Ok(())
}

/// Every month value a (possibly masked) month field admits.
fn month_candidates(m: DateField) -> Vec<u8> {
    match m.value() {
        Some(v) if (1..=12).contains(&v) => vec![v],
        // Callers run after month validation (sub-year codes with a day are
        // already rejected), so this arm only guards against misuse.
        Some(_) => Vec::new(),
        None => (1..=12).filter(|v| field_matches(m, *v)).collect(),
    }
}

/// Every day value a (possibly masked) day field admits.
fn day_candidates(d: DateField) -> Vec<u8> {
    match d.value() {
        Some(v) if (1..=31).contains(&v) => vec![v],
        Some(_) => Vec::new(),
        None => (1..=31).filter(|v| field_matches(d, *v)).collect(),
    }
}

/// Whether a concrete value matches a field's digits, masked positions free.
fn field_matches(f: DateField, v: u8) -> bool {
    f.digits[0].is_none_or(|p| p == v / 10) && f.digits[1].is_none_or(|p| p == v % 10)
}

/// Decision D11: a (possibly masked) year-month-day needs at least one valid
/// calendar completion.
fn day_has_valid_completion(year: &YearKind, m: DateField, d: DateField) -> bool {
    let months = month_candidates(m);
    let days = day_candidates(d);
    let mut leap_possible: Option<bool> = None;
    for &mm in &months {
        for &dd in &days {
            if month_admits_day(mm, dd, year, &mut leap_possible) {
                return true;
            }
        }
    }
    false
}

/// Whether month `mm` can hold day `dd`. Deliberately a predicate rather than
/// a month-length function: February short-circuits to 28 for any day the
/// 28th admits, so leap-ness is only asked once a later day forces the
/// question, and `leap_possible` memoises that answer across the scan.
fn month_admits_day(mm: u8, dd: u8, year: &YearKind, leap_possible: &mut Option<bool>) -> bool {
    let max = match mm {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if dd <= 28 {
                28
            } else {
                let leap = *leap_possible.get_or_insert_with(|| year_leap_possible(year));
                if leap { 29 } else { 28 }
            }
        }
        _ => unreachable!("month candidates are 1-12"),
    };
    dd <= max
}

/// Whether any completion of this year could be a leap year.
fn year_leap_possible(year: &YearKind) -> bool {
    match year {
        YearKind::Standard { negative, digits } => {
            if digits.iter().all(Option::is_some) {
                let mut v: i64 = 0;
                for d in digits.iter().flatten() {
                    v = v * 10 + i64::from(*d);
                }
                is_leap(if *negative { -v } else { v })
            } else {
                (0..=9999_i64).any(|y| year_matches(*digits, y) && is_leap(y))
            }
        }
        // Y-prefixed years never carry months, so this is only reachable in
        // theory; be permissive.
        YearKind::Big { value } => is_leap(*value),
        YearKind::Exponential { .. } => true,
    }
}

/// Whether `y` matches the year's digits, masked positions free.
fn year_matches(digits: [Option<u8>; 4], y: i64) -> bool {
    let actual = [(y / 1000) % 10, (y / 100) % 10, (y / 10) % 10, y % 10];
    digits
        .iter()
        .zip(actual)
        .all(|(pat, a)| pat.is_none_or(|p| i64::from(p) == a))
}

/// `Y…` years (ISO 8601-2 §4.7.2-4.7.4): year precision only, |value| > 9999.
fn parse_prefixed_year(s: &str, base: usize) -> Result<Date, ParseError> {
    let mut c = Cur::new(s, base);
    c.eat(b'Y');
    let negative = c.eat(b'-');
    let value_off = c.pos();
    let (mantissa, mantissa_len) = c.take_number()?;
    if mantissa_len == 0 {
        return Err(c.fail("expected digits after 'Y'"));
    }
    // Leading zeros are only meaningful in four-digit years (P1 4.5); in a
    // Y-year they would also desynchronize the S-digit budget from the
    // canonical (zero-stripped) rendering (decision D20).
    if mantissa > 0 && mantissa_len as u64 != u64::from(mantissa.ilog10() + 1) {
        return Err(err(
            value_off,
            "leading zeros are not allowed in Y-prefixed years",
        ));
    }
    let (kind, digit_count) = if c.eat(b'E') {
        let exp_off = c.pos();
        let (exp, exp_len) = c.take_number()?;
        if exp_len == 0 {
            return Err(err(exp_off, "expected digits after exponent 'E'"));
        }
        if mantissa == 0 {
            return Err(err(
                value_off,
                "exponential year significand cannot be zero",
            ));
        }
        let Some(exponent) = u32::try_from(exp).ok().filter(|e| *e <= 100_000) else {
            return Err(err(exp_off, "exponent out of supported range"));
        };
        let significand = if negative { -mantissa } else { mantissa };
        // Reject values expressible as a plain four-digit year (decision D1).
        if let Some(v) = 10_i64
            .checked_pow(exponent)
            .and_then(|p| significand.checked_mul(p))
        {
            if v.abs() <= 9999 {
                return Err(err(value_off, "Y-prefixed years require |year| > 9999"));
            }
        }
        (
            YearKind::Exponential {
                significand,
                exponent,
            },
            mantissa_len as u64 + u64::from(exponent),
        )
    } else {
        if mantissa <= 9999 {
            return Err(err(value_off, "Y-prefixed years require |year| > 9999"));
        }
        (
            YearKind::Big {
                value: if negative { -mantissa } else { mantissa },
            },
            mantissa_len as u64,
        )
    };
    let significant = if c.eat(b'S') {
        let sig_off = c.pos();
        let (n, len) = c.take_number()?;
        let sig = u32::try_from(n)
            .ok()
            .filter(|s| *s >= 1 && u64::from(*s) <= digit_count);
        if len == 0 || sig.is_none() {
            return Err(err(
                sig_off,
                "significant digits exceed the year's digit count",
            ));
        }
        sig
    } else {
        None
    };
    let mut qualifier = Qualifier::default();
    if let Some(q) = c.take_qualifier() {
        qualifier.merge(q);
    }
    if !c.eof() {
        return Err(c.fail("Y-prefixed years are year-precision only"));
    }
    Ok(Date {
        year: Year {
            kind,
            significant_digits: significant,
            qualifier,
        },
        month: None,
        day: None,
    })
}

// ---------------------------------------------------------------- datetime

/// Parse a date-time expression.
fn parse_datetime(s: &str) -> Result<Edtf, ParseError> {
    // The dispatcher only routes strings containing 'T' here; a graceful
    // error beats a library panic if that invariant ever breaks.
    let Some((date_part, time_part)) = s.split_once('T') else {
        return Err(err(0, "datetime requires 'T'"));
    };
    let date = parse_plain_complete_date(date_part)?;
    let time = parse_time(time_part, date_part.len() + 1)?;
    Ok(Edtf::DateTime(DateTime { date, time }))
}

/// Date-times exist only at level 0: a plain, complete, unqualified
/// YYYY-MM-DD (Annex A.4.3).
fn parse_plain_complete_date(s: &str) -> Result<Date, ParseError> {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return Err(err(0, "date-times require a complete YYYY-MM-DD date"));
    }
    let digit = |i: usize| -> Result<u8, ParseError> {
        match b[i] {
            d @ b'0'..=b'9' => Ok(d - b'0'),
            _ => Err(err(i, "date-times require a plain all-digit date")),
        }
    };
    let ydigits = [
        Some(digit(0)?),
        Some(digit(1)?),
        Some(digit(2)?),
        Some(digit(3)?),
    ];
    let month = DateField {
        digits: [Some(digit(5)?), Some(digit(6)?)],
        qualifier: Qualifier::default(),
    };
    let day = DateField {
        digits: [Some(digit(8)?), Some(digit(9)?)],
        qualifier: Qualifier::default(),
    };
    check_plain_month(month)?;
    let kind = YearKind::Standard {
        negative: false,
        digits: ydigits,
    };
    if !day_has_valid_completion(&kind, month, day) {
        return Err(err(8, "day is out of range for the month"));
    }
    Ok(Date {
        year: Year {
            kind,
            significant_digits: None,
            qualifier: Qualifier::default(),
        },
        month: Some(month),
        day: Some(day),
    })
}

/// Validate the month of a date-time: always a plain calendar month at the
/// fixed offset 5 of `YYYY-MM-DD`. The range check is live — `2020-13-01T…`
/// lands on it — while the unspecified-digit arm above it is defensive:
/// [`parse_plain_complete_date`] builds both digits itself, so only some
/// future caller could take that arm.
fn check_plain_month(month: DateField) -> Result<(), ParseError> {
    // Both digits were just parsed; a graceful error beats a panic if that
    // invariant ever breaks.
    let Some(m) = month.value() else {
        return Err(err(5, "month must be fully specified"));
    };
    if !(1..=12).contains(&m) {
        return Err(err(5, "month must be 01-12"));
    }
    Ok(())
}

/// Parse the time half of a date-time.
fn parse_time(s: &str, base: usize) -> Result<Time, ParseError> {
    let b = s.as_bytes();
    if b.len() < 8 || b[2] != b':' || b[5] != b':' {
        return Err(err(base, "time must be hh:mm:ss"));
    }
    let two = |i: usize| -> Result<u8, ParseError> {
        match (b[i], b[i + 1]) {
            (t @ b'0'..=b'9', o @ b'0'..=b'9') => Ok((t - b'0') * 10 + (o - b'0')),
            _ => Err(err(base + i, "time components must be two digits")),
        }
    };
    let hour = two(0)?;
    let minute = two(3)?;
    let second = two(6)?;
    if hour > 23 {
        return Err(err(base, "hour must be 00-23 (24 is not allowed)"));
    }
    if minute > 59 {
        return Err(err(base + 3, "minute must be 00-59"));
    }
    // 60 admits a leap second; not verified against the leap-second table
    // (decision D3).
    if second > 60 {
        return Err(err(base + 6, "second must be 00-60"));
    }
    let rest = &s[8..];
    let shift = if rest.is_empty() {
        None
    } else if rest == "Z" {
        Some(TimeShift::Utc)
    } else {
        Some(parse_shift(rest, base + 8)?)
    };
    Ok(Time {
        hour,
        minute,
        second,
        shift,
    })
}

/// Parse a UTC designator or numeric time-zone shift.
fn parse_shift(s: &str, base: usize) -> Result<TimeShift, ParseError> {
    let b = s.as_bytes();
    let negative = match b[0] {
        b'+' => false,
        b'-' => true,
        _ => return Err(err(base, "time shift must be Z, +hh, +hh:mm or negative")),
    };
    let (hours, minutes, hours_only) = match b.len() {
        3 => {
            let h = shift_two(b, 1, base)?;
            (h, 0, true)
        }
        6 if b[3] == b':' => {
            let h = shift_two(b, 1, base)?;
            let m = shift_two(b, 4, base)?;
            (h, m, false)
        }
        _ => return Err(err(base, "time shift must be \u{b1}hh or \u{b1}hh:mm")),
    };
    if minutes > 59 {
        return Err(err(base + 4, "shift minutes must be 00-59"));
    }
    let total = i16::from(hours) * 60 + i16::from(minutes);
    if total > 14 * 60 {
        return Err(err(base, "time shift exceeds \u{b1}14:00"));
    }
    if negative && total == 0 {
        return Err(err(base, "negative zero time shift is not allowed"));
    }
    Ok(TimeShift::Offset {
        minutes: if negative { -total } else { total },
        hours_only,
    })
}

/// Read a two-digit shift component at `i`.
fn shift_two(b: &[u8], i: usize, base: usize) -> Result<u8, ParseError> {
    match (b[i], b[i + 1]) {
        (t @ b'0'..=b'9', o @ b'0'..=b'9') => Ok((t - b'0') * 10 + (o - b'0')),
        _ => Err(err(base + i, "time shift components must be two digits")),
    }
}

// ---------------------------------------------------------------- intervals

/// Parse an interval expression.
fn parse_interval(s: &str) -> Result<Edtf, ParseError> {
    let mut parts = s.splitn(3, '/');
    let start_str = parts.next().unwrap_or("");
    let end_str = parts
        .next()
        .ok_or_else(|| err(s.len(), "interval requires '/'"))?;
    if parts.next().is_some() {
        return Err(err(
            offset_in(s, end_str) + end_str.len(),
            "interval must contain exactly one '/'",
        ));
    }
    let start = parse_endpoint(start_str, offset_in(s, start_str), true)?;
    let end = parse_endpoint(end_str, offset_in(s, end_str), false)?;
    if !start.is_dated() && !end.is_dated() {
        return Err(err(0, "interval needs at least one dated endpoint"));
    }
    // The end must not precede the start (decision D18; cf. ISO 8601-2
    // §7.14.2 Example 2). Comparable only when both endpoints are dates.
    if let (IntervalEndpoint::Date(a), IntervalEndpoint::Date(b)) = (&start, &end) {
        if dates_out_of_order(a, b) {
            return Err(err(
                offset_in(s, end_str),
                "interval end precedes interval start",
            ));
        }
    }
    Ok(Edtf::Interval(Interval { start, end }))
}

/// Parse one interval endpoint.
fn parse_endpoint(s: &str, base: usize, is_start: bool) -> Result<IntervalEndpoint, ParseError> {
    if s.is_empty() {
        return Ok(IntervalEndpoint::Unknown);
    }
    if s == ".." {
        return Ok(IntervalEndpoint::Open);
    }
    if let Some(rest) = s.strip_prefix("..") {
        if !is_start {
            return Err(err(
                base,
                "'..'-prefixed date is only allowed as interval start",
            ));
        }
        return Ok(IntervalEndpoint::OnOrBefore(parse_date_at(rest, base + 2)?));
    }
    if let Some(rest) = s.strip_suffix("..") {
        if is_start {
            return Err(err(
                base + rest.len(),
                "'..'-suffixed date is only allowed as interval end",
            ));
        }
        return Ok(IntervalEndpoint::OnOrAfter(parse_date_at(rest, base)?));
    }
    Ok(IntervalEndpoint::Date(parse_date_at(s, base)?))
}

/// True when `a` cannot possibly precede-or-touch `b`: a's earliest possible
/// day is later than b's latest possible day.
fn dates_out_of_order(a: &Date, b: &Date) -> bool {
    match (date_bounds(a).earliest, date_bounds(b).latest) {
        (Bound::Date(lo), Bound::Date(hi)) => lo > hi,
        _ => false,
    }
}

// ---------------------------------------------------------------- sets

/// Parse a set expression.
fn parse_set(s: &str) -> Result<Edtf, ParseError> {
    let b = s.as_bytes();
    let (kind, close) = match b[0] {
        b'{' => (SetKind::AllMembers, b'}'),
        _ => (SetKind::OneMember, b']'),
    };
    if b.len() < 2 || b[b.len() - 1] != close {
        return Err(err(s.len().saturating_sub(1), "unterminated set"));
    }
    let inner = &s[1..s.len() - 1];
    if inner.is_empty() {
        return Err(err(1, "set must contain at least one element"));
    }
    if let Some(pos) = inner
        .bytes()
        .position(|c| matches!(c, b'{' | b'}' | b'[' | b']'))
    {
        return Err(err(1 + pos, "sets cannot nest"));
    }
    let mut elements = Vec::new();
    for part in inner.split(',') {
        let part_off = offset_in(s, part);
        if part.is_empty() {
            return Err(err(part_off, "empty set element"));
        }
        elements.push(parse_set_element(part, part_off)?);
    }
    Ok(Edtf::Set(Set { kind, elements }))
}

/// Parse one set element, which may itself be a range.
fn parse_set_element(p: &str, base: usize) -> Result<SetElement, ParseError> {
    if p == ".." {
        return Err(err(base, "'..' alone is not a set element"));
    }
    if let Some(rest) = p.strip_prefix("..") {
        return Ok(SetElement::OnOrBefore(parse_date_at(rest, base + 2)?));
    }
    if let Some(rest) = p.strip_suffix("..") {
        return Ok(SetElement::OnOrAfter(parse_date_at(rest, base)?));
    }
    if let Some(idx) = p.find("..") {
        let (a, b) = (&p[..idx], &p[idx + 2..]);
        if b.contains("..") {
            return Err(err(
                base + idx + 2,
                "set element cannot contain multiple '..'",
            ));
        }
        let from = parse_date_at(a, base)?;
        let to = parse_date_at(b, base + idx + 2)?;
        check_range_endpoint(&from, base)?;
        check_range_endpoint(&to, base + idx + 2)?;
        if from.precision() != to.precision() {
            return Err(err(
                base + idx + 2,
                "set range endpoints must share precision",
            ));
        }
        if dates_out_of_order(&from, &to) {
            return Err(err(base + idx + 2, "set range end precedes range start"));
        }
        return Ok(SetElement::Range(from, to));
    }
    Ok(SetElement::Date(parse_date_at(p, base)?))
}

/// A `..` range endpoint must denote a single concrete calendar value at
/// year, month or day precision (decision D27): every expansion example in
/// ISO 8601-2 §6.3 c/§6.4 uses plain dates, both sides "should be of the
/// same precision", a mask-carrying endpoint would make the range a relation
/// between value *sets*, seasons have no spec-defined successor, and a
/// qualifier on an endpoint has no defined spread over the expanded members.
fn check_range_endpoint(d: &Date, off: usize) -> Result<(), ParseError> {
    if d.has_unspecified() {
        return Err(err(
            off,
            "set range endpoints cannot contain unspecified digits",
        ));
    }
    if d.is_uncertain() || d.is_approximate() {
        return Err(err(off, "set range endpoints cannot be qualified"));
    }
    if d.precision() == Precision::Season {
        return Err(err(off, "set range endpoints cannot be seasons"));
    }
    if d.year.significant_digits.is_some() {
        return Err(err(
            off,
            "set range endpoints cannot carry significant digits",
        ));
    }
    Ok(())
}

// ------------------------------------------------------------ guard tests
//
// The parser's own callers already exclude the inputs below, so these
// defensive arms are unreachable through `parse`. They are the contract for
// misuse of the private/`pub(crate)` entry points — a positioned error or a
// panic, never a silent wrong answer — and only a white-box test can pin it.

#[cfg(test)]
mod tests {
    #![expect(
        clippy::missing_panics_doc,
        reason = "a test asserts by panicking; that is the failure signal, so there is no caller to warn"
    )]
    #![expect(
        clippy::let_underscore_untyped,
        reason = "the discarded value's type is fixed by the call it comes from"
    )]
    #![expect(
        clippy::inline_modules,
        reason = "a module small enough to read in place belongs in place"
    )]
    #![allow(
        clippy::unwrap_used,
        reason = "test code: a panic here is the failure signal, not a crash path"
    )]

    use super::*;

    /// Every caller filters empty slices first; the fallback must still be a
    /// positioned error rather than an index panic on the first byte.
    #[test]
    fn empty_date_slice_errors_at_its_base_offset() {
        let e = parse_date_at("", 7).unwrap_err();
        assert_eq!(e.message, "empty date");
        assert_eq!(e.offset, 7);
    }

    /// A sub-year code where a calendar month belongs: no calendar month
    /// matches it, and the answer is an empty candidate list, not a panic.
    #[test]
    fn month_candidates_of_a_sub_year_code_are_empty() {
        let spring = DateField {
            digits: [Some(2), Some(1)],
            qualifier: Qualifier::default(),
        };
        assert!(month_candidates(spring).is_empty());
    }

    /// The day-completion scan only ever passes months from
    /// `month_candidates`; anything else is misuse and must panic rather
    /// than pick some month length.
    #[test]
    #[should_panic(expected = "month candidates are 1-12")]
    fn month_admits_day_rejects_a_month_outside_1_12() {
        let year = YearKind::Standard {
            negative: false,
            digits: [Some(2), Some(0), Some(0), Some(1)],
        };
        let _ = month_admits_day(21, 1, &year, &mut None);
    }

    /// `Y`-prefixed years never carry a month, so leap-ness is only asked of
    /// them in theory; a big year answers for its own value.
    #[test]
    fn big_year_leap_possible_follows_the_value() {
        assert!(year_leap_possible(&YearKind::Big { value: 20_000 }));
        assert!(!year_leap_possible(&YearKind::Big { value: 20_001 }));
        assert!(year_leap_possible(&YearKind::Big { value: -20_000 }));
    }

    /// An exponential year stands for a value the model never pins down, so
    /// the permissive answer keeps a 29 February completion available. 17E2 is
    /// 1700, a century year the real leap rule rejects, so only the
    /// unconditional arm can satisfy it; the spec's own 17E7 example is
    /// 170 000 000, divisible by 400, and would pass under either reading.
    #[test]
    fn exponential_year_is_permissively_leap() {
        assert!(year_leap_possible(&YearKind::Exponential {
            significand: 17,
            exponent: 2,
        }));
        assert!(year_leap_possible(&YearKind::Exponential {
            significand: 17,
            exponent: 7,
        }));
    }

    /// A digit run too wide for `i64` is real input, not only a guard: it has
    /// to be reported at the run's first digit rather than wrap around.
    #[test]
    fn an_oversized_digit_run_errors_at_the_first_digit() {
        let e = parse("Y99999999999999999999").unwrap_err();
        assert_eq!(e.message, "number out of supported range");
        assert_eq!(e.offset, 1);
    }

    /// The dispatcher only routes strings containing `/` here; without one the
    /// missing second half is reported at end of input.
    #[test]
    fn interval_without_a_slash_errors_at_end_of_input() {
        let e = parse_interval("1985").unwrap_err();
        assert_eq!(e.message, "interval requires '/'");
        assert_eq!(e.offset, 4);
    }

    /// The dispatcher only routes `T`-bearing strings here; without one the
    /// split fails and the error must say so.
    #[test]
    fn datetime_without_t_errors() {
        let e = parse_datetime("1985-04-12").unwrap_err();
        assert_eq!(e.message, "datetime requires 'T'");
        assert_eq!(e.offset, 0);
    }

    /// `parse_plain_complete_date` builds both month digits itself, so only a
    /// direct call reaches the unspecified-digit arm; the other two outcomes
    /// are pinned alongside it to hold the whole check in place.
    #[test]
    fn plain_month_check_covers_masked_and_out_of_range_months() {
        let field = |digits| DateField {
            digits,
            qualifier: Qualifier::default(),
        };

        let masked = check_plain_month(field([None, Some(4)])).unwrap_err();
        assert_eq!(masked.message, "month must be fully specified");
        assert_eq!(masked.offset, 5);

        let thirteen = check_plain_month(field([Some(1), Some(3)])).unwrap_err();
        assert_eq!(thirteen.message, "month must be 01-12");
        assert_eq!(thirteen.offset, 5);

        check_plain_month(field([Some(0), Some(4)])).unwrap();
    }
}
