// SPDX-FileCopyrightText: Copyright (c) the edtf contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Spec-derived tests beyond the legacy corpus: Annex A examples, the
//! reject list (spec-notes §5), and the D1-D17 decisions (spec-notes §9).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test/bench code: a panic here is the failure signal, not a crash path"
)]

use edtf_core::{Edtf, IntervalEndpoint, Precision, SetKind, is_valid, level};

fn assert_level(s: &str, want: u8) {
    assert_eq!(level(s), Some(want), "level of {s:?}");
}

fn assert_invalid(s: &str) {
    assert!(!is_valid(s), "{s:?} should be invalid");
}

#[test]
fn level0_dates_and_datetimes() {
    assert_level("1985-04-12", 0);
    assert_level("1985-04", 0);
    assert_level("1985", 0);
    assert_level("0000", 0); // year zero exists
    assert_level("2000-02-29", 0); // leap day, leap year
    assert_level("1985-04-12T23:20:30", 0);
    assert_level("1985-04-12T23:59:60Z", 0); // leap second accepted (D3)
    assert_level("1985-04-12T23:20:30+14:00", 0);
}

#[test]
fn level1_features() {
    assert_level("-1985", 1);
    assert_level("-0001", 1); // year before 0000
    assert_level("Y170000002", 1);
    assert_level("Y-170000002", 1);
    assert_level("2001-21", 1);
    assert_level("2001-24", 1);
    assert_level("1985-04-12?", 1);
    assert_level("1985-04?", 1);
    assert_level("1985~", 1);
    assert_level("1985%", 1);
    assert_level("1985-04-XX", 1);
    assert_level("1985-XX-XX", 1);
    assert_level("2004-XX", 1);
    assert_level("201X", 1);
    assert_level("20XX", 1);
    assert_level("1985-04-12/..", 1);
    assert_level("../1985-04-12", 1);
    assert_level("1986-04/", 1);
    assert_level("/1985", 1);
    assert_level("1984?/2004%", 1);
    assert_level("../1985-04-12?", 1);
    assert_level("1985-04-12~/", 1);
}

#[test]
fn level2_features() {
    assert_level("Y-17E7", 2);
    assert_level("Y3388E2S3", 2);
    assert_level("1950S2", 2);
    assert_level("Y171010000S3", 2);
    assert_level("2001-34", 2); // quarter 2
    assert_level("2001-41", 2); // semestral 2
    assert_level("2004-06~-11", 2);
    assert_level("2004?-06-11", 2);
    assert_level("?2004-06-~11", 2);
    assert_level("2004-%06-11", 2);
    assert_level("2004-06?-~11", 2); // mixed group + individual (8.4.6)
    assert_level("156X-12-25", 2);
    assert_level("XXXX-12-XX", 2);
    assert_level("1XXX-XX", 2);
    assert_level("1560-X2", 2);
    assert_level("XXXX", 2);
    assert_level("2XXX", 2);
    assert_level("1XX3", 2);
    assert_level("{1960,1961-12}", 2);
    assert_level("[1667,1760-12]", 2);
    assert_level("[..1984]", 2);
    assert_level("[1984..]", 2);
    assert_level("[1670..1672]", 2);
    assert_level("{..1983-12-31,1984-10-10..1984-11-01,1984-11-05..}", 2);
    assert_level("2004-06-~01/2004-06-~20", 2);
    assert_level("..2004-06-01/2004-06-20..", 2);
    assert_level("2004-06-XX/2004-07-03", 2); // X in interval endpoint (D16)
}

#[test]
fn calendar_validation() {
    assert_invalid("1985-02-30");
    assert_invalid("1985-04-31");
    assert_invalid("2001-02-29"); // not a leap year
    assert_invalid("1900-02-29"); // centennial, not leap
    assert!(is_valid("2000-02-29")); // divisible by 400
    assert!(is_valid("1985-0X-31")); // months 01,03,05,07,08 complete it (D11)
    assert_invalid("1985-02-3X"); // no completion: Feb has no day 30-39
    assert_invalid("X900-02-29"); // no X900 year is a leap year
    assert!(is_valid("XXXX-02-29")); // some leap year exists
    assert_invalid("1985-00");
    assert_invalid("1985-13");
    assert_invalid("2004-20"); // 13-20 unassigned
    assert_invalid("1985-04-00");
    assert_invalid("1985-04-32");
    assert_invalid("2001-21-05"); // seasons cannot carry a day
}

#[test]
fn reject_list_not_edtf() {
    assert_invalid("19850412"); // basic format
    assert_invalid("1985-102"); // ordinal date
    assert_invalid("1985-W15"); // week date
    assert_invalid("P1Y2M"); // duration
    assert_invalid("R12/1985/1990"); // recurring interval
    assert_invalid("1985Y4M12D"); // explicit form
    assert_invalid("198"); // bare decade
    assert_invalid("19"); // bare century
    assert_invalid("2018-01-15/02-20"); // end-component elision
    assert_invalid("1985-4-12"); // single-digit month
    assert_invalid("1985-04-12T23:20"); // reduced-precision time
    assert_invalid("1985-04-12T24:00:00"); // hour 24
    assert_invalid("1985-04-12T23:20:30+15:00"); // shift beyond ±14:00
    assert_invalid("1985-04-12T23:20:30-00:00"); // negative zero shift (D4)
    assert_invalid("1985-04-12T23:20:30-00");
    assert_invalid("hello");
    assert_invalid("");
    assert_invalid("1985 -04"); // whitespace
    assert_invalid("..1984"); // bare '..' outside sets/intervals
}

#[test]
fn decisions() {
    assert_invalid("Y1985"); // D1: Y-form needs |year| > 9999
    assert_invalid("Y-1985");
    assert_invalid("Y12E2"); // D1: resolves to 1200
    assert_invalid("-0000"); // D2
    assert_invalid("../.."); // D6: no dated endpoint
    assert_invalid("/");
    assert_invalid("../");
    assert_invalid("/..");
    assert_invalid("1950S0"); // D7: precision bounds
    assert_invalid("1950S5");
    assert!(is_valid("1950S4"));
    assert_invalid("195XS2"); // S cannot combine with X
    assert_invalid("-12345"); // D13: >4-digit negative needs Y-form
    assert!(is_valid("-1985-04-12")); // D17: negative year with month/day
    assert_level("-1985-04-12", 1);
    assert_invalid("Y20000-04"); // Y-years are year-only

    // D20: no leading zeros in Y-year significands. Found by cargo-fuzz:
    // "Y08470847E1S9" used to parse with a 9-digit S-budget, then its
    // canonical zero-stripped form failed to reparse.
    assert_invalid("Y08470847E1S9");
    assert_invalid("Y-01694194E1S9");
    assert_invalid("Y018470");
    assert_invalid("Y012E3");
    assert!(is_valid("Y8470847E1S8"));
    assert_invalid("{}");
    assert_invalid("{1960,}");
    assert_invalid("{1960, 1961}"); // space
    assert_invalid("[..]");
    assert_invalid("1985-04-12/1985-04-12T10:00:00"); // datetime interval endpoint
}

#[test]
fn model_accessors() {
    let d = match Edtf::parse("2004-06~-11").unwrap() {
        Edtf::Date(d) => d,
        other => panic!("expected date, got {other:?}"),
    };
    assert!(d.year.qualifier.approximate);
    assert!(d.month.unwrap().qualifier.approximate);
    assert!(!d.day.unwrap().qualifier.is_qualified());
    assert_eq!(d.precision(), Precision::Day);
    assert_eq!(d.year.value(), Some(2004));

    let season = match Edtf::parse("2001-21").unwrap() {
        Edtf::Date(d) => d,
        other => panic!("expected date, got {other:?}"),
    };
    assert_eq!(season.precision(), Precision::Season);

    let iv = match Edtf::parse("1985-04-12/..").unwrap() {
        Edtf::Interval(iv) => iv,
        other => panic!("expected interval, got {other:?}"),
    };
    assert!(matches!(iv.end, IntervalEndpoint::Open));

    let set = match Edtf::parse("[1667,1760-12]").unwrap() {
        Edtf::Set(s) => s,
        other => panic!("expected set, got {other:?}"),
    };
    assert_eq!(set.kind, SetKind::OneMember);
    assert_eq!(set.elements.len(), 2);
}

#[test]
fn error_offsets_point_at_the_problem() {
    fn offset_of(s: &str) -> usize {
        Edtf::parse(s).unwrap_err().offset
    }
    assert_eq!(offset_of("1985-13"), 5); // the bad month
    assert_eq!(offset_of("1985-02-30"), 8); // the bad day
    assert_eq!(offset_of("1985-04-12T25:00:00"), 11); // the bad hour
    assert_eq!(offset_of("1985-04-12T23:20:30+15:00"), 19); // the bad shift
    assert_eq!(offset_of("1985 -04"), 4); // the space
    assert_eq!(offset_of("2004/2003"), 5); // the out-of-order end
    assert_eq!(offset_of("{1960,1985-00}"), 11); // bad month inside a set
    assert_eq!(offset_of("../1985-13"), 8); // bad month after '..' prefix
}

#[test]
fn rejection_edges_carry_precise_errors() {
    fn message_of(s: &str) -> String {
        Edtf::parse(s).unwrap_err().to_string()
    }
    // Exponent domain cap (the exponent itself, not the resulting year).
    assert!(message_of("Y1E100001").contains("exponent out of supported range"));
    // Shift minutes are a separate field from shift magnitude (±14:00).
    assert!(message_of("1985-04-12T10:00:00+05:60").contains("shift minutes must be 00-59"));
    // A set range is a single '..' between two dates.
    assert!(message_of("[1985..1990..1995]").contains("multiple '..'"));
}

#[test]
fn component_flags_reach_into_sets() {
    // The `any_date` sweep must see every set element shape.
    assert!(Edtf::parse("[1985?]").unwrap().is_uncertain());
    assert!(Edtf::parse("[..1985~]").unwrap().is_approximate());
    assert!(Edtf::parse("[198X]").unwrap().has_unspecified());
    // Range endpoints reject qualifiers and masks at parse time, so the
    // sweep across a range can only ever come back clean.
    assert!(!Edtf::parse("[1985..1990]").unwrap().is_uncertain());
}

#[test]
fn edtf_implements_fromstr() {
    let v: Edtf = "1985-04".parse().unwrap();
    assert_eq!(v.to_string(), "1985-04");
    "1985-13".parse::<Edtf>().unwrap_err();
}

#[test]
fn component_flags_reach_into_interval_endpoints() {
    // Open/unknown endpoints carry no date: the sweep must fall through to
    // the other endpoint, and short-circuit when the first already matches.
    assert!(Edtf::parse("../1985?").unwrap().is_uncertain());
    assert!(Edtf::parse("1985?/..").unwrap().is_uncertain());
    assert!(!Edtf::parse("../1985").unwrap().is_uncertain());
}
