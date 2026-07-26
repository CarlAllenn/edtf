//! Property tests: the "valid by construction" guarantee from issue #20.
//!
//! Generators are constructive (build the prose from a known date, then
//! assert the exact expected EDTF), mirroring edtf-core's props.rs strategy —
//! nothing is filtered, so shrinking stays meaningful. Every asserted output
//! is round-tripped through `edtf_core::Edtf::parse` and its level checked;
//! that re-parse is what makes the guarantee real, since core has no
//! validating constructors.

use edtf_core::Edtf;
use edtf_normalize::{normalize, normalize_with, Language, Options, Outcome};
use proptest::prelude::*;

/// Assert a Normalized outcome and re-parse the output in core.
fn assert_normalized(input: &str, expected: &str, opts: &Options) {
    match normalize_with(input, opts) {
        Outcome::Normalized(n) => {
            assert_eq!(n.edtf, expected, "input: {input:?}");
            let parsed = Edtf::parse(&n.edtf).expect("output must parse in core");
            assert_eq!(parsed, n.value, "value/edtf mismatch for {input:?}");
        }
        other => panic!("expected Normalized for {input:?}, got {other:?}"),
    }
}

fn en() -> Options {
    Options::default()
}

fn ru() -> Options {
    Options {
        language: Language::Russian,
        ..Options::default()
    }
}

const MONTHS_EN: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

const MONTHS_RU: [&str; 12] = [
    "января",
    "февраля",
    "марта",
    "апреля",
    "мая",
    "июня",
    "июля",
    "августа",
    "сентября",
    "октября",
    "ноября",
    "декабря",
];

proptest! {
    #[test]
    fn plain_years_round_trip(y in 100i32..=9999) {
        assert_normalized(&format!("{y}"), &format!("{y:04}"), &en());
    }

    #[test]
    fn circa_years(y in 100i32..=9999) {
        assert_normalized(&format!("circa {y}"), &format!("{y:04}~"), &en());
        assert_normalized(&format!("около {y}"), &format!("{y:04}~"), &ru());
    }

    #[test]
    fn bc_years_use_astronomical_numbering(b in 2u32..=9999) {
        // N3: year b BC = astronomical -(b-1).
        let astro = -(b as i32 - 1);
        assert_normalized(&format!("{b} BC"), &format!("-{:04}", -astro), &en());
        assert_normalized(&format!("{b} до н. э."), &format!("-{:04}", -astro), &ru());
    }

    #[test]
    fn decades_mask_the_final_digit(p3 in 100u16..=999) {
        prop_assume!(p3 % 10 != 0); // '..00s' forms are deliberately ambiguous (N6)
        assert_normalized(&format!("{p3}0s"), &format!("{p3}X"), &en());
        assert_normalized(&format!("{p3}0-е"), &format!("{p3}X"), &ru());
    }

    #[test]
    fn centuries_are_off_by_one(n in 2u32..=20) {
        // N2: the nth century masks as (n-1)XX.
        assert_normalized(&format!("{n}th century"), &format!("{:02}XX", n - 1), &en());
        assert_normalized(&format!("{n} век"), &format!("{:02}XX", n - 1), &ru());
    }

    #[test]
    fn century_parts_partition_the_century(n in 2u32..=20) {
        // N1: early ∪ mid ∪ late covers the century exactly, as does
        // first half ∪ second half.
        let s = (n as i32 - 1) * 100 + 1;
        let e = n as i32 * 100;
        assert_normalized(&format!("early {n}th century"), &format!("{s:04}/{:04}", s + 29), &en());
        assert_normalized(&format!("mid {n}th century"), &format!("{:04}/{:04}", s + 30, s + 69), &en());
        assert_normalized(&format!("late {n}th century"), &format!("{:04}/{e:04}", s + 70), &en());
        assert_normalized(&format!("first half of the {n}th century"), &format!("{s:04}/{:04}", s + 49), &en());
        assert_normalized(&format!("second half of the {n}th century"), &format!("{:04}/{e:04}", s + 50), &en());
    }

    #[test]
    fn month_year_forms(y in 1000i32..=9999, m in 0usize..12) {
        let expected = format!("{y:04}-{:02}", m + 1);
        assert_normalized(&format!("{} {y}", MONTHS_EN[m]), &expected, &en());
        assert_normalized(&format!("{} {y} года", MONTHS_RU[m]), &expected, &ru());
    }

    #[test]
    fn unambiguous_numeric_dates(y in 1000i32..=9999, m in 1u32..=12, d in 13u32..=28) {
        // d > 12 proves the order; d ≤ 28 is valid in every month (N5).
        let expected = format!("{y:04}-{m:02}-{d:02}");
        assert_normalized(&format!("{d}/{m}/{y}"), &expected, &en());
        assert_normalized(&format!("{d}.{m}.{y}"), &expected, &ru());
    }

    #[test]
    fn ambiguous_numeric_dates_always_report_both(
        y in 1000i32..=9999, a in 1u32..=12, b in 1u32..=12
    ) {
        prop_assume!(a != b);
        match normalize(&format!("{a}/{b}/{y}")) {
            Outcome::Ambiguous(amb) => {
                assert_eq!(amb.interpretations.len(), 2);
                for i in &amb.interpretations {
                    let parsed = Edtf::parse(&i.edtf).expect("interpretation must parse");
                    assert_eq!(parsed, i.value);
                }
            }
            other => panic!("expected Ambiguous, got {other:?}"),
        }
    }

    #[test]
    fn year_ranges(a in 1000i32..=9998, span in 1i32..=500) {
        let b = (a + span).min(9999);
        assert_normalized(&format!("from {a} to {b}"), &format!("{a:04}/{b:04}"), &en());
        assert_normalized(&format!("с {a} по {b}"), &format!("{a:04}/{b:04}"), &ru());
    }

    #[test]
    fn before_after_open_intervals(y in 1000i32..=9999) {
        assert_normalized(&format!("before {y}"), &format!("../{y:04}"), &en());
        assert_normalized(&format!("after {y}"), &format!("{y:04}/.."), &en());
        assert_normalized(&format!("до {y}"), &format!("../{y:04}"), &ru());
        assert_normalized(&format!("после {y}"), &format!("{y:04}/.."), &ru());
    }

    #[test]
    fn every_outcome_is_internally_consistent(y in 1000i32..=9999, m in 1u32..=12, d in 1u32..=31) {
        // Over arbitrary numeric input (valid or not): whatever comes back,
        // every emitted string parses in core and equals its value.
        match normalize(&format!("{d}/{m}/{y}")) {
            Outcome::Normalized(n) => {
                let parsed = Edtf::parse(&n.edtf).expect("must parse");
                assert_eq!(parsed, n.value);
            }
            Outcome::Ambiguous(amb) => {
                assert!(amb.interpretations.len() >= 2, "ambiguity needs 2+ readings");
                for i in &amb.interpretations {
                    let parsed = Edtf::parse(&i.edtf).expect("must parse");
                    assert_eq!(parsed, i.value);
                }
            }
            Outcome::NoMatch { .. } => {} // Feb 30 and friends fail closed
        }
    }
}
