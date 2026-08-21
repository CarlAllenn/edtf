// SPDX-FileCopyrightText: Copyright (c) the edtf contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Canonical formatting: `Display` renders every expression in the
//! spec-preferred form (ISO 8601-2 §8.2.4 — complete qualification over
//! group, group over individual, no redundant qualifiers).
//!
//! Round-trip property: for any valid input, `parse(format(parse(input)))`
//! yields a semantically identical value; spellings that mean the same
//! thing (`?2004-?06-?11` vs `2004-06-11?`) normalize to one form.

#![expect(
    clippy::min_ident_chars,
    reason = "y, m and d are the universal notation for a date's components, and this crate is about little else"
)]

use alloc::{string::String, vec::Vec};
use core::fmt::{self, Display, Formatter, Write as _};

use crate::types::{
    Date, DateField, DateTime, Edtf, Interval, IntervalEndpoint, Qualifier, Set, SetElement,
    SetKind, Time, TimeShift, Year, YearKind,
};

impl Display for Edtf {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Date(d) => d.fmt(f),
            Self::DateTime(dt) => dt.fmt(f),
            Self::Interval(iv) => iv.fmt(f),
            Self::Set(s) => s.fmt(f),
        }
    }
}

fn qual_symbol(q: Qualifier) -> char {
    match (q.uncertain, q.approximate) {
        (true, true) => '%',
        (true, false) => '?',
        (false, true) => '~',
        (false, false) => unreachable!("caller checks is_qualified"),
    }
}

fn year_body(year: &Year) -> String {
    let mut s = String::new();
    match year.kind {
        YearKind::Standard { negative, digits } => {
            if negative {
                s.push('-');
            }
            for d in digits {
                match d {
                    Some(v) => s.push((b'0' + v) as char),
                    None => s.push('X'),
                }
            }
        }
        YearKind::Big { value } => {
            let _ = write!(s, "Y{value}");
        }
        YearKind::Exponential {
            significand,
            exponent,
        } => {
            let _ = write!(s, "Y{significand}E{exponent}");
        }
    }
    if let Some(p) = year.significant_digits {
        let _ = write!(s, "S{p}");
    }
    s
}

fn field_body(f: DateField) -> String {
    let mut s = String::new();
    for d in f.digits {
        match d {
            Some(v) => s.push((b'0' + v) as char),
            None => s.push('X'),
        }
    }
    s
}

/// Index of the last leading part sharing the year's qualifier `q0` — the
/// reach of one group marker. The `Display` impl for `Date` returns early for
/// fully uniform dates, so in practice the scan always stops at an unequal
/// part; the length guard is what keeps it total for any slice, including one
/// whose every part shares `q0`.
fn qualified_prefix_end(parts: &[(String, Qualifier)], q0: Qualifier) -> usize {
    let mut end = 0;
    while end + 1 < parts.len() && parts[end + 1].1 == q0 {
        end += 1;
    }
    end
}

impl Display for Date {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut parts: Vec<(String, Qualifier)> = Vec::new();
        parts.push((year_body(&self.year), self.year.qualifier));
        if let Some(m) = &self.month {
            parts.push((field_body(*m), m.qualifier));
        }
        if let Some(d) = &self.day {
            parts.push((field_body(*d), d.qualifier));
        }

        let all_equal = parts.iter().all(|(_, q)| *q == parts[0].1);
        if all_equal {
            // Unqualified, or complete qualification: one trailing symbol.
            let q = parts[0].1;
            for (i, (body, _)) in parts.iter().enumerate() {
                if i > 0 {
                    f.write_char('-')?;
                }
                f.write_str(body)?;
            }
            if q.is_qualified() {
                f.write_char(qual_symbol(q))?;
            }
            return Ok(());
        }

        // Longest qualified prefix sharing the year's qualifier becomes one
        // group marker; anything after it gets individual (left) qualifiers.
        let q0 = parts[0].1;
        let prefix_end = q0.is_qualified().then(|| qualified_prefix_end(&parts, q0));
        for (i, (body, q)) in parts.iter().enumerate() {
            if i > 0 {
                f.write_char('-')?;
            }
            let in_prefix = prefix_end.is_some_and(|end| i <= end);
            if !in_prefix && q.is_qualified() {
                f.write_char(qual_symbol(*q))?;
            }
            f.write_str(body)?;
            if prefix_end == Some(i) {
                f.write_char(qual_symbol(q0))?;
            }
        }
        Ok(())
    }
}

impl Display for Time {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{:02}:{:02}:{:02}", self.hour, self.minute, self.second)?;
        match self.shift {
            None => Ok(()),
            Some(TimeShift::Utc) => f.write_char('Z'),
            Some(TimeShift::Offset {
                minutes,
                hours_only,
            }) => {
                let sign = if minutes < 0 { '-' } else { '+' };
                let mag = minutes.unsigned_abs();
                let (h, m) = (mag / 60, mag % 60);
                // The parser only sets `hours_only` for ±hh forms, whose
                // minutes are zero by construction.
                if hours_only && m == 0 {
                    write!(f, "{sign}{h:02}")
                } else {
                    write!(f, "{sign}{h:02}:{m:02}")
                }
            }
        }
    }
}

impl Display for DateTime {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}T{}", self.date, self.time)
    }
}

impl Display for Interval {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        fn endpoint(f: &mut Formatter<'_>, e: &IntervalEndpoint) -> fmt::Result {
            match e {
                IntervalEndpoint::Unknown => Ok(()),
                IntervalEndpoint::Open => f.write_str(".."),
                IntervalEndpoint::Date(d) => d.fmt(f),
                IntervalEndpoint::OnOrBefore(d) => write!(f, "..{d}"),
                IntervalEndpoint::OnOrAfter(d) => write!(f, "{d}.."),
            }
        }
        endpoint(f, &self.start)?;
        f.write_char('/')?;
        endpoint(f, &self.end)
    }
}

impl Display for Set {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let (open, close) = match self.kind {
            SetKind::AllMembers => ('{', '}'),
            SetKind::OneMember => ('[', ']'),
        };
        f.write_char(open)?;
        for (i, e) in self.elements.iter().enumerate() {
            if i > 0 {
                f.write_char(',')?;
            }
            match e {
                SetElement::Date(d) => d.fmt(f)?,
                SetElement::OnOrBefore(d) => write!(f, "..{d}")?,
                SetElement::OnOrAfter(d) => write!(f, "{d}..")?,
                SetElement::Range(a, b) => write!(f, "{a}..{b}")?,
            }
        }
        f.write_char(close)
    }
}

#[cfg(test)]
mod tests {
    use alloc::format;

    use super::*;

    const UNCERTAIN: Qualifier = Qualifier {
        uncertain: true,
        approximate: false,
    };

    /// `qual_symbol` has no symbol for "no qualification": the caller must
    /// filter on `is_qualified` first, and misuse is a panic rather than a
    /// silently wrong character in the output.
    #[test]
    #[should_panic(expected = "caller checks is_qualified")]
    fn unqualified_has_no_symbol() {
        qual_symbol(Qualifier::default());
    }

    /// The prefix scan's length guard: parts that all share `q0` walk to the
    /// last index instead of indexing past the end. `Display for Date` never
    /// hands it such a slice — a uniformly qualified date takes the
    /// complete-qualification early return — so only a direct call reaches it.
    #[test]
    fn prefix_scan_stops_at_the_final_part() {
        let uniform = [
            (String::from("2004"), UNCERTAIN),
            (String::from("06"), UNCERTAIN),
            (String::from("11"), UNCERTAIN),
        ];
        assert_eq!(qualified_prefix_end(&uniform, UNCERTAIN), 2);

        // The reachable shape, for contrast: the scan stops before the day.
        let mixed = [
            (String::from("2004"), UNCERTAIN),
            (String::from("06"), UNCERTAIN),
            (String::from("11"), Qualifier::default()),
        ];
        assert_eq!(qualified_prefix_end(&mixed, UNCERTAIN), 1);
    }

    /// The parser only sets `hours_only` on `±hh`, whose minutes are zero, so
    /// a hand-built shift carrying both is out of the parser's reach. Display
    /// degrades gracefully rather than truncating: the minutes survive.
    #[test]
    fn hours_only_shift_with_minutes_still_renders_them() {
        let t = Time {
            hour: 9,
            minute: 30,
            second: 0,
            shift: Some(TimeShift::Offset {
                minutes: 90,
                hours_only: true,
            }),
        };
        assert_eq!(format!("{t}"), "09:30:00+01:30");
    }
}
