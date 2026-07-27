//! Interop cross-check against the test suites of the reference
//! implementations the ecosystem runs: edtf.js (`LoC`'s de facto reference)
//! and python-edtf. Rules, same as the `LoC` corpus: ISO 8601-2:2019 Annex A
//! wins on any disagreement; every divergence is a D-decision in
//! docs/spec-notes.md (D3, D5, D6, D15, D17, D21, D22); implementation
//! extensions are recorded as must-rejects, not adopted.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test/bench code: a panic here is the failure signal, not a crash path"
)]

use edtf_core::Edtf;
use serde_json::Value;

fn corpus() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/interop/reference-impl-corpus.json"
    );
    let text = std::fs::read_to_string(path).expect("read interop corpus");
    serde_json::from_str(&text).expect("parse interop corpus")
}

fn sources(case: &Value) -> String {
    case["sources"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join("+")
        })
        .unwrap_or_default()
}

/// Accepted by the reference implementations and by us: parse at the
/// ISO-derived level and survive a canonical round trip.
#[test]
fn shared_accepts_parse_at_iso_level_and_round_trip() {
    let c = corpus();
    for case in c["accepts"].as_array().expect("accepts is an array") {
        let s = case["edtf"].as_str().expect("edtf field");
        let want = case["our_level"].as_u64().expect("our_level field");
        let src = sources(case);

        let parsed = Edtf::parse(s)
            .unwrap_or_else(|e| panic!("interop example {s:?} ({src}) should parse: {e}"));
        assert_eq!(
            u64::from(parsed.level()),
            want,
            "wrong level for interop example {s:?} ({src})"
        );

        let rendered = parsed.to_string();
        let reparsed = Edtf::parse(&rendered).unwrap_or_else(|e| {
            panic!("interop example {s:?}: canonical form {rendered:?} fails to reparse: {e}")
        });
        assert_eq!(parsed, reparsed, "round trip changed meaning for {s:?}");
    }
}

/// Rejected by the reference implementations and by ISO: stay rejected.
#[test]
fn shared_rejects_stay_rejected() {
    let c = corpus();
    for case in c["rejects"].as_array().expect("rejects is an array") {
        let s = case["edtf"].as_str().expect("edtf field");
        assert!(
            Edtf::parse(s).is_err(),
            "{s:?} should be rejected ({}, {})",
            sources(case),
            case["reason"].as_str().unwrap_or("")
        );
    }
}

/// Accepted by a reference implementation but not by ISO Annex A: each is a
/// named implementation-ism and must stay rejected here.
#[test]
fn implementation_isms_must_reject() {
    let c = corpus();
    for case in c["must_reject"]
        .as_array()
        .expect("must_reject is an array")
    {
        let s = case["edtf"].as_str().expect("edtf field");
        assert!(
            Edtf::parse(s).is_err(),
            "{s:?} is a {} extension and must be rejected: {}",
            sources(case),
            case["ism"].as_str().unwrap_or("")
        );
    }
}

/// Rejected by a reference implementation but accepted here with ISO
/// backing: the direct-evidence divergences, each tied to a D-decision.
#[test]
fn documented_divergences_parse_at_their_level() {
    let c = corpus();
    for case in c["we_accept_they_reject"]
        .as_array()
        .expect("we_accept_they_reject is an array")
    {
        let s = case["edtf"].as_str().expect("edtf field");
        let want = case["our_level"].as_u64().expect("our_level field");
        let decision = case["decision"].as_str().unwrap_or("");

        let parsed = Edtf::parse(s).unwrap_or_else(|e| {
            panic!("divergence case {s:?} ({decision}) should parse here: {e}")
        });
        assert_eq!(
            u64::from(parsed.level()),
            want,
            "wrong level for divergence case {s:?} ({decision})"
        );
    }
}
