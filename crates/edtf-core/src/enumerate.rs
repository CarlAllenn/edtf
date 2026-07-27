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
                        },
                    });
                }
                State::Set { queue, idx: 0 }
            },
        };
        Ok(Values { state })
    }
}

/// Lazy iterator over the values an expression denotes; see [`Edtf::values`].
#[derive(Debug, Clone)]
pub struct Values {
    state: State,
}

#[derive(Debug, Clone)]
enum State {
    Singleton(Option<Edtf>),
    Date(DateValues),
    Set {
        queue: Vec<ElementValues>,
        idx: usize,
    },
}

#[derive(Debug, Clone)]
enum ElementValues {
    Date(DateValues),
    Range(RangeWalk),
}

impl ElementValues {
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
        cur: i64,
        hi: i64,
        qualifier: Qualifier,
        done: bool,
    },
    /// Unspecified digits: valid completions in ascending calendar order.
    Masked(Masked),
}

impl DateValues {
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
            },
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
struct Masked {
    year: MaskedYear,
    year_count: u32,
    months: Option<Vec<u8>>,
    days: Option<Vec<u8>>,
    yq: Qualifier,
    mq: Qualifier,
    dq: Qualifier,
    yi: u32,
    mi: usize,
    di: usize,
}

impl Masked {
    fn new(d: &Date) -> Self {
        // Masks only occur on standard years; Y-years are digit-valued.
        let (year, year_count) = match (d.year.value(), d.year.kind) {
            (Some(v), _) => (MaskedYear::Fixed(v), 1),
            (None, YearKind::Standard { digits, .. }) => {
                // At most four maskable digit positions.
                let n = u32::try_from(digits.iter().filter(|x| x.is_none()).count()).unwrap_or(4);
                (MaskedYear::Pattern(digits), 10u32.pow(n))
            },
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
            },
        }
    }

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
    end: (i64, u8, u8),
    precision: Precision,
    done: bool,
}

impl RangeWalk {
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
                },
                Precision::Day => {
                    if d < last_day(m, is_leap(y)) {
                        (y, m, d + 1)
                    } else if m == 12 {
                        (y + 1, 1, 1)
                    } else {
                        (y, m + 1, 1)
                    }
                },
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
