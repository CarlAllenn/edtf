// SPDX-FileCopyrightText: Copyright (c) the edtf contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Interop cross-check against every example on the Library of Congress
//! EDTF specification page (loc.gov/standards/datetime) — the original that
//! the ISO 8601-2:2019 profile codifies. Where `LoC` and ISO disagree, ISO
//! Annex A wins; divergences are recorded in docs/spec-notes.md (D19, D20)
//! and as `our_level` overrides in the fixture.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test/bench code: a panic here is the failure signal, not a crash path"
)]
#![expect(
    clippy::tests_outside_test_module,
    reason = "an integration test under tests/ is compiled as its own crate whose every item is test support, so there is no non-test code for a mod tests to separate it from"
)]
#![expect(
    clippy::min_ident_chars,
    reason = "the test bodies use the same y/m/d date-component names as the code they exercise"
)]
#![expect(
    clippy::indexing_slicing,
    reason = "indexing a fixture the test itself constructed; an out-of-range index is a failing test, not a crash path"
)]
#![expect(clippy::panic, reason = "a panic in a test IS the failure signal")]
#![expect(
    clippy::absolute_paths,
    reason = "a one-use std path written in full at the call site"
)]

use edtf_core::Edtf;
use serde_json::Value;

fn corpus() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/loc/loc-edtf-examples.json"
    );
    let text = std::fs::read_to_string(path).expect("read LoC corpus");
    serde_json::from_str(&text).expect("parse LoC corpus")
}

#[test]
fn loc_examples_parse_at_their_level_and_round_trip() {
    let c = corpus();
    for case in c["examples"].as_array().expect("examples is an array") {
        let s = case["edtf"].as_str().expect("edtf field");
        let feature = case["feature"].as_str().unwrap_or("");
        // ISO-derived level; falls back to LoC's own level when they agree.
        let want = case["our_level"]
            .as_u64()
            .or_else(|| case["level"].as_u64())
            .expect("level field");

        let parsed = Edtf::parse(s)
            .unwrap_or_else(|e| panic!("LoC example {s:?} ({feature}) should parse: {e}"));
        assert_eq!(
            u64::from(parsed.level()),
            want,
            "wrong level for LoC example {s:?} ({feature})"
        );

        let rendered = parsed.to_string();
        let reparsed = Edtf::parse(&rendered).unwrap_or_else(|e| {
            panic!("LoC example {s:?}: canonical form {rendered:?} fails to reparse: {e}")
        });
        assert_eq!(parsed, reparsed, "round trip changed meaning for {s:?}");
    }
}

#[test]
fn loc_invalid_cases_reject() {
    let c = corpus();
    for case in c["invalid"].as_array().expect("invalid is an array") {
        let s = case["edtf"].as_str().expect("edtf field");
        assert!(
            Edtf::parse(s).is_err(),
            "{s:?} should be rejected ({})",
            case["feature"].as_str().unwrap_or("")
        );
    }
}
