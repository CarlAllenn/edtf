// SPDX-FileCopyrightText: Copyright (c) the edtf contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Conformance tests against the legacy corpus (built from every ISO 8601-2
//! Annex A example + adversarial cases). The corpus is the cross-check
//! oracle; docs/spec-notes.md is the authority on any disagreement.

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
#![expect(
    clippy::absolute_paths,
    reason = "a one-use std path written in full at the call site"
)]
#![expect(clippy::panic, reason = "a panic in a test IS the failure signal")]

use edtf_core::Edtf;
use serde_json::Value;

fn corpus() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/legacy/edtf-conformance-corpus.json"
    );
    let text = std::fs::read_to_string(path).expect("read conformance corpus");
    serde_json::from_str(&text).expect("parse conformance corpus")
}

#[test]
fn corpus_valid_cases_parse_at_expected_level() {
    let c = corpus();
    for (section, want_level) in [("level0", 0_u8), ("level1", 1), ("level2", 2)] {
        for case in c[section].as_array().expect("section is an array") {
            let s = case["edtf"].as_str().expect("edtf field");
            match Edtf::parse(s) {
                Ok(parsed) => assert_eq!(
                    parsed.level(),
                    want_level,
                    "wrong level for {s:?} ({})",
                    case["note"].as_str().unwrap_or("")
                ),
                Err(e) => panic!(
                    "{s:?} should be valid ({section}: {}): {e}",
                    case["note"].as_str().unwrap_or("")
                ),
            }
        }
    }
}

#[test]
fn corpus_invalid_cases_reject() {
    let c = corpus();
    for case in c["invalid"].as_array().expect("invalid is an array") {
        let s = case["edtf"].as_str().expect("edtf field");
        assert!(
            Edtf::parse(s).is_err(),
            "{s:?} should be rejected ({})",
            case["note"].as_str().unwrap_or("")
        );
    }
}
