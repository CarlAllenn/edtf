//! Pinned enumeration semantics: every Clause 6 expansion example, the
//! §9.2.2 mask denotations, §4.4.3 sweeps, the D24–D29 decisions, and the
//! D27 parse-time range-endpoint rules (docs/spec-notes.md).

use edtf_core::{Edtf, Unenumerable};

/// Enumerate `input` and render each value canonically.
fn values_of(input: &str) -> Vec<String> {
    Edtf::parse(input)
        .unwrap()
        .values()
        .unwrap()
        .map(|v| v.to_string())
        .collect()
}

fn unenumerable(input: &str) -> Unenumerable {
    Edtf::parse(input).unwrap().values().unwrap_err()
}

// ------------------------------------------------------------- singletons

#[test]
fn concrete_expressions_yield_themselves_verbatim() {
    for s in [
        "1985",
        "1985-04",
        "1985-04-12",
        "1985-04-12?",
        "2004-06~-11",
        "-1985-04-12",
        "2001-21",
        "2001-34%",
        "Y170000002",
        "Y17E7",
        "1985-04-12T23:20:30Z",
    ] {
        assert_eq!(values_of(s), [s], "{s} must be a singleton of itself");
    }
}

// ------------------------------------------------------------ set expansion

#[test]
fn clause_6_4_example_1_expands_exactly_as_the_spec_does() {
    assert_eq!(
        values_of("{1667,1668,1670..1672}"),
        ["1667", "1668", "1670", "1671", "1672"]
    );
}

#[test]
fn day_precision_range_expands_inclusively() {
    // §6.3 Example 8: 1984-10-10 through 1984-11-01, 23 days.
    let days = values_of("{1984-10-10..1984-11-01}");
    assert_eq!(days.len(), 23);
    assert_eq!(days.first().unwrap(), "1984-10-10");
    assert_eq!(days[21], "1984-10-31");
    assert_eq!(days.last().unwrap(), "1984-11-01");
}

#[test]
fn month_precision_range_crosses_year_boundary() {
    assert_eq!(
        values_of("{1760-11..1761-02}"),
        ["1760-11", "1760-12", "1761-01", "1761-02"]
    );
}

#[test]
fn day_precision_range_honours_leap_february() {
    assert_eq!(
        values_of("{2000-02-28..2000-03-01}"),
        ["2000-02-28", "2000-02-29", "2000-03-01"]
    );
    assert_eq!(
        values_of("{2001-02-28..2001-03-01}"),
        ["2001-02-28", "2001-03-01"]
    );
}

#[test]
fn degenerate_range_yields_one_value() {
    assert_eq!(values_of("{1670..1670}"), ["1670"]);
}

#[test]
fn one_member_sets_enumerate_their_candidates() {
    // §6.2: [] means one member applies; the members are the candidates.
    assert_eq!(values_of("[1667,1760-12]"), ["1667", "1760-12"]);
    assert_eq!(values_of("[1670..1672]"), ["1670", "1671", "1672"]);
}

#[test]
fn elements_enumerate_in_written_order_without_cross_element_dedup() {
    // 6.1: sets carry "no specified order" — written order is preserved,
    // and a value written twice is yielded twice (D29).
    assert_eq!(values_of("{1985,1960}"), ["1985", "1960"]);
    assert_eq!(
        values_of("{1960,1959..1961}"),
        ["1960", "1959", "1960", "1961"]
    );
}

#[test]
fn negative_year_range_crosses_year_zero() {
    // Astronomical numbering: -0001, 0000, 0001 are consecutive (D2).
    assert_eq!(
        values_of("{-0002..0001}"),
        ["-0002", "-0001", "0000", "0001"]
    );
}

#[test]
fn y_prefixed_year_range_expands() {
    assert_eq!(
        values_of("{Y10000..Y10002}"),
        ["Y10000", "Y10001", "Y10002"]
    );
}

#[test]
fn range_walk_crosses_the_9999_boundary_between_year_forms() {
    assert_eq!(values_of("{9999..Y10001}"), ["9999", "Y10000", "Y10001"]);
}

// ------------------------------------------------------- mask completions

#[test]
fn masked_year_enumerates_its_decade() {
    let decade: Vec<String> = (1560..=1569).map(|y| y.to_string()).collect();
    assert_eq!(values_of("156X"), decade);
}

#[test]
fn spec_9_2_2_example_6_february_or_december() {
    assert_eq!(values_of("1560-X2"), ["1560-02", "1560-12"]);
}

#[test]
fn masked_month_with_day_31_keeps_only_valid_completions() {
    // D11/D26: only the 31-day months among 01–09 admit day 31.
    assert_eq!(
        values_of("1985-0X-31"),
        [
            "1985-01-31",
            "1985-03-31",
            "1985-05-31",
            "1985-07-31",
            "1985-08-31"
        ]
    );
}

#[test]
fn masked_year_with_leap_day_keeps_only_leap_completions() {
    assert_eq!(
        values_of("196X-02-29"),
        ["1960-02-29", "1964-02-29", "1968-02-29"]
    );
}

#[test]
fn masked_year_composes_with_season_codes() {
    // D22: seasons are ordinary year-month expressions; 200X-21 denotes
    // ten springs.
    let springs = values_of("200X-21");
    assert_eq!(springs.len(), 10);
    assert_eq!(springs.first().unwrap(), "2000-21");
    assert_eq!(springs.last().unwrap(), "2009-21");
}

#[test]
fn fully_masked_date_streams_every_proleptic_gregorian_day() {
    // ~3.65M values: must stream lazily, never allocate the list.
    let n = Edtf::parse("XXXX-XX-XX").unwrap().values().unwrap().count();
    // 10000 years × 365 days + 2425 leap days (2500 − 100 + 25).
    assert_eq!(n, 3_652_425);
}

#[test]
fn qualifiers_copy_through_to_every_completion() {
    // D28: qualification never moves the value set (§8.4.2 NOTE); it is
    // copied onto each yielded value, component by component.
    assert_eq!(values_of("156X~").first().unwrap(), "1560~");
    assert_eq!(values_of("156X~").last().unwrap(), "1569~");
    assert_eq!(values_of("1985-XX?").len(), 12);
    assert_eq!(values_of("1985-XX?").first().unwrap(), "1985-01?");
    // Canonical form renders year-only qualification as a group marker
    // (§8.2.4): ?1985-XX completions spell as 1985?-MM.
    assert_eq!(values_of("?1985-XX"), {
        let expected: Vec<String> = (1..=12).map(|m| format!("1985?-{m:02}")).collect();
        expected
    });
}

// ------------------------------------------------------ significant digits

#[test]
fn significant_digit_year_sweeps_its_denoted_range() {
    // §4.4.3: 1950S2 denotes some year 1900–1999.
    let years = values_of("1950S2");
    assert_eq!(years.len(), 100);
    assert_eq!(years.first().unwrap(), "1900");
    assert_eq!(years.last().unwrap(), "1999");
}

#[test]
fn full_precision_significant_year_is_a_single_value() {
    assert_eq!(values_of("1950S4"), ["1950"]);
}

#[test]
fn big_year_significant_sweep_follows_d19() {
    // D19: Y171010000S3 denotes 171000000–171999999 (§4.4.3, not the
    // Annex A.6.3 Example 2 erratum).
    let mut vals = Edtf::parse("Y171010000S3").unwrap().values().unwrap();
    assert_eq!(vals.next().unwrap().to_string(), "Y171000000");
    assert_eq!(vals.last().unwrap().to_string(), "Y171999999");
}

// ------------------------------------------------------------ unenumerable

#[test]
fn intervals_are_not_enumerable() {
    // D24: Clause 10 has no enumeration language — an interval denotes one
    // continuous extent.
    for s in [
        "2004/2005",
        "1985-04-12/..",
        "/1985",
        "2004-06-01/2004-06-20",
    ] {
        assert_eq!(unenumerable(s), Unenumerable::Interval, "{s}");
    }
}

#[test]
fn unbounded_set_elements_are_not_enumerable() {
    // D25: §6.3 a–b are "indication", never "expansion".
    for s in [
        "{..1983-12-31}",
        "[1760-12..]",
        "{1667,1668,1984-11-05..}",
        "{..1983-12-31,1984-10-10..1984-11-01,1984-11-05..}",
    ] {
        assert_eq!(unenumerable(s), Unenumerable::UnboundedSetElement, "{s}");
    }
}

#[test]
fn overflowing_sweeps_report_year_range_overflow() {
    // The same expressions where bounds() reports Unknown.
    assert_eq!(unenumerable("Y99E17S1"), Unenumerable::YearRangeOverflow);
}

// --------------------------------------------------- D27 parse-time rules

#[test]
fn d27_rejects_degenerate_range_endpoints_at_parse_time() {
    for s in [
        // mixed precision (§6.3 c: sides "should be of the same precision")
        "{1984..1985-04}",
        "{2004-02-01..2005}",
        // masked endpoints (a range between value sets has no §6.3 meaning)
        "{156X..1600}",
        "{1984-10-1X..1984-11-01}",
        // season endpoints (no spec-defined successor)
        "{2001-21..2001-24}",
        // qualified endpoints (no defined spread over expanded members)
        "{1670?..1672}",
        "{1670..~1672}",
        "{1984-10-10..1984-11-01%}",
        // significant-digit endpoints (each denotes a set, not a value)
        "{1950S2..1960}",
    ] {
        assert!(Edtf::parse(s).is_err(), "{s} must be rejected (D27)");
    }
}

#[test]
fn d27_keeps_every_spec_exemplified_range_valid() {
    for s in [
        "{1667,1668,1670..1672}",
        "[1760-01,1760-02,1760-12..]",
        "{..1983-12-31,1984-10-10..1984-11-01,1984-11-05..}",
        "{-0100..0100}",
        "{Y10000..Y10002}",
    ] {
        assert!(Edtf::parse(s).is_ok(), "{s} must stay valid");
    }
}
