// SPDX-FileCopyrightText: Copyright (c) the edtf contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Enumeration of the concrete calendar values an expression denotes.
//!
//! ISO 8601-2 grounds exactly three value-set constructions, and
//! [`Edtf::values`] enumerates those and nothing else:
//!
//! - set range elements expand inclusively at their own precision (§6.3 c, §6.4
//!   Example 1: `{1667,1668,1670..1672}` *is* `{1667,1668,1670,1671, 1672}`);
//! - unspecified digits denote their valid completions (§9.2.2 Example 6:
//!   `1560-X2` is "either February or December");
//! - significant-digit years denote a swept year range (§4.4.3: `1950S2` is
//!   some year 1900–1999).
//!
//! Everything else is a singleton (a concrete date, datetime, season or
//! `Y`-year yields itself, verbatim) or [`Unenumerable`]: intervals denote
//! one continuous extent — Clause 10 has no enumeration language — and the
//! `..`-prefixed/suffixed set elements of §6.3 a–b are "indication", never
//! "expansion" (§6.4 Example 2 pointedly leaves `1760-12..` unexpanded).
//!
//! Values are yielded lazily in written element order, ascending within
//! each element, with qualifiers copied through unchanged (qualification
//! never moves the value set, §8.4.2 NOTE). Decisions D24–D29 in
//! docs/spec-notes.md.

#![expect(
    clippy::min_ident_chars,
    reason = "y, m and d are the universal notation for a date's components, and this crate is about little else"
)]
#![expect(
    clippy::arithmetic_side_effects,
    reason = "every flagged operation is bounded where it stands: slice indices by a length guard on the line above, digit values to 0-9 by the match arm that binds them, and the JDN forms by being computed in i128 so any i64 year fits. The operations that genuinely could leave range already use checked_/saturating_ and return an error rather than wrapping"
)]
#![expect(
    clippy::integer_division_remainder_used,
    reason = "calendar arithmetic is integer division by definition — the leap rules are /4, /100 and /400, and a float would be wrong"
)]
#![expect(
    clippy::pattern_type_mismatch,
    reason = "matching through a reference without restating & at every level is the idiomatic form the rest of this crate uses"
)]
#![expect(
    clippy::as_conversions,
    reason = "the operands are proven in range by the guard or type immediately above each cast, and try_from at these sites would add an unreachable error path"
)]
#![expect(
    clippy::absolute_paths,
    reason = "a one-use std path written in full at the call site is clearer than an import that only appears once"
)]
#![expect(
    clippy::indexing_slicing,
    reason = "each index is preceded by the bounds check that justifies it, or indexes a fixed-size array whose length is in its type"
)]
#![expect(
    clippy::integer_division,
    reason = "calendar arithmetic is integer division by definition — the leap rules are /4, /100 and /400, and a float would be wrong"
)]
#![expect(
    clippy::missing_inline_in_public_items,
    reason = "inlining is the compiler's call across a crate boundary, and the release profile enables fat LTO — annotating every public item would assert a decision this crate has not measured"
)]
#![expect(
    clippy::missing_trait_methods,
    reason = "the default implementations are what this type wants; overriding them to satisfy a lint would be code with no reason to exist"
)]
#![expect(
    clippy::single_call_fn,
    reason = "a named helper used once is extraction for readability, which is the opposite of a defect; several are also the named steps the module docs describe"
)]
#![expect(
    clippy::unreachable,
    reason = "an unreachable! whose comment names the caller-side check that makes it unreachable — a deliberate assertion of an invariant, not an unhandled case"
)]
#![expect(
    clippy::exhaustive_enums,
    reason = "these enums enumerate what ISO 8601-2 defines; the spec fixes the variants, so non_exhaustive would promise additions the format cannot make"
)]
#![expect(
    clippy::missing_errors_doc,
    reason = "edtf-core declares every module private and exports only named types, so nothing flagged here is reachable from outside the crate and there is no published error contract to document"
)]
#![expect(
    clippy::too_long_first_doc_paragraph,
    reason = "the items are crate-private, so these paragraphs render in no rustdoc summary; they are written to be read next to the code they describe"
)]

use alloc::vec::Vec;

use crate::{
    bounds::{
        big_width, day_candidates_of, is_leap, last_day, month_candidates_of, significant_range,
    },
    types::{Date, DateField, Edtf, Precision, Qualifier, SetElement, Year, YearKind},
};

/// Why [`Edtf::values`] cannot enumerate an expression (decisions D24–D25).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Unenumerable {
    /// Intervals denote a single continuous extent, not a collection of
    /// values (ISO 8601-2 Clause 10 defines no enumeration for them).
    Interval,
    /// The set contains a `..date` or `date..` element, whose value set is
    /// unbounded (§6.3 a–b give "indication", not "expansion").
    UnboundedSetElement,
    /// A significant-digit sweep or range endpoint lies beyond the years
    /// this library computes with — the same cases where `bounds()` reports
    /// [`Bound::Unknown`](crate::Bound::Unknown).
    YearRangeOverflow,
}

impl core::fmt::Display for Unenumerable {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::Interval => "intervals denote an extent, not enumerable values",
            Self::UnboundedSetElement => "'..'-open set elements denote unbounded value sets",
            Self::YearRangeOverflow => "year range exceeds the computable range",
        })
    }
}

impl core::error::Error for Unenumerable {}

impl Edtf {
    /// Enumerate the concrete calendar values this expression denotes, each
    /// at its own precision (decisions D24–D29 in docs/spec-notes.md).
    ///
    /// The iterator is lazy — state is proportional to the number of set
    /// elements, never to the number of values — so pathological
    /// cardinalities (`XXXX-XX-XX` denotes ~3.65 million days) stream
    /// without allocation. Errors are structural and complete at
    /// construction: iteration itself cannot fail.
    ///
    /// ```
    /// use edtf_core::Edtf;
    ///
    /// // ISO 8601-2 §6.4 Example 1, expanded exactly as the spec does:
    /// let set = Edtf::parse("{1667,1668,1670..1672}").unwrap();
    /// let years: Vec<String> = set.values().unwrap().map(|v| v.to_string()).collect();
    /// assert_eq!(years, ["1667", "1668", "1670", "1671", "1672"]);
    ///
    /// // Unspecified digits enumerate their valid completions (§9.2.2):
    /// let masked = Edtf::parse("1985-0X-31").unwrap();
    /// let months: Vec<String> = masked.values().unwrap().map(|v| v.to_string()).collect();
    /// assert_eq!(
    ///     months,
    ///     [
    ///         "1985-01-31",
    ///         "1985-03-31",
    ///         "1985-05-31",
    ///         "1985-07-31",
    ///         "1985-08-31"
    ///     ]
    /// );
    ///
    /// // Intervals denote one extent, not a collection:
    /// assert!(Edtf::parse("2004/2005").unwrap().values().is_err());
    /// ```
    ///
    /// # Errors
    ///
    /// [`Unenumerable`] when the value denotes no finite collection: an
    /// interval, a set with an unbounded `..date`/`date..` element, or a
    /// year sweep beyond the numeric range this library computes with.
    pub fn values(&self) -> Result<Values, Unenumerable> {
        let state = match self {
            Self::Interval(_) => return Err(Unenumerable::Interval),
            Self::DateTime(dt) => State::Singleton(Some(Self::DateTime(*dt))),
            Self::Date(d) => State::Date(DateValues::new(d)?),
            Self::Set(s) => {
                let mut queue = Vec::with_capacity(s.elements.len());
                for e in &s.elements {
                    queue.push(match e {
                        SetElement::Date(d) => ElementValues::Date(DateValues::new(d)?),
                        SetElement::Range(a, b) => ElementValues::Range(RangeWalk::new(a, b)?),
                        SetElement::OnOrBefore(_) | SetElement::OnOrAfter(_) => {
                            return Err(Unenumerable::UnboundedSetElement);
                        }
                    });
                }
                State::Set { queue, idx: 0 }
            }
        };
        Ok(Values { state })
    }
}

/// Lazy iterator over the values an expression denotes; see [`Edtf::values`].
#[derive(Debug, Clone)]
pub struct Values {
    /// What the iterator is currently walking.
    state: State,
}

#[derive(Debug, Clone)]
/// The shape of value the expression enumerates.
enum State {
    /// A concrete expression: itself, once.
    Singleton(Option<Edtf>),
    /// A single date with masked or ranged fields.
    Date(DateValues),
    /// A set: each element walked in written order.
    Set {
        /// Element walkers still to be drained, in written order.
        queue: Vec<ElementValues>,
        /// Index of the element currently being drained.
        idx: usize,
    },
}

#[derive(Debug, Clone)]
/// How one set element enumerates.
enum ElementValues {
    /// A date element, masked or concrete.
    Date(DateValues),
    /// A range element, walked endpoint to endpoint.
    Range(RangeWalk),
}

impl ElementValues {
    /// The next date this element yields, or `None` when drained.
    fn next(&mut self) -> Option<Date> {
        match self {
            Self::Date(dv) => dv.next(),
            Self::Range(rw) => rw.next(),
        }
    }
}

impl Iterator for Values {
    type Item = Edtf;

    fn next(&mut self) -> Option<Edtf> {
        match &mut self.state {
            State::Singleton(v) => v.take(),
            State::Date(dv) => dv.next().map(Edtf::Date),
            State::Set { queue, idx } => loop {
                let cur = queue.get_mut(*idx)?;
                if let Some(d) = cur.next() {
                    return Some(Edtf::Date(d));
                }
                *idx += 1;
            },
        }
    }
}

/// Rebuild a concrete year value in its canonical syntactic form: four
/// digits within ±9999 (D2: `-0000` cannot arise — the value would be 0),
/// `Y`-prefixed beyond (D1).
fn concrete_year(v: i64, qualifier: Qualifier) -> Year {
    let kind = if (-9999..=9999).contains(&v) {
        let mag = v.unsigned_abs();
        YearKind::Standard {
            negative: v < 0,
            digits: [
                Some((mag / 1000 % 10) as u8),
                Some((mag / 100 % 10) as u8),
                Some((mag / 10 % 10) as u8),
                Some((mag % 10) as u8),
            ],
        }
    } else {
        YearKind::Big { value: v }
    };
    Year {
        kind,
        significant_digits: None,
        qualifier,
    }
}

/// A concrete value as a `DateField`, carrying the field's qualifier.
const fn concrete_field(v: u8, qualifier: Qualifier) -> DateField {
    DateField {
        digits: [Some(v / 10), Some(v % 10)],
        qualifier,
    }
}

/// Values of a single (possibly masked or significant-digit) date.
#[derive(Debug, Clone)]
enum DateValues {
    /// Fully specified: yields itself, verbatim, once.
    Singleton(Option<Date>),
    /// `S`-suffix year sweep (§4.4.3), ascending, year precision.
    Sweep {
        /// The year currently being yielded.
        cur: i64,
        /// The last year in the walk, inclusive.
        hi: i64,
        /// The qualifier every yielded date carries.
        qualifier: Qualifier,
        /// Set once the walk has passed `hi`.
        done: bool,
    },
    /// Unspecified digits: valid completions in ascending calendar order.
    Masked(Masked),
}

impl DateValues {
    /// Build a walker for one date, or say why it cannot be enumerated.
    fn new(d: &Date) -> Result<Self, Unenumerable> {
        if d.year.significant_digits.is_some() {
            // S-years are year-precision only (the parser enforces this).
            let value = d.year.value().ok_or(Unenumerable::YearRangeOverflow)?;
            let (lo, hi) =
                significant_range(value, d.year.significant_digits, big_width(&d.year.kind))
                    .ok_or(Unenumerable::YearRangeOverflow)?;
            return Ok(Self::Sweep {
                cur: lo,
                hi,
                qualifier: d.year.qualifier,
                done: false,
            });
        }
        if !d.has_unspecified() {
            return Ok(Self::Singleton(Some(*d)));
        }
        Ok(Self::Masked(Masked::new(d)))
    }

    /// The next date in the walk, or `None` when it is finished.
    fn next(&mut self) -> Option<Date> {
        match self {
            Self::Singleton(d) => d.take(),
            Self::Sweep {
                cur,
                hi,
                qualifier,
                done,
            } => {
                if *done {
                    return None;
                }
                let v = *cur;
                if v == *hi {
                    *done = true;
                } else {
                    *cur += 1;
                }
                Some(Date {
                    year: concrete_year(v, *qualifier),
                    month: None,
                    day: None,
                })
            }
            Self::Masked(m) => m.next(),
        }
    }
}

/// Odometer over the valid completions of a masked date: years ascend
/// (least-significant masked digit fastest, so consecutive counters yield
/// consecutive matching years), months and days ascend within each year,
/// and calendar-invalid day combinations are skipped. Validation (D11)
/// guarantees at least one completion overall; masked years are
/// non-negative (D21) and masked months draw from 01–12 only (D14).
#[derive(Debug, Clone)]
enum MaskedYear {
    /// Concrete (possibly negative) year value.
    Fixed(i64),
    /// Masked digit pattern to sweep (non-negative, D21).
    Pattern([Option<u8>; 4]),
}

#[derive(Debug, Clone)]
/// Odometer over a masked date's free digits, most significant first.
struct Masked {
    /// The masked year and the completions it admits.
    year: MaskedYear,
    /// How many year completions the mask has.
    year_count: u32,
    /// Month candidates, when the month itself is masked.
    months: Option<Vec<u8>>,
    /// Day candidates, when the day itself is masked.
    days: Option<Vec<u8>>,
    /// Qualifier carried by every yielded year.
    yq: Qualifier,
    /// Qualifier carried by every yielded month.
    mq: Qualifier,
    /// Qualifier carried by every yielded day.
    dq: Qualifier,
    /// Position in the year odometer.
    yi: u32,
    /// Position in the month candidates.
    mi: usize,
    /// Position in the day candidates.
    di: usize,
}

impl Masked {
    /// Build the odometer for a masked date.
    fn new(d: &Date) -> Self {
        // Masks only occur on standard years; Y-years are digit-valued.
        let (year, year_count) = match (d.year.value(), d.year.kind) {
            (Some(v), _) => (MaskedYear::Fixed(v), 1),
            (None, YearKind::Standard { digits, .. }) => {
                // At most four maskable digit positions.
                let n = u32::try_from(digits.iter().filter(|x| x.is_none()).count()).unwrap_or(4);
                (MaskedYear::Pattern(digits), 10_u32.pow(n))
            }
            (None, _) => unreachable!("only standard years carry X digits"),
        };
        Self {
            year,
            year_count,
            months: d.month.map(month_candidates_of),
            days: d.day.map(day_candidates_of),
            yq: d.year.qualifier,
            mq: d.month.map(|m| m.qualifier).unwrap_or_default(),
            dq: d.day.map(|f| f.qualifier).unwrap_or_default(),
            yi: 0,
            mi: 0,
            di: 0,
        }
    }

    /// The year the odometer's `counter`th position denotes.
    fn year_at(&self, counter: u32) -> i64 {
        match self.year {
            MaskedYear::Fixed(v) => v,
            MaskedYear::Pattern(digits) => {
                let mut rem = counter;
                let mut filled = digits.map(|d| d.unwrap_or(0));
                for pos in (0..4).rev() {
                    if digits[pos].is_none() {
                        filled[pos] = (rem % 10) as u8;
                        rem /= 10;
                    }
                }
                filled.iter().fold(0, |acc, d| acc * 10 + i64::from(*d))
            }
        }
    }

    /// The next completion, or `None` once every position is spent.
    fn next(&mut self) -> Option<Date> {
        loop {
            if self.yi >= self.year_count {
                return None;
            }
            let y = self.year_at(self.yi);
            let Some(months) = &self.months else {
                self.yi += 1;
                return Some(Date {
                    year: concrete_year(y, self.yq),
                    month: None,
                    day: None,
                });
            };
            while self.mi < months.len() {
                let m = months[self.mi];
                let Some(days) = &self.days else {
                    self.mi += 1;
                    // `m` may be a season code 21–41 (only when written
                    // concretely — masked slots draw from 01–12, D14).
                    return Some(Date {
                        year: concrete_year(y, self.yq),
                        month: Some(concrete_field(m, self.mq)),
                        day: None,
                    });
                };
                while self.di < days.len() {
                    let dd = days[self.di];
                    self.di += 1;
                    if dd <= last_day(m, is_leap(y)) {
                        return Some(Date {
                            year: concrete_year(y, self.yq),
                            month: Some(concrete_field(m, self.mq)),
                            day: Some(concrete_field(dd, self.dq)),
                        });
                    }
                }
                self.di = 0;
                self.mi += 1;
            }
            self.mi = 0;
            self.yi += 1;
        }
    }
}

/// Inclusive expansion of a set range element `a..b` (§6.3 c, §6.4) by
/// calendar successor at the endpoints' shared precision. The parser (D27)
/// guarantees the endpoints are concrete, unqualified, non-season,
/// same-precision dates in order (D18), so the walk terminates by reaching
/// `b` exactly.
#[derive(Debug, Clone)]
struct RangeWalk {
    /// Current position; slots beyond the precision stay 0 so plain
    /// tuple equality detects the end.
    cur: (i64, u8, u8),
    /// The last (year, month, day) in the walk, inclusive.
    end: (i64, u8, u8),
    /// The precision the walk steps at.
    precision: Precision,
    /// Set once the walk has passed `end`.
    done: bool,
}

impl RangeWalk {
    /// Build a walker between two endpoints, or say why it cannot be enumerated.
    fn new(a: &Date, b: &Date) -> Result<Self, Unenumerable> {
        let ay = a.year.value().ok_or(Unenumerable::YearRangeOverflow)?;
        let by = b.year.value().ok_or(Unenumerable::YearRangeOverflow)?;
        let field = |f: Option<DateField>| f.and_then(DateField::value).unwrap_or(0);
        Ok(Self {
            cur: (ay, field(a.month), field(a.day)),
            end: (by, field(b.month), field(b.day)),
            precision: a.precision(),
            done: false,
        })
    }

    /// The next date in the range, or `None` when it is finished.
    fn next(&mut self) -> Option<Date> {
        if self.done {
            return None;
        }
        let (y, m, d) = self.cur;
        if self.cur == self.end {
            self.done = true;
        } else {
            self.cur = match self.precision {
                Precision::Year => (y + 1, 0, 0),
                Precision::Month => {
                    if m == 12 {
                        (y + 1, 1, 0)
                    } else {
                        (y, m + 1, 0)
                    }
                }
                Precision::Day => {
                    if d < last_day(m, is_leap(y)) {
                        (y, m, d + 1)
                    } else if m == 12 {
                        (y + 1, 1, 1)
                    } else {
                        (y, m + 1, 1)
                    }
                }
                Precision::Season => unreachable!("D27 rejects season range endpoints"),
            };
        }
        let q = Qualifier::default();
        Some(Date {
            year: concrete_year(y, q),
            month: (self.precision != Precision::Year).then(|| concrete_field(m, q)),
            day: (self.precision == Precision::Day).then(|| concrete_field(d, q)),
        })
    }
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::missing_panics_doc,
        reason = "a test asserts by panicking; that is the failure signal, so there is no caller to warn"
    )]
    #![expect(
        clippy::inline_modules,
        reason = "a module small enough to read in place belongs in place"
    )]
    #![allow(
        clippy::unwrap_used,
        reason = "test code: a panic here is the failure signal, not a crash path"
    )]

    use alloc::vec;

    use super::*;
    use crate::types::{Set, SetKind};

    const PLAIN: Qualifier = Qualifier {
        uncertain: false,
        approximate: false,
    };

    /// A hand-built date carrying an `X` in a field while its year is an
    /// exponential too large for `i64` — `Year::value()` is then `None` on a
    /// kind that cannot hold `X` digits. The parser cannot build this (masks
    /// only attach to standard years), and the odometer refuses to guess a
    /// year pattern for it.
    #[test]
    #[should_panic(expected = "only standard years carry X digits")]
    fn masked_field_under_an_unrepresentable_year_panics() {
        let d = Date {
            year: Year {
                // 1 × 10^100 overflows i64, so `value()` yields None.
                kind: YearKind::Exponential {
                    significand: 1,
                    exponent: 100,
                },
                significant_digits: None,
                qualifier: PLAIN,
            },
            month: Some(DateField {
                digits: [Some(0), None],
                qualifier: PLAIN,
            }),
            day: None,
        };
        drop(Edtf::Date(d).values());
    }

    /// D27 rejects season endpoints in a set range at parse time — seasons
    /// have no spec-defined successor, so the walk has no step to take. A
    /// hand-built `2001-21..2002-21` reaches the guard on the first step.
    #[test]
    #[should_panic(expected = "D27 rejects season range endpoints")]
    fn season_range_endpoints_have_no_successor() {
        let season = |y: i64| Date {
            year: concrete_year(y, PLAIN),
            month: Some(concrete_field(21, PLAIN)),
            day: None,
        };
        let set = Edtf::Set(Set {
            kind: SetKind::AllMembers,
            elements: vec![SetElement::Range(season(2001), season(2002))],
        });
        let mut values = set.values().unwrap();
        drop(values.next());
    }
}
