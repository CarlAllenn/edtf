//! Canonical formatting tests: exact renderings plus the round-trip
//! property over every valid string in both fixture corpora.

use edtf_core::Edtf;
use serde_json::Value;

fn canon(s: &str) -> String {
    Edtf::parse(s)
        .unwrap_or_else(|e| panic!("{s:?} should parse: {e}"))
        .to_string()
}

#[test]
fn canonical_forms_stay_canonical() {
    // Already-preferred spellings render unchanged.
    for s in [
        "1985-04-12",
        "1985-04",
        "1985",
        "-1985",
        "Y170000002",
        "Y-17E7",
        "1950S2",
        "2001-21",
        "1985-04-12?",
        "1985~",
        "1985%",
        "2004-06~-11",
        "2004?-06-11",
        "2004-%06-11",
        "1985-04-XX",
        "156X-12-25",
        "1985-04-12T23:20:30Z",
        "1985-04-12T23:20:30+04:30",
        "1985-04-12T23:20:30+04",
        "1985-04-12/..",
        "../1985-04-12",
        "1986-04/",
        "/1985",
        "..2004-06-01/2004-06-20..",
        "{1960,1961-12}",
        "[1667,1760-12]",
        "[..1984]",
        "{1667,1668,1670..1672}",
    ] {
        assert_eq!(canon(s), s, "canonical form should be stable");
    }
}

#[test]
fn preferred_form_normalization() {
    // ISO 8601-2 §8.2.4: complete > group > individual, no redundancy.
    assert_eq!(canon("?2004-?06-?11"), "2004-06-11?");
    assert_eq!(canon("?2004-?06-11"), "2004-06?-11");
    assert_eq!(canon("2004-06-11%"), "2004-06-11%");
    // Group beats individual (8.2.4 Ex.2): year-only qual renders as `2004?-…`.
    assert_eq!(canon("?2004-06-~11"), "2004?-06-~11");
    assert_eq!(canon("2004?-06-?11"), "2004?-06-?11"); // gap prevents grouping
}

#[test]
fn roundtrip_all_fixture_strings() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/legacy/edtf-conformance-corpus.json"
    );
    let c: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    for section in ["level0", "level1", "level2"] {
        for case in c[section].as_array().unwrap() {
            let s = case["edtf"].as_str().unwrap();
            let parsed = Edtf::parse(s).unwrap();
            let rendered = parsed.to_string();
            let reparsed = Edtf::parse(&rendered).unwrap_or_else(|e| {
                panic!("canonical form {rendered:?} of {s:?} must reparse: {e}")
            });
            assert_eq!(
                parsed, reparsed,
                "round-trip must preserve semantics for {s:?} (rendered {rendered:?})"
            );
            assert_eq!(parsed.level(), reparsed.level(), "level stable for {s:?}");
        }
    }
}
