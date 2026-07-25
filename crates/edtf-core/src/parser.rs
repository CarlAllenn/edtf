//! Hand-written recursive-descent parser and validator for EDTF levels 0–2.
//!
//! Grammar and validation rules follow docs/spec-notes.md, which cites the
//! ISO 8601-1/-2:2019 sections for every production. Open decisions D1–D17
//! are implemented as proposed there.

use crate::bounds::{date_bounds, is_leap, Bound};
use crate::types::*;
use alloc::vec;
use alloc::vec::Vec;

pub(crate) fn parse(input: &str) -> Result<Edtf, ParseError> {
    if input.is_empty() {
        return Err(err("empty input"));
    }
    if !input.is_ascii() {
        return Err(err("EDTF permits ASCII characters only"));
    }
    if input.bytes().any(|b| b == b' ' || b.is_ascii_control()) {
        return Err(err("whitespace is not allowed"));
    }
    match input.as_bytes()[0] {
        b'{' | b'[' => parse_set(input),
        _ if input.contains('/') => parse_interval(input),
        _ if input.contains('T') => parse_datetime(input),
        _ => parse_date(input).map(Edtf::Date),
    }
}

fn err(message: &'static str) -> ParseError {
    ParseError { message }
}

// ---------------------------------------------------------------- cursor

struct Cur<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Cur<'a> {
    fn new(s: &'a str) -> Self {
        Self {
            b: s.as_bytes(),
            i: 0,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let c = self.peek();
        if c.is_some() {
            self.i += 1;
        }
        c
    }

    fn eat(&mut self, c: u8) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn eof(&self) -> bool {
        self.i >= self.b.len()
    }

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
                .ok_or_else(|| err("number out of supported range"))?;
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
                    })
                }
            }
        }
        Ok(out)
    }
}

// ---------------------------------------------------------------- dates

pub(crate) fn parse_date(s: &str) -> Result<Date, ParseError> {
    if s.is_empty() {
        return Err(err("empty date"));
    }
    if s.as_bytes()[0] == b'Y' {
        return parse_prefixed_year(s);
    }
    let mut c = Cur::new(s);

    // Year: optional individual qualifier, optional '-', exactly four digit/X.
    let mut year_qual = Qualifier::default();
    if let Some(q) = c.take_qualifier() {
        year_qual.merge(q);
    }
    let negative = c.eat(b'-');
    let mut ydigits = [None; 4];
    for slot in &mut ydigits {
        match c.bump() {
            Some(d @ b'0'..=b'9') => *slot = Some(d - b'0'),
            Some(b'X') => *slot = None,
            _ => return Err(err("year must have exactly four digits (or X)")),
        }
    }
    let year_has_x = ydigits.iter().any(|d| d.is_none());
    if negative && year_has_x {
        return Err(err("negative years cannot contain unspecified digits"));
    }
    if negative && ydigits == [Some(0); 4] {
        return Err(err("-0000 is not a valid year"));
    }

    // Optional S<digits> significant-digit suffix (year-precision only).
    let mut significant = None;
    if c.eat(b'S') {
        if year_has_x {
            return Err(err(
                "significant digits cannot combine with unspecified digits",
            ));
        }
        let (n, len) = c.take_number()?;
        if len == 0 || n == 0 || n > 4 {
            return Err(err("significant digits must be 1-4 for a four-digit year"));
        }
        significant = Some(n as u32);
    }

    // Optional trailing qualifier (group: year, or complete if year-only).
    if let Some(q) = c.take_qualifier() {
        year_qual.merge(q);
    }

    if c.eof() {
        return finish_date(negative, ydigits, significant, year_qual, None, None);
    }
    if significant.is_some() {
        return Err(err("significant-digit years are year-precision only"));
    }
    if !c.eat(b'-') {
        return Err(err("expected '-' before month"));
    }

    // Month.
    let mut month_qual = Qualifier::default();
    if let Some(q) = c.take_qualifier() {
        month_qual.merge(q);
    }
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
        return finish_date(negative, ydigits, None, year_qual, Some(month), None);
    }
    if !c.eat(b'-') {
        return Err(err("expected '-' before day"));
    }

    // Day.
    let mut day_qual = Qualifier::default();
    if let Some(q) = c.take_qualifier() {
        day_qual.merge(q);
    }
    let day_digits = c.take_two("day")?;
    if let Some(q) = c.take_qualifier() {
        // Trailing qualifier after the last component = complete qualification.
        day_qual.merge(q);
        month.qualifier.merge(q);
        year_qual.merge(q);
    }
    if !c.eof() {
        return Err(err("unexpected characters after day"));
    }
    let day = DateField {
        digits: day_digits,
        qualifier: day_qual,
    };
    finish_date(negative, ydigits, None, year_qual, Some(month), Some(day))
}

fn finish_date(
    negative: bool,
    ydigits: [Option<u8>; 4],
    significant: Option<u32>,
    year_qual: Qualifier,
    month: Option<DateField>,
    day: Option<DateField>,
) -> Result<Date, ParseError> {
    let year = Year {
        kind: YearKind::Standard {
            negative,
            digits: ydigits,
        },
        significant_digits: significant,
        qualifier: year_qual,
    };
    validate_month_day(&year.kind, month.as_ref(), day.as_ref())?;
    Ok(Date { year, month, day })
}

fn validate_month_day(
    year: &YearKind,
    month: Option<&DateField>,
    day: Option<&DateField>,
) -> Result<(), ParseError> {
    let Some(m) = month else {
        return Ok(());
    };
    match m.value() {
        Some(v) => {
            if !((1..=12).contains(&v) || (21..=41).contains(&v)) {
                return Err(err("month must be 01-12 or a sub-year code 21-41"));
            }
            if (21..=41).contains(&v) && day.is_some() {
                return Err(err("sub-year groupings cannot carry a day"));
            }
        }
        None => {
            // Masked months match calendar months 01-12 only (decision D14);
            // sub-year codes must be written explicitly.
            if month_candidates(m).is_empty() {
                return Err(err("no calendar month matches the unspecified digits"));
            }
        }
    }
    if let Some(d) = day {
        if !day_has_valid_completion(year, m, d) {
            return Err(err("day is out of range for the month"));
        }
    }
    Ok(())
}

fn month_candidates(m: &DateField) -> Vec<u8> {
    match m.value() {
        Some(v) if (1..=12).contains(&v) => vec![v],
        Some(_) => Vec::new(),
        None => (1..=12).filter(|v| field_matches(m, *v)).collect(),
    }
}

fn day_candidates(d: &DateField) -> Vec<u8> {
    match d.value() {
        Some(v) if (1..=31).contains(&v) => vec![v],
        Some(_) => Vec::new(),
        None => (1..=31).filter(|v| field_matches(d, *v)).collect(),
    }
}

fn field_matches(f: &DateField, v: u8) -> bool {
    let tens = v / 10;
    let ones = v % 10;
    f.digits[0].is_none_or(|p| p == tens) && f.digits[1].is_none_or(|p| p == ones)
}

/// Decision D11: a (possibly masked) year-month-day needs at least one valid
/// calendar completion.
fn day_has_valid_completion(year: &YearKind, m: &DateField, d: &DateField) -> bool {
    let months = month_candidates(m);
    let days = day_candidates(d);
    let mut leap_possible: Option<bool> = None;
    for &mm in &months {
        for &dd in &days {
            let max = match mm {
                1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
                4 | 6 | 9 | 11 => 30,
                2 => {
                    if dd <= 28 {
                        28
                    } else {
                        let leap = *leap_possible.get_or_insert_with(|| year_leap_possible(year));
                        if leap {
                            29
                        } else {
                            28
                        }
                    }
                }
                _ => unreachable!("month candidates are 1-12"),
            };
            if dd <= max {
                return true;
            }
        }
    }
    false
}

fn year_leap_possible(year: &YearKind) -> bool {
    match year {
        YearKind::Standard { negative, digits } => {
            if digits.iter().all(|d| d.is_some()) {
                let mut v: i64 = 0;
                for d in digits.iter().flatten() {
                    v = v * 10 + i64::from(*d);
                }
                is_leap(if *negative { -v } else { v })
            } else {
                (0..=9999i64).any(|y| year_matches(digits, y) && is_leap(y))
            }
        }
        // Y-prefixed years never carry months, so this is only reachable in
        // theory; be permissive.
        YearKind::Big { value } => is_leap(*value),
        YearKind::Exponential { .. } => true,
    }
}

fn year_matches(digits: &[Option<u8>; 4], y: i64) -> bool {
    let actual = [(y / 1000) % 10, (y / 100) % 10, (y / 10) % 10, y % 10];
    digits
        .iter()
        .zip(actual)
        .all(|(pat, a)| pat.is_none_or(|p| i64::from(p) == a))
}

/// `Y…` years (ISO 8601-2 §4.7.2-4.7.4): year precision only, |value| > 9999.
fn parse_prefixed_year(s: &str) -> Result<Date, ParseError> {
    let mut c = Cur::new(s);
    c.eat(b'Y');
    let negative = c.eat(b'-');
    let (mantissa, mantissa_len) = c.take_number()?;
    if mantissa_len == 0 {
        return Err(err("expected digits after 'Y'"));
    }
    let kind;
    let digit_count: u64;
    if c.eat(b'E') {
        let (exp, exp_len) = c.take_number()?;
        if exp_len == 0 {
            return Err(err("expected digits after exponent 'E'"));
        }
        if mantissa == 0 {
            return Err(err("exponential year significand cannot be zero"));
        }
        if exp > 100_000 {
            return Err(err("exponent out of supported range"));
        }
        let significand = if negative { -mantissa } else { mantissa };
        let exponent = exp as u32;
        // Reject values expressible as a plain four-digit year (decision D1).
        if let Some(v) = 10i64
            .checked_pow(exponent)
            .and_then(|p| significand.checked_mul(p))
        {
            if v.abs() <= 9999 {
                return Err(err("Y-prefixed years require |year| > 9999"));
            }
        }
        kind = YearKind::Exponential {
            significand,
            exponent,
        };
        digit_count = mantissa_len as u64 + u64::from(exponent);
    } else {
        if mantissa <= 9999 {
            return Err(err("Y-prefixed years require |year| > 9999"));
        }
        kind = YearKind::Big {
            value: if negative { -mantissa } else { mantissa },
        };
        digit_count = mantissa_len as u64;
    }
    let mut significant = None;
    if c.eat(b'S') {
        let (n, len) = c.take_number()?;
        if len == 0 || n == 0 || (n as u64) > digit_count {
            return Err(err("significant digits exceed the year's digit count"));
        }
        significant = Some(n as u32);
    }
    let mut qualifier = Qualifier::default();
    if let Some(q) = c.take_qualifier() {
        qualifier.merge(q);
    }
    if !c.eof() {
        return Err(err("Y-prefixed years are year-precision only"));
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

fn parse_datetime(s: &str) -> Result<Edtf, ParseError> {
    let (date_part, time_part) = s.split_once('T').expect("caller checked for 'T'");
    let date = parse_plain_complete_date(date_part)?;
    let time = parse_time(time_part)?;
    Ok(Edtf::DateTime(DateTime { date, time }))
}

/// Date-times exist only at level 0: a plain, complete, unqualified
/// YYYY-MM-DD (Annex A.4.3).
fn parse_plain_complete_date(s: &str) -> Result<Date, ParseError> {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return Err(err("date-times require a complete YYYY-MM-DD date"));
    }
    let digit = |i: usize| -> Result<u8, ParseError> {
        match b[i] {
            d @ b'0'..=b'9' => Ok(d - b'0'),
            _ => Err(err("date-times require a plain all-digit date")),
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
    let m = month.value().expect("fully specified");
    if !(1..=12).contains(&m) {
        return Err(err("month must be 01-12"));
    }
    let kind = YearKind::Standard {
        negative: false,
        digits: ydigits,
    };
    if !day_has_valid_completion(&kind, &month, &day) {
        return Err(err("day is out of range for the month"));
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

fn parse_time(s: &str) -> Result<Time, ParseError> {
    let b = s.as_bytes();
    if b.len() < 8 || b[2] != b':' || b[5] != b':' {
        return Err(err("time must be hh:mm:ss"));
    }
    let two = |i: usize| -> Result<u8, ParseError> {
        match (b[i], b[i + 1]) {
            (t @ b'0'..=b'9', o @ b'0'..=b'9') => Ok((t - b'0') * 10 + (o - b'0')),
            _ => Err(err("time components must be two digits")),
        }
    };
    let hour = two(0)?;
    let minute = two(3)?;
    let second = two(6)?;
    if hour > 23 {
        return Err(err("hour must be 00-23 (24 is not allowed)"));
    }
    if minute > 59 {
        return Err(err("minute must be 00-59"));
    }
    // 60 admits a leap second; not verified against the leap-second table
    // (decision D3).
    if second > 60 {
        return Err(err("second must be 00-60"));
    }
    let rest = &s[8..];
    let shift = if rest.is_empty() {
        None
    } else if rest == "Z" {
        Some(TimeShift::Utc)
    } else {
        Some(parse_shift(rest)?)
    };
    Ok(Time {
        hour,
        minute,
        second,
        shift,
    })
}

fn parse_shift(s: &str) -> Result<TimeShift, ParseError> {
    let b = s.as_bytes();
    let negative = match b[0] {
        b'+' => false,
        b'-' => true,
        _ => return Err(err("time shift must be Z, +hh, +hh:mm or negative")),
    };
    let (hours, minutes, hours_only) = match b.len() {
        3 => {
            let h = shift_two(b, 1)?;
            (h, 0, true)
        }
        6 if b[3] == b':' => {
            let h = shift_two(b, 1)?;
            let m = shift_two(b, 4)?;
            (h, m, false)
        }
        _ => return Err(err("time shift must be ±hh or ±hh:mm")),
    };
    if minutes > 59 {
        return Err(err("shift minutes must be 00-59"));
    }
    let total = i16::from(hours) * 60 + i16::from(minutes);
    if total > 14 * 60 {
        return Err(err("time shift exceeds ±14:00"));
    }
    if negative && total == 0 {
        return Err(err("negative zero time shift is not allowed"));
    }
    Ok(TimeShift::Offset {
        minutes: if negative { -total } else { total },
        hours_only,
    })
}

fn shift_two(b: &[u8], i: usize) -> Result<u8, ParseError> {
    match (b[i], b[i + 1]) {
        (t @ b'0'..=b'9', o @ b'0'..=b'9') => Ok((t - b'0') * 10 + (o - b'0')),
        _ => Err(err("time shift components must be two digits")),
    }
}

// ---------------------------------------------------------------- intervals

fn parse_interval(s: &str) -> Result<Edtf, ParseError> {
    let mut parts = s.splitn(3, '/');
    let start_str = parts.next().expect("split yields at least one part");
    let end_str = parts.next().ok_or_else(|| err("interval requires '/'"))?;
    if parts.next().is_some() {
        return Err(err("interval must contain exactly one '/'"));
    }
    let start = parse_endpoint(start_str, true)?;
    let end = parse_endpoint(end_str, false)?;
    if !start.is_dated() && !end.is_dated() {
        return Err(err("interval needs at least one dated endpoint"));
    }
    // The end must not precede the start (decision D18; cf. ISO 8601-2
    // §7.14.2 Example 2). Comparable only when both endpoints are dates.
    if let (IntervalEndpoint::Date(s), IntervalEndpoint::Date(e)) = (&start, &end) {
        if dates_out_of_order(s, e) {
            return Err(err("interval end precedes interval start"));
        }
    }
    Ok(Edtf::Interval(Interval { start, end }))
}

fn parse_endpoint(s: &str, is_start: bool) -> Result<IntervalEndpoint, ParseError> {
    if s.is_empty() {
        return Ok(IntervalEndpoint::Unknown);
    }
    if s == ".." {
        return Ok(IntervalEndpoint::Open);
    }
    if let Some(rest) = s.strip_prefix("..") {
        if !is_start {
            return Err(err("'..'-prefixed date is only allowed as interval start"));
        }
        return Ok(IntervalEndpoint::OnOrBefore(parse_date(rest)?));
    }
    if let Some(rest) = s.strip_suffix("..") {
        if is_start {
            return Err(err("'..'-suffixed date is only allowed as interval end"));
        }
        return Ok(IntervalEndpoint::OnOrAfter(parse_date(rest)?));
    }
    Ok(IntervalEndpoint::Date(parse_date(s)?))
}

// ---------------------------------------------------------------- sets

fn parse_set(s: &str) -> Result<Edtf, ParseError> {
    let b = s.as_bytes();
    let (kind, close) = match b[0] {
        b'{' => (SetKind::AllMembers, b'}'),
        _ => (SetKind::OneMember, b']'),
    };
    if b.len() < 2 || b[b.len() - 1] != close {
        return Err(err("unterminated set"));
    }
    let inner = &s[1..s.len() - 1];
    if inner.is_empty() {
        return Err(err("set must contain at least one element"));
    }
    if inner
        .bytes()
        .any(|c| matches!(c, b'{' | b'}' | b'[' | b']'))
    {
        return Err(err("sets cannot nest"));
    }
    let mut elements = Vec::new();
    for part in inner.split(',') {
        if part.is_empty() {
            return Err(err("empty set element"));
        }
        elements.push(parse_set_element(part)?);
    }
    Ok(Edtf::Set(Set { kind, elements }))
}

fn parse_set_element(p: &str) -> Result<SetElement, ParseError> {
    if p == ".." {
        return Err(err("'..' alone is not a set element"));
    }
    if let Some(rest) = p.strip_prefix("..") {
        return Ok(SetElement::OnOrBefore(parse_date(rest)?));
    }
    if let Some(rest) = p.strip_suffix("..") {
        return Ok(SetElement::OnOrAfter(parse_date(rest)?));
    }
    if let Some(idx) = p.find("..") {
        let (a, b) = (&p[..idx], &p[idx + 2..]);
        if b.contains("..") {
            return Err(err("set element cannot contain multiple '..'"));
        }
        let (from, to) = (parse_date(a)?, parse_date(b)?);
        if dates_out_of_order(&from, &to) {
            return Err(err("set range end precedes range start"));
        }
        return Ok(SetElement::Range(from, to));
    }
    Ok(SetElement::Date(parse_date(p)?))
}

/// True when `a` cannot possibly precede-or-touch `b`: a's earliest possible
/// day is later than b's latest possible day.
fn dates_out_of_order(a: &Date, b: &Date) -> bool {
    match (date_bounds(a).earliest, date_bounds(b).latest) {
        (Bound::Date(lo), Bound::Date(hi)) => lo > hi,
        _ => false,
    }
}
