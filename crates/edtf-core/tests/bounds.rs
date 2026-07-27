//! Bounds tests: spec-derived expectations plus the legacy corpus's
//! earliest/latest fields wherever they are concrete dates or infinities.
//! (Corpus rows with null bounds encode a Postgres-layer clamping convention
//! that belongs to the database wrapper, not the core — those are skipped
//! here and covered by the explicit cases below.)

use edtf_core::{Bound, Edtf};
use serde_json::Value;

fn bound_str(b: Bound) -> String {
    match b {
        Bound::Date(d) => {
            // Corpus format: big years unpadded, 4-digit years padded.
            if d.year > 9999 {
                format!("{}-{:02}-{:02}", d.year, d.month, d.day)
            } else {
                d.to_string()
            }
        },
        Bound::NegativeInfinity => "-infinity".into(),
        Bound::PositiveInfinity => "infinity".into(),
        Bound::Unknown => "unknown".into(),
    }
}

fn assert_bounds(input: &str, earliest: &str, latest: &str) {
    let b = Edtf::parse(input)
        .unwrap_or_else(|e| panic!("{input:?} should parse: {e}"))
        .bounds();
    assert_eq!(bound_str(b.earliest), earliest, "earliest of {input:?}");
    assert_eq!(bound_str(b.latest), latest, "latest of {input:?}");
}

#[test]
fn corpus_concrete_bounds_match() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/legacy/edtf-conformance-corpus.json"
    );
    let c: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    for section in ["level0", "level1", "level2"] {
        for case in c[section].as_array().unwrap() {
            let s = case["edtf"].as_str().unwrap();
            let (Some(earliest), Some(latest)) =
                (case["earliest"].as_str(), case["latest"].as_str())
            else {
                continue; // null bounds = archive DB convention, not core
            };
            assert_bounds(s, earliest, latest);
        }
    }
}

#[test]
fn masked_digit_bounds() {
    assert_bounds("156X", "1560-01-01", "1569-12-31");
    assert_bounds("XXXX", "0000-01-01", "9999-12-31"); // four-digit by spec (9.2.1.2 c4)
    assert_bounds("1XX3", "1003-01-01", "1993-12-31");
    assert_bounds("1560-X2", "1560-02-01", "1560-12-31");
    assert_bounds("1985-0X-31", "1985-01-31", "1985-08-31"); // 09 has 30 days
    assert_bounds("XXXX-02-29", "0000-02-29", "9996-02-29"); // leap completions only
    assert_bounds("196X-02-29", "1960-02-29", "1968-02-29");
    assert_bounds("XXXX-12-XX", "0000-12-01", "9999-12-31");
}

#[test]
fn season_bounds_full_table() {
    assert_bounds("2001-21", "2001-03-01", "2001-05-31"); // spring
    assert_bounds("2001-22", "2001-06-01", "2001-08-31"); // summer
    assert_bounds("2001-23", "2001-09-01", "2001-11-30"); // autumn
    assert_bounds("2001-24", "2001-12-01", "2002-02-28"); // winter wraps; 2002 common
    assert_bounds("2003-24", "2003-12-01", "2004-02-29"); // wraps into leap year
    assert_bounds("2001-29", "2001-09-01", "2001-11-30"); // southern spring
    assert_bounds("2001-30", "2001-12-01", "2002-02-28"); // southern summer wraps
    assert_bounds("2001-33", "2001-01-01", "2001-03-31"); // Q1
    assert_bounds("2001-36", "2001-10-01", "2001-12-31"); // Q4
    assert_bounds("2001-37", "2001-01-01", "2001-04-30"); // quadrimester 1
    assert_bounds("2001-39", "2001-09-01", "2001-12-31"); // quadrimester 3
    assert_bounds("2001-40", "2001-01-01", "2001-06-30"); // semestral 1
    assert_bounds("2001-41", "2001-07-01", "2001-12-31"); // semestral 2
}

#[test]
fn significant_digit_bounds() {
    assert_bounds("1950S2", "1900-01-01", "1999-12-31");
    assert_bounds("1950S4", "1950-01-01", "1950-12-31");
    // Annex A.6.3 Example 2 prints "171010000-171010999", contradicting the
    // normative rule in §4.4.3 (keep the leftmost P digits — its own Example 2)
    // and the LoC EDTF original. The normative clause wins (spec-notes D19).
    assert_bounds("Y171010000S3", "171000000-01-01", "171999999-12-31");
    assert_bounds("Y3388E2S3", "338000-01-01", "338999-12-31");
    assert_bounds("Y17E7", "170000000-01-01", "170000000-12-31");
    // Found by cargo-fuzz: 9e18 fits in i64 but the S1 sweep's upper end
    // (9999999999999999999) does not — must degrade to unknown, not panic.
    assert_bounds("Y9E18S1", "unknown", "unknown");
    // Parsing computes bounds for interval ordering (D18); huge exponents on
    // both ends must stay total there too.
    assert!(edtf_core::Edtf::parse("Y8E20202/Y9E18S1").is_ok());
}

#[test]
fn negative_year_bounds() {
    assert_bounds("-1985", "-1985-01-01", "-1985-12-31");
    assert_bounds("-0001", "-0001-01-01", "-0001-12-31");
    assert_bounds("-1985-04-12", "-1985-04-12", "-1985-04-12");
    // Year 0 and -4 are leap years in the astronomical/proleptic reckoning.
    assert_bounds("0000-02-29", "0000-02-29", "0000-02-29");
    assert!(edtf_core::is_valid("-0004-02-29"));
    assert!(!edtf_core::is_valid("-0003-02-29"));
}

#[test]
fn interval_and_set_bounds() {
    assert_bounds("1985-04-12/..", "1985-04-12", "infinity");
    assert_bounds("../1985-04-12", "-infinity", "1985-04-12");
    assert_bounds("1986-04/", "1986-04-01", "unknown");
    assert_bounds("/1985", "unknown", "1985-12-31");
    assert_bounds("..2004-06-01/2004-06-20..", "-infinity", "infinity");
    assert_bounds("[..1984]", "-infinity", "1984-12-31");
    assert_bounds("[1984..]", "1984-01-01", "infinity");
    assert_bounds("{1667,1668,1670..1672}", "1667-01-01", "1672-12-31");
    assert_bounds("{1961-12,1960}", "1960-01-01", "1961-12-31"); // order-independent
}

#[test]
fn qualification_does_not_move_bounds() {
    assert_bounds("1985~", "1985-01-01", "1985-12-31");
    assert_bounds("2004-06-~01/2004-06-~20", "2004-06-01", "2004-06-20");
}

#[test]
fn interval_ordering_enforced() {
    // Decision D18: an interval whose end cannot reach its start is invalid.
    assert!(!edtf_core::is_valid("2004/2003"));
    assert!(!edtf_core::is_valid("2004-06-11/2004-06-10"));
    assert!(!edtf_core::is_valid("{1672..1670}"));
    // Overlap suffices — these remain valid:
    assert!(edtf_core::is_valid("2004-06/2004"));
    assert!(edtf_core::is_valid("198X/1985")); // 1980 ≤ 1985 possible
    assert!(edtf_core::is_valid("2004-02-01/2005-02-08"));
}
