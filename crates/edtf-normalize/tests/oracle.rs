//! python-edtf natural-language oracle corpus (issue #20, same ceremony as
//! the interop corpus of issue #12). Fixture:
//! tests/fixtures/natlang/python-edtf-natlang.json.
//!
//! Buckets: `agreements` are enforced to match python-edtf exactly;
//! `agreements_no_match` are `NoMatch` for both; `divergences` are enforced
//! against OUR documented output (each entry cites its N-decision and records
//! the python-edtf-ism we do not adopt); `not_adopted` are python-edtf's
//! substring-extraction and guessing behaviors, enforced as `NoMatch` (N11).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test/bench code: a panic here is the failure signal, not a crash path"
)]

use edtf_normalize::{Outcome, normalize};
use serde_json::Value;

fn fixture() -> Value {
    let raw = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/natlang/python-edtf-natlang.json"
    ));
    serde_json::from_str(raw).expect("fixture must be valid JSON")
}

fn entries<'a>(doc: &'a Value, bucket: &str) -> impl Iterator<Item = &'a Value> + use<'a> {
    doc[bucket]
        .as_array()
        .unwrap_or_else(|| panic!("bucket {bucket} must be an array"))
        .iter()
}

#[test]
fn agreements_match_python_edtf() {
    let doc = fixture();
    for case in entries(&doc, "agreements") {
        let input = case["input"].as_str().expect("input");
        let expected = case["edtf"].as_str().expect("edtf");
        match normalize(input) {
            Outcome::Normalized(n) => {
                assert_eq!(n.edtf, expected, "agreement broken for {input:?}");
            },
            other => panic!("expected Normalized({expected}) for {input:?}, got {other:?}"),
        }
    }
}

#[test]
fn shared_no_match_cases() {
    let doc = fixture();
    for case in entries(&doc, "agreements_no_match") {
        let input = case["input"].as_str().expect("input");
        assert!(
            matches!(normalize(input), Outcome::NoMatch { .. }),
            "expected NoMatch for {input:?}"
        );
    }
}

#[test]
fn divergences_produce_our_documented_output() {
    let doc = fixture();
    for case in entries(&doc, "divergences") {
        let input = case["input"].as_str().expect("input");
        let kind = case["ours_kind"].as_str().expect("ours_kind");
        // Every divergence must cite its N-decision.
        assert!(
            case["decision"]
                .as_str()
                .is_some_and(|d| d.starts_with('N')),
            "divergence {input:?} must cite an N-decision"
        );
        match (kind, normalize(input)) {
            ("normalized", Outcome::Normalized(n)) => {
                let ours = case["ours"].as_str().expect("ours");
                assert_eq!(n.edtf, ours, "divergence drifted for {input:?}");
                // The whole point of diverging: our output differs from theirs.
                assert_ne!(n.edtf, case["theirs"].as_str().expect("theirs"));
            },
            ("ambiguous", Outcome::Ambiguous(a)) => {
                let want: Vec<&str> = case["ours_interpretations"]
                    .as_array()
                    .expect("ours_interpretations")
                    .iter()
                    .map(|v| v.as_str().expect("interpretation string"))
                    .collect();
                let got: Vec<&str> = a.interpretations.iter().map(|i| i.edtf.as_str()).collect();
                assert_eq!(got, want, "divergence drifted for {input:?}");
            },
            (kind, other) => panic!("expected {kind} for {input:?}, got {other:?}"),
        }
    }
}

#[test]
fn not_adopted_behaviors_stay_no_match() {
    let doc = fixture();
    for case in entries(&doc, "not_adopted") {
        let input = case["input"].as_str().expect("input");
        assert!(
            case["ism"].as_str().is_some_and(|s| !s.is_empty()),
            "not_adopted {input:?} must document the ism"
        );
        assert!(
            matches!(normalize(input), Outcome::NoMatch { .. }),
            "expected NoMatch for {input:?} (python-edtf: {:?})",
            case["theirs"].as_str()
        );
    }
}

#[test]
fn every_normalized_or_ambiguous_output_parses_in_core() {
    // Belt and braces over the whole corpus: anything we emit must parse.
    let doc = fixture();
    let inputs = entries(&doc, "agreements")
        .chain(entries(&doc, "divergences"))
        .filter_map(|c| c["input"].as_str());
    for input in inputs {
        match normalize(input) {
            Outcome::Normalized(n) => {
                let parsed = edtf_core::Edtf::parse(&n.edtf)
                    .unwrap_or_else(|e| panic!("{input:?} emitted unparsable {:?}: {e}", n.edtf));
                assert_eq!(parsed, n.value);
            },
            Outcome::Ambiguous(a) => {
                for i in &a.interpretations {
                    let parsed = edtf_core::Edtf::parse(&i.edtf).unwrap_or_else(|e| {
                        panic!("{input:?} emitted unparsable {:?}: {e}", i.edtf)
                    });
                    assert_eq!(parsed, i.value);
                }
            },
            Outcome::NoMatch { .. } => {},
        }
    }
}
