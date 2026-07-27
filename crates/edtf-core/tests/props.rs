//! Model-side property tests: generate random *valid* EDTF values as
//! structured data and check invariants over every one of them.
//!
//! This is the complementary bug class to the string-side fuzzing in
//! `tests/fuzz.rs` and `fuzz/`: values the parser may never produce but the
//! type system can represent, and invariant violations on shapes random
//! strings rarely reach.
//!
//! Generators are constructive rather than filtered: masks are applied to an
//! already-valid concrete date (so at least that completion stays valid,
//! decision D11), ranges are compare-and-swapped into order using the same
//! bounds comparison the parser applies (decision D18), and time shifts are
//! only generated in shapes `Display` can spell.

use edtf_core::{
    Bound, Date, DateField, DateTime, Edtf, Interval, IntervalEndpoint, Modality, Qualifier,
    Relation, Set, SetElement, SetKind, Time, TimeShift, Unenumerable, Year, YearKind,
};
use proptest::prelude::*;

// ------------------------------------------------------------ calendar

fn is_leap(y: i64) -> bool {
    y.rem_euclid(4) == 0 && (y.rem_euclid(100) != 0 || y.rem_euclid(400) == 0)
}

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
        _ => unreachable!("month is 1-12"),
    }
}

fn year_digits(y: u16) -> [Option<u8>; 4] {
    [
        Some((y / 1000 % 10) as u8),
        Some((y / 100 % 10) as u8),
        Some((y / 10 % 10) as u8),
        Some((y % 10) as u8),
    ]
}

fn field_digits(v: u8) -> [Option<u8>; 2] {
    [Some(v / 10), Some(v % 10)]
}

/// Turn digits into `X`s where `bits` says so.
fn mask<const N: usize>(mut digits: [Option<u8>; N], bits: [bool; N]) -> [Option<u8>; N] {
    for (d, m) in digits.iter_mut().zip(bits) {
        if m {
            *d = None;
        }
    }
    digits
}

/// The parser's interval/range ordering rule (decision D18): out of order
/// only when `a`'s earliest possible day is later than `b`'s latest.
fn dates_out_of_order(a: &Date, b: &Date) -> bool {
    let lo = Edtf::Date(*a).bounds().earliest;
    let hi = Edtf::Date(*b).bounds().latest;
    match (lo, hi) {
        (Bound::Date(lo), Bound::Date(hi)) => lo > hi,
        _ => false,
    }
}

// ------------------------------------------------------------ strategies

fn qualifier() -> impl Strategy<Value = Qualifier> {
    (any::<bool>(), any::<bool>()).prop_map(|(uncertain, approximate)| Qualifier {
        uncertain,
        approximate,
    })
}

/// Four-digit year, possibly negative (then unmasked, and never -0000) or
/// masked (then non-negative) — the parser's standard-year rules.
fn standard_year() -> impl Strategy<Value = Year> {
    (any::<bool>(), 0u16..=9999, any::<[bool; 4]>(), qualifier()).prop_map(
        |(negative, y, bits, qualifier)| {
            let negative = negative && y != 0;
            let digits = if negative {
                year_digits(y)
            } else {
                mask(year_digits(y), bits)
            };
            Year {
                kind: YearKind::Standard { negative, digits },
                significant_digits: None,
                qualifier,
            }
        },
    )
}

/// Standard year with an `S` suffix: unmasked, 1-4 significant digits.
fn sig_standard_year() -> impl Strategy<Value = Year> {
    (any::<bool>(), 0u16..=9999, 1u32..=4, qualifier()).prop_map(|(negative, y, s, qualifier)| {
        Year {
            kind: YearKind::Standard {
                negative: negative && y != 0,
                digits: year_digits(y),
            },
            significant_digits: Some(s),
            qualifier,
        }
    })
}

/// `Y`-prefixed year: |value| > 9999, significant digits within the digit
/// count.
fn big_year() -> impl Strategy<Value = Year> {
    (
        10_000i64..=9_999_999_999_999,
        any::<bool>(),
        proptest::option::of(1u32..=13),
        qualifier(),
    )
        .prop_map(|(mag, negative, sig, qualifier)| {
            let digit_count = mag.ilog10() + 1;
            Year {
                kind: YearKind::Big {
                    value: if negative { -mag } else { mag },
                },
                significant_digits: sig.map(|s| s.min(digit_count)),
                qualifier,
            }
        })
}

/// `Y…E…` year: non-zero significand, |value| > 9999, significant digits
/// within significand digits + exponent.
fn exp_year() -> impl Strategy<Value = Year> {
    (
        1i64..=999_999,
        any::<bool>(),
        0u32..=12,
        proptest::option::of(1u32..=18),
        qualifier(),
    )
        .prop_filter("|value| must exceed 9999", |(mag, _, exp, ..)| {
            i128::from(*mag) * 10i128.pow(*exp) > 9999
        })
        .prop_map(|(mag, negative, exponent, sig, qualifier)| {
            let digit_count = mag.ilog10() + 1 + exponent;
            Year {
                kind: YearKind::Exponential {
                    significand: if negative { -mag } else { mag },
                    exponent,
                },
                significant_digits: sig.map(|s| s.min(digit_count)),
                qualifier,
            }
        })
}

/// A concrete, valid calendar (year, month, day) triple.
fn ymd() -> impl Strategy<Value = (u16, u8, u8)> {
    (0u16..=9999, 1u8..=12).prop_flat_map(|(y, m)| {
        let hi = last_day(m, is_leap(i64::from(y)));
        (Just(y), Just(m), 1u8..=hi)
    })
}

fn year_only_date() -> impl Strategy<Value = Date> {
    prop_oneof![
        4 => standard_year(),
        1 => sig_standard_year(),
        1 => big_year(),
        1 => exp_year(),
    ]
    .prop_map(|year| Date {
        year,
        month: None,
        day: None,
    })
}

/// Standard year plus an explicit sub-year grouping code 21-41 (no day).
fn season_date() -> impl Strategy<Value = Date> {
    (standard_year(), 21u8..=41, qualifier()).prop_map(|(year, code, q)| Date {
        year,
        month: Some(DateField {
            digits: field_digits(code),
            qualifier: q,
        }),
        day: None,
    })
}

/// Year-month or year-month-day, starting from a concrete valid date and
/// masking digits — the concrete completion keeps every mask valid (D11).
fn month_day_date() -> impl Strategy<Value = Date> {
    (
        ymd(),
        any::<bool>(),
        any::<bool>(),
        any::<[bool; 4]>(),
        any::<[bool; 2]>(),
        any::<[bool; 2]>(),
        (qualifier(), qualifier(), qualifier()),
    )
        .prop_map(
            |((y, m, d), negative, with_day, ybits, mbits, dbits, (yq, mq, dq))| {
                let negative = negative && y != 0;
                let digits = if negative {
                    year_digits(y)
                } else {
                    mask(year_digits(y), ybits)
                };
                Date {
                    year: Year {
                        kind: YearKind::Standard { negative, digits },
                        significant_digits: None,
                        qualifier: yq,
                    },
                    month: Some(DateField {
                        digits: mask(field_digits(m), mbits),
                        qualifier: mq,
                    }),
                    day: with_day.then_some(DateField {
                        digits: mask(field_digits(d), dbits),
                        qualifier: dq,
                    }),
                }
            },
        )
}

fn date() -> impl Strategy<Value = Date> {
    prop_oneof![
        2 => year_only_date(),
        1 => season_date(),
        4 => month_day_date(),
    ]
}

/// Time shifts only in the shapes `Display` can spell: `hours_only`
/// requires a whole-hour offset.
fn shift() -> impl Strategy<Value = Option<TimeShift>> {
    prop_oneof![
        1 => Just(None),
        1 => Just(Some(TimeShift::Utc)),
        1 => (-14i16..=14).prop_map(|h| Some(TimeShift::Offset {
            minutes: h * 60,
            hours_only: true,
        })),
        1 => (-840i16..=840).prop_map(|minutes| Some(TimeShift::Offset {
            minutes,
            hours_only: false,
        })),
    ]
}

/// Date-times exist only at level 0: plain, complete, unqualified date.
fn datetime() -> impl Strategy<Value = DateTime> {
    (ymd(), (0u8..=23, 0u8..=59, 0u8..=60, shift())).prop_map(
        |((y, m, d), (hour, minute, second, shift))| DateTime {
            date: Date {
                year: Year {
                    kind: YearKind::Standard {
                        negative: false,
                        digits: year_digits(y),
                    },
                    significant_digits: None,
                    qualifier: Qualifier::default(),
                },
                month: Some(DateField {
                    digits: field_digits(m),
                    qualifier: Qualifier::default(),
                }),
                day: Some(DateField {
                    digits: field_digits(d),
                    qualifier: Qualifier::default(),
                }),
            },
            time: Time {
                hour,
                minute,
                second,
                shift,
            },
        },
    )
}

fn interval() -> impl Strategy<Value = Interval> {
    let start = prop_oneof![
        5 => date().prop_map(IntervalEndpoint::Date),
        1 => Just(IntervalEndpoint::Open),
        1 => Just(IntervalEndpoint::Unknown),
        2 => date().prop_map(IntervalEndpoint::OnOrBefore),
    ];
    let end = prop_oneof![
        5 => date().prop_map(IntervalEndpoint::Date),
        1 => Just(IntervalEndpoint::Open),
        1 => Just(IntervalEndpoint::Unknown),
        2 => date().prop_map(IntervalEndpoint::OnOrAfter),
    ];
    (start, end)
        .prop_filter("interval needs a dated endpoint", |(s, e)| {
            s.is_dated() || e.is_dated()
        })
        .prop_map(|(start, end)| {
            // Comparable concrete endpoints get compare-and-swapped into
            // order; a swap is always sufficient because a single date's
            // bounds are themselves ordered.
            if let (IntervalEndpoint::Date(a), IntervalEndpoint::Date(b)) = (&start, &end) {
                if dates_out_of_order(a, b) {
                    return Interval {
                        start: IntervalEndpoint::Date(*b),
                        end: IntervalEndpoint::Date(*a),
                    };
                }
            }
            Interval { start, end }
        })
}

/// A concrete, unqualified, unmasked date at the given precision
/// (0 = year, 1 = month, 2 = day) — the only shape D27 admits as a `..`
/// range endpoint. Years may be negative (never -0000).
fn plain_date(y: i64, m: u8, d: u8, precision: u8) -> Date {
    Date {
        year: Year {
            kind: YearKind::Standard {
                negative: y < 0,
                digits: year_digits(y.unsigned_abs() as u16),
            },
            significant_digits: None,
            qualifier: Qualifier::default(),
        },
        month: (precision >= 1).then_some(DateField {
            digits: field_digits(m),
            qualifier: Qualifier::default(),
        }),
        day: (precision >= 2).then_some(DateField {
            digits: field_digits(d),
            qualifier: Qualifier::default(),
        }),
    }
}

/// D27 range elements: both endpoints concrete, unqualified, non-season,
/// same-precision, compare-and-swapped into order at that precision.
fn range_element() -> impl Strategy<Value = SetElement> {
    (ymd(), ymd(), any::<[bool; 2]>(), 0u8..=2).prop_map(
        |((y1, m1, d1), (y2, m2, d2), neg, precision)| {
            let sy1 = if neg[0] && y1 != 0 {
                -i64::from(y1)
            } else {
                i64::from(y1)
            };
            let sy2 = if neg[1] && y2 != 0 {
                -i64::from(y2)
            } else {
                i64::from(y2)
            };
            let key = |y: i64, m: u8, d: u8| match precision {
                0 => (y, 0, 0),
                1 => (y, m, 0),
                _ => (y, m, d),
            };
            let (a, b) = if key(sy1, m1, d1) <= key(sy2, m2, d2) {
                ((sy1, m1, d1), (sy2, m2, d2))
            } else {
                ((sy2, m2, d2), (sy1, m1, d1))
            };
            SetElement::Range(
                plain_date(a.0, a.1, a.2, precision),
                plain_date(b.0, b.1, b.2, precision),
            )
        },
    )
}

fn set_element() -> impl Strategy<Value = SetElement> {
    prop_oneof![
        5 => date().prop_map(SetElement::Date),
        1 => date().prop_map(SetElement::OnOrBefore),
        1 => date().prop_map(SetElement::OnOrAfter),
        3 => range_element(),
    ]
}

fn set() -> impl Strategy<Value = Set> {
    (
        any::<bool>(),
        proptest::collection::vec(set_element(), 1..=4),
    )
        .prop_map(|(all, elements)| Set {
            kind: if all {
                SetKind::AllMembers
            } else {
                SetKind::OneMember
            },
            elements,
        })
}

fn edtf() -> impl Strategy<Value = Edtf> {
    prop_oneof![
        5 => date().prop_map(Edtf::Date),
        1 => datetime().prop_map(Edtf::DateTime),
        2 => interval().prop_map(Edtf::Interval),
        2 => set().prop_map(Edtf::Set),
    ]
}

// ------------------------------------------------------------ properties

proptest! {
    // Cheap cases (sub-second for all four properties together), so run
    // well above the default 256.
    #![proptest_config(ProptestConfig::with_cases(2048))]

    /// `Display` → `parse` round-trips to the *identical* value — generative
    /// over the model, not just over strings the parser accepted first.
    #[test]
    fn display_parse_roundtrips_to_identity(v in edtf()) {
        let s = v.to_string();
        let reparsed = Edtf::parse(&s);
        prop_assert_eq!(reparsed, Ok(v), "canonical form was {:?}", s);
    }

    /// `is_valid(to_string())` holds for every generated value.
    #[test]
    fn display_of_every_value_is_valid(v in edtf()) {
        let s = v.to_string();
        prop_assert!(edtf_core::is_valid(&s), "rejected {:?}", s);
    }

    /// `level()` is 0-2 and stable under round-trip.
    #[test]
    fn level_is_classified_and_roundtrip_stable(v in edtf()) {
        prop_assert!(v.level() <= 2);
        let s = v.to_string();
        let reparsed = Edtf::parse(&s);
        prop_assert!(reparsed.is_ok(), "rejected {:?}", s);
        prop_assert_eq!(reparsed.unwrap().level(), v.level(), "input {:?}", s);
    }

    /// `bounds()` is total (returns without panicking) and ordered:
    /// earliest <= latest whenever both are concrete days.
    #[test]
    fn bounds_are_total_and_ordered(v in edtf()) {
        let b = v.bounds();
        if let (Bound::Date(lo), Bound::Date(hi)) = (b.earliest, b.latest) {
            prop_assert!(lo <= hi, "earliest {} > latest {} for {}", lo, hi, v);
        }
    }

    /// `relation()` is converse-symmetric: swapping the operands maps every
    /// relation to its converse with the same modality (issue #22's
    /// antisymmetry ceremony, strengthened to all six relations).
    #[test]
    fn relation_is_converse_symmetric(a in edtf(), b in edtf()) {
        let ab = a.relation(&b);
        let ba = b.relation(&a);
        for r in Relation::ALL {
            prop_assert_eq!(
                ab.modality(r), ba.modality(r.converse()),
                "{} vs {}: {:?} != converse", a, b, r
            );
        }
    }

    /// The possible-set is sound: never empty, at most one Definite (and
    /// then nothing else possible), Definite only ever Before/After/Equal,
    /// and any Unknown bound on either side means possible-everything.
    #[test]
    fn relation_possible_set_is_sound(a in edtf(), b in edtf()) {
        let rel = a.relation(&b);
        let possible: Vec<Relation> = rel.possible().collect();
        prop_assert!(!possible.is_empty(), "{} vs {}: empty possible set", a, b);
        let definite: Vec<Relation> =
            Relation::ALL.into_iter().filter(|r| rel.is_definite(*r)).collect();
        prop_assert!(definite.len() <= 1, "{} vs {}: two definites", a, b);
        prop_assert_eq!(rel.definite(), definite.first().copied());
        if let Some(d) = rel.definite() {
            prop_assert_eq!(&possible, &[d], "{} vs {}: definite must be sole", a, b);
            prop_assert!(
                matches!(d, Relation::Before | Relation::After | Relation::Equal),
                "{} vs {}: containment/overlap can never be forced", a, b
            );
        }
        let unknown = |bd: Bound| bd == Bound::Unknown;
        if [a.bounds(), b.bounds()]
            .iter()
            .any(|bo| unknown(bo.earliest) || unknown(bo.latest))
        {
            for r in Relation::ALL {
                prop_assert_eq!(
                    rel.modality(r), Modality::Possible,
                    "{} vs {}: Unknown must be possible-everything", a, b
                );
            }
        }
    }

    /// Definite-Before agrees with the bounds regions directly: it holds
    /// exactly when A's latest day precedes B's earliest day and no bound
    /// anywhere is Unknown (Unknown is possible-everything even when the
    /// other bounds alone would settle the comparison).
    #[test]
    fn definite_before_matches_bounds(a in edtf(), b in edtf()) {
        let no_unknown = [a.bounds(), b.bounds()]
            .iter()
            .all(|bo| bo.earliest != Bound::Unknown && bo.latest != Bound::Unknown);
        let expected = no_unknown && matches!(
            (a.bounds().latest, b.bounds().earliest),
            (Bound::Date(hi), Bound::Date(lo)) if hi < lo
        );
        prop_assert_eq!(
            a.relation(&b).is_definite(Relation::Before), expected,
            "{} vs {}", a, b
        );
    }

    /// D18 consistency: any interval the parser accepts as `a/b` must not
    /// relate its endpoints as Definitely-After.
    #[test]
    fn interval_endpoints_never_definitely_after(iv in interval()) {
        let s = Edtf::Interval(iv).to_string();
        let Ok(Edtf::Interval(parsed)) = Edtf::parse(&s) else {
            return Err(TestCaseError::fail(format!("{s} did not reparse as interval")));
        };
        if let (IntervalEndpoint::Date(start), IntervalEndpoint::Date(end)) =
            (parsed.start, parsed.end)
        {
            let rel = Edtf::Date(start).relation(&Edtf::Date(end));
            prop_assert!(
                !rel.is_definite(Relation::After),
                "parser accepted {} yet start is definitely after end", s
            );
        }
    }

    /// Every expression relates to itself reflexively-honestly: Equal is
    /// always possible, and single-day expressions are definitely Equal.
    #[test]
    fn self_relation_admits_equal(v in edtf()) {
        let rel = v.relation(&v);
        prop_assert!(rel.is_possible(Relation::Equal), "{}", v);
        if let (Bound::Date(lo), Bound::Date(hi)) = (v.bounds().earliest, v.bounds().latest) {
            if lo == hi {
                prop_assert_eq!(rel.definite(), Some(Relation::Equal), "{}", v);
            }
        }
    }
}

// ------------------------------------------------- values() (D24-D29)

/// Independent restatement of when a significant-digit sweep overflows:
/// the swept upper magnitude exceeds i64 (computed in i128, so the check
/// does not share the implementation's checked arithmetic).
fn sweep_overflows(value: i64, precision: u32, width: u32) -> bool {
    let sweep = width.saturating_sub(precision).min(38);
    let modulus = 10i128.pow(sweep);
    let mag = i128::from(value.unsigned_abs());
    let lo = mag - mag.rem_euclid(modulus);
    lo + (modulus - 1) > i128::from(i64::MAX)
}

/// The digit width of a year form, restated from public model fields.
fn year_width(kind: &YearKind) -> u32 {
    match kind {
        YearKind::Standard { .. } => 4,
        YearKind::Big { value } => value.unsigned_abs().to_string().len() as u32,
        YearKind::Exponential {
            significand,
            exponent,
        } => significand.unsigned_abs().to_string().len() as u32 + exponent,
    }
}

fn date_unenumerable(d: &Date) -> Option<Unenumerable> {
    if let Some(p) = d.year.significant_digits {
        let Some(value) = d.year.value() else {
            return Some(Unenumerable::YearRangeOverflow);
        };
        if sweep_overflows(value, p, year_width(&d.year.kind)) {
            return Some(Unenumerable::YearRangeOverflow);
        }
    }
    None
}

/// The structural rule for when `values()` must fail, scanning set
/// elements in written order exactly as construction does.
fn expected_unenumerable(v: &Edtf) -> Option<Unenumerable> {
    match v {
        Edtf::Interval(_) => Some(Unenumerable::Interval),
        Edtf::DateTime(_) => None,
        Edtf::Date(d) => date_unenumerable(d),
        Edtf::Set(s) => {
            for e in &s.elements {
                let err = match e {
                    SetElement::OnOrBefore(_) | SetElement::OnOrAfter(_) => {
                        Some(Unenumerable::UnboundedSetElement)
                    },
                    SetElement::Date(d) => date_unenumerable(d),
                    SetElement::Range(a, b) => {
                        if a.year.value().is_none() || b.year.value().is_none() {
                            Some(Unenumerable::YearRangeOverflow)
                        } else {
                            None
                        }
                    },
                };
                if err.is_some() {
                    return err;
                }
            }
            None
        },
    }
}

/// Cap on how much of a (possibly astronomically large) value stream a
/// property consumes per case.
const VALUES_CAP: usize = 600;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// `values()` errs exactly when the structural rule of D24-D25 says so
    /// - total agreement, no other failure mode.
    #[test]
    fn values_err_matches_structure(v in edtf()) {
        prop_assert_eq!(v.values().err(), expected_unenumerable(&v), "{}", v);
    }

    /// Every yielded value round-trips through Display identically, is
    /// concrete (no X digits), and its bounds lie within the input's.
    #[test]
    fn values_are_valid_concrete_and_within_bounds(v in edtf()) {
        let Ok(vals) = v.values() else { return Ok(()); };
        let outer = v.bounds();
        for item in vals.take(VALUES_CAP) {
            let s = item.to_string();
            let reparsed = Edtf::parse(&s);
            prop_assert_eq!(reparsed.as_ref(), Ok(&item), "{} yielded {}", v, s);
            prop_assert!(!item.has_unspecified(), "{} yielded masked {}", v, s);
            let b = item.bounds();
            if let (Bound::Date(olo), Bound::Date(ilo)) = (outer.earliest, b.earliest) {
                prop_assert!(olo <= ilo, "{} yielded {} before earliest bound", v, s);
            }
            if let (Bound::Date(ohi), Bound::Date(ihi)) = (outer.latest, b.latest) {
                prop_assert!(ihi <= ohi, "{} yielded {} after latest bound", v, s);
            }
        }
    }

    /// A single date's values ascend strictly, start exactly at the
    /// expression's earliest bound, and (when the stream is exhausted
    /// within the cap) end exactly at its latest bound.
    #[test]
    fn single_date_values_ascend_and_agree_with_bounds(d in date()) {
        let v = Edtf::Date(d);
        let Ok(mut vals) = v.values() else { return Ok(()); };
        let first = vals.next().expect("D11 guarantees a completion");
        prop_assert_eq!(first.bounds().earliest, v.bounds().earliest, "{}", v);
        let mut last = first;
        let mut exhausted = true;
        for _ in 0..VALUES_CAP {
            match vals.next() {
                None => break,
                Some(item) => {
                    if let (Bound::Date(p), Bound::Date(c)) =
                        (last.bounds().earliest, item.bounds().earliest)
                    {
                        prop_assert!(p < c, "{}: {} not after {}", v, item, last);
                    }
                    last = item;
                }
            }
        }
        if vals.next().is_some() {
            exhausted = false;
        }
        if exhausted {
            prop_assert_eq!(last.bounds().latest, v.bounds().latest, "{}", v);
        }
    }

    /// A set of concrete singleton elements enumerates exactly those
    /// elements, verbatim, in written order (D29).
    #[test]
    fn set_of_singletons_yields_elements_in_order(
        parts in proptest::collection::vec((ymd(), 0u8..=2), 1..=4),
        all in any::<bool>(),
    ) {
        let dates: Vec<Date> = parts
            .iter()
            .map(|((y, m, d), p)| plain_date(i64::from(*y), *m, *d, *p))
            .collect();
        let set = Edtf::Set(Set {
            kind: if all { SetKind::AllMembers } else { SetKind::OneMember },
            elements: dates.iter().copied().map(SetElement::Date).collect(),
        });
        let got: Vec<Edtf> = set.values().unwrap().collect();
        let want: Vec<Edtf> = dates.into_iter().map(Edtf::Date).collect();
        prop_assert_eq!(got, want, "{}", set);
    }

    /// Mask completions equal brute-force enumeration of the whole digit
    /// domain (restricted to <= 2 masked year digits to keep the oracle
    /// small), including qualifier copy-through (D28).
    #[test]
    fn masked_date_values_equal_brute_force(d in month_day_date()) {
        let YearKind::Standard { negative, digits } = d.year.kind else {
            unreachable!("month_day_date generates standard years")
        };
        prop_assume!(digits.iter().filter(|x| x.is_none()).count() <= 2);
        let field_admits = |f: &DateField, v: u8| {
            f.digits[0].is_none_or(|p| p == v / 10) && f.digits[1].is_none_or(|p| p == v % 10)
        };
        let years: Vec<i64> = match d.year.value() {
            Some(v) => vec![v],
            None => (0i64..=9999)
                .filter(|y| {
                    let ds = year_digits(*y as u16);
                    (0..4).all(|i| digits[i].is_none() || digits[i] == ds[i])
                })
                .collect(),
        };
        let month = d.month.expect("month_day_date always has a month");
        let mut want: Vec<Edtf> = Vec::new();
        for y in years {
            for m in (1..=12u8).filter(|m| field_admits(&month, *m)) {
                match d.day {
                    None => want.push(Edtf::Date(Date {
                        year: Year {
                            kind: YearKind::Standard {
                                negative: negative && d.year.value().is_some(),
                                digits: year_digits(y.unsigned_abs() as u16),
                            },
                            significant_digits: None,
                            qualifier: d.year.qualifier,
                        },
                        month: Some(DateField {
                            digits: field_digits(m),
                            qualifier: month.qualifier,
                        }),
                        day: None,
                    })),
                    Some(day) => {
                        for dd in (1..=31u8).filter(|x| field_admits(&day, *x)) {
                            if dd > last_day(m, is_leap(y)) {
                                continue;
                            }
                            want.push(Edtf::Date(Date {
                                year: Year {
                                    kind: YearKind::Standard {
                                        negative: negative && d.year.value().is_some(),
                                        digits: year_digits(y.unsigned_abs() as u16),
                                    },
                                    significant_digits: None,
                                    qualifier: d.year.qualifier,
                                },
                                month: Some(DateField {
                                    digits: field_digits(m),
                                    qualifier: month.qualifier,
                                }),
                                day: Some(DateField {
                                    digits: field_digits(dd),
                                    qualifier: day.qualifier,
                                }),
                            }));
                        }
                    }
                }
            }
        }
        let got: Vec<Edtf> = Edtf::Date(d).values().unwrap().collect();
        prop_assert_eq!(got, want, "input {}", Edtf::Date(d));
    }
}
