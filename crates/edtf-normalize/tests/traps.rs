//! The acceptance trap list from issue #20, plus the grammar edges each
//! N-decision in docs/normalize-notes.md commits to. Every expected string
//! here must parse in edtf-core at the asserted level — enforced inline.

use edtf_core::Edtf;
use edtf_normalize::{Note, NumericOrder, Options, Outcome, normalize, normalize_with};

/// Assert a Normalized outcome with the given canonical EDTF and level.
#[track_caller]
fn ok(input: &str, expected: &str, level: u8) {
    match normalize(input) {
        Outcome::Normalized(n) => {
            assert_eq!(n.edtf, expected, "input: {input:?}");
            let parsed = Edtf::parse(&n.edtf).expect("output must parse in core");
            assert_eq!(parsed, n.value, "value/edtf mismatch for {input:?}");
            assert_eq!(parsed.level(), level, "level mismatch for {input:?}");
        },
        other => panic!("expected Normalized for {input:?}, got {other:?}"),
    }
}

/// Assert an Ambiguous outcome offering exactly these EDTF readings, in order.
#[track_caller]
fn ambiguous(input: &str, expected: &[&str]) {
    match normalize(input) {
        Outcome::Ambiguous(a) => {
            let got: Vec<&str> = a.interpretations.iter().map(|i| i.edtf.as_str()).collect();
            assert_eq!(got, expected, "input: {input:?}");
            for i in &a.interpretations {
                assert_eq!(
                    Edtf::parse(&i.edtf).expect("interpretation must parse"),
                    i.value
                );
            }
        },
        other => panic!("expected Ambiguous for {input:?}, got {other:?}"),
    }
}

#[track_caller]
fn no_match(input: &str) {
    assert!(
        matches!(normalize(input), Outcome::NoMatch { .. }),
        "expected NoMatch for {input:?}"
    );
}

// ---------------------------------------------------------------------------
// The issue #20 trap list, verbatim

#[test]
fn bc_years_are_astronomical() {
    // 500 BC is astronomical -0499: year 0 exists (N3).
    ok("500 BC", "-0499", 1);
    ok("500 BCE", "-0499", 1);
    // 64 BCE is astronomical -0063 — python-edtf emits -0064, applying the
    // year-0 offset to centuries but not years (documented divergence, N3).
    ok("c64 BCE", "-0063~", 1);
    ok("1 AD", "0001", 0);
    ok("79 CE", "0079", 0);
}

#[test]
fn centuries_are_off_by_one() {
    ok("19th century", "18XX", 1);
    ok("1st century", "00XX", 1);
    ok("nineteenth century", "18XX", 1);
    ok("10c", "09XX", 1);
    ok("2nd century AD", "01XX", 1);
}

#[test]
fn bc_centuries_are_exact_intervals() {
    // Negative years cannot carry X digits (core rejects them), so BC
    // centuries get exact intervals, not python-edtf's invalid "-01XX" (N2).
    ok("2nd century BC", "-0199/-0100", 1);
    ok("2nd century BCE", "-0199/-0100", 1);
}

#[test]
fn century_parts_are_decade_rounded_intervals() {
    // N1: thirds (early/mid/late) and halves, rounded to decades.
    ok("early 19th century", "1801/1830", 0);
    ok("mid 19th century", "1831/1870", 0);
    ok("late 19th century", "1871/1900", 0);
    ok("first half of the 19th century", "1801/1850", 0);
    ok("second half of the 19th century", "1851/1900", 0);
    // BC centuries take their parts chronologically.
    ok("late 2nd century BC", "-0129/-0100", 1);
    ok("early 2nd century BC", "-0199/-0170", 1);
}

#[test]
fn first_century_bc_has_no_mask() {
    // -00XX would need year -0000; the span is -0099..0000 (N2).
    ok("1st century BC", "-0099/0000", 1);
}

#[test]
fn decades() {
    ok("1980s", "198X", 1);
    ok("1980's", "198X", 1);
}

#[test]
fn bare_decades_are_ambiguous() {
    ambiguous("the 80s", &["188X", "198X"]);
    ambiguous("'80s", &["188X", "198X"]);
    let opts = Options {
        default_century: Some(1900),
        ..Options::default()
    };
    match normalize_with("the 80s", &opts) {
        Outcome::Normalized(n) => {
            assert_eq!(n.edtf, "198X");
            assert!(n.notes.contains(&Note::DefaultCenturyApplied));
        },
        other => panic!("expected Normalized, got {other:?}"),
    }
}

#[test]
fn century_boundary_decades_are_ambiguous() {
    // "1900s" is the 1900–1909 decade or the whole century (N6).
    ambiguous("1900s", &["190X", "19XX"]);
    ambiguous("1800s", &["180X", "18XX"]);
}

#[test]
fn qualifiers() {
    ok("circa 1920", "1920~", 1);
    ok("ca. 1920", "1920~", 1);
    ok("c.1920", "1920~", 1);
    ok("c1920", "1920~", 1);
    ok("about 1920", "1920~", 1);
    ok("possibly 1920", "1920?", 1);
    ok("probably 1920", "1920?", 1);
    ok("circa 1840s", "184X~", 1);
    ok("possibly circa 1920", "1920%", 1);
    ok("19th century?", "18XX?", 1);
    ok("ca. 19c", "18XX~", 1);
}

#[test]
fn seasons() {
    ok("spring 2001", "2001-21", 1);
    ok("Summer 1872", "1872-22", 1);
    ok("Autumn 1872", "1872-23", 1);
    ok("Fall 1872", "1872-23", 1);
    ok("Winter 1872", "1872-24", 1);
    ok("about Spring 1849", "1849-21~", 1);
}

#[test]
fn ranges() {
    ok("1914-1918", "1914/1918", 0);
    ok("1914\u{2013}18", "1914/1918", 0); // en-dash, elided century (N4)
    ok("from 1914 to 1918", "1914/1918", 0);
    ok("between 1914 and 1918", "1914/1918", 0);
    ok("1851-52", "1851/1852", 0);
    ok("1852 - 1860", "1852/1860", 0);
    ok("1860s-1870s", "186X/187X", 2);
    ok("1856-ca. 1865", "1856/1865~", 1);
    ok("1857-mid 1860s", "1857/186X", 2); // modifier dropped, recorded (N1)
    ok("17-19th centuries", "16XX/18XX", 2);
}

#[test]
fn reversed_ranges_are_rejected() {
    no_match("1918-1914");
}

#[test]
fn before_after_are_open_intervals() {
    // N8: current-spec `..`, not the old-draft bare slash.
    ok("before 1917", "../1917", 1);
    ok("after 1917", "1917/..", 1);
    ok("until 1917", "../1917", 1);
    ok("since 1917", "1917/..", 1);
    ok("earlier than 1917", "../1917", 1);
    ok("later than 1917", "1917/..", 1);
    ok("before January 1928", "../1928-01", 1);
    ok("before approx January 18 1928", "../1928-01-18~", 1);
    ok("after about the 1920s", "192X~/..", 2);
    ok("before 19th century", "../18XX", 2);
}

#[test]
fn month_and_day_forms() {
    ok("June 1940", "1940-06", 0);
    ok("Jun 1940", "1940-06", 0);
    ok("Jun. 1940", "1940-06", 0);
    ok("12 April 1985", "1985-04-12", 0);
    ok("April 12, 1985", "1985-04-12", 0);
    ok("12th April 1985", "1985-04-12", 0);
    ok("the year 1800", "1800", 0);
}

#[test]
fn missing_year_is_masked() {
    // N9: no year → XXXX, never an assumed current year.
    ok("January", "XXXX-01", 2);
    ok("January 12", "XXXX-01-12", 2);
}

#[test]
fn numeric_dates_report_ambiguity() {
    // N5: both fields ≤ 12 and no context → both readings.
    ambiguous("12/04/1985", &["1985-04-12", "1985-12-04"]);
    let day_first = Options {
        numeric_order: Some(NumericOrder::DayFirst),
        ..Options::default()
    };
    match normalize_with("12/04/1985", &day_first) {
        Outcome::Normalized(n) => {
            assert_eq!(n.edtf, "1985-04-12");
            assert!(n.notes.contains(&Note::NumericResolvedByOption));
        },
        other => panic!("expected Normalized, got {other:?}"),
    }
    // A field over 12 proves the order (N5).
    ok("13/04/1985", "1985-04-13", 0);
    ok("04/13/1985", "1985-04-13", 0);
    ok("1985/04/12", "1985-04-12", 0);
    // Equal fields: order cannot matter.
    ok("04/04/1985", "1985-04-04", 0);
    // Month-year numerics.
    ok("7/2008", "2008-07", 0);
    // Two-digit years are not adopted: unknowable century (N5).
    no_match("12/04/85");
}

#[test]
fn impossible_numeric_dates_fail_closed() {
    // 31/02/1985: invalid as DMY (Feb 31) and as MDY (month 31).
    no_match("31/02/1985");
    // 30/02/1985: DMY is Feb 30 (invalid), MDY is month 30 (invalid).
    no_match("30/02/1985");
    // 02/29/1985: 1985 is not a leap year — the core re-parse catches it.
    no_match("02/29/1985");
    ok("02/29/1984", "1984-02-29", 0);
}

#[test]
fn season_code_range_collision() {
    // "1914-21" is valid EDTF (spring 1914) AND a plausible elided range (N13);
    // every sub-year code 21..=41 collides the same way.
    ambiguous("1914-21", &["1914-21", "1914/1921"]);
    ambiguous("1910-30", &["1910-30", "1910/1930"]);
    // Outside the code range there is only the elided-range reading.
    ok("1914-18", "1914/1918", 0);
    ok("1861-67", "1861/1867", 0);
    ok("1869-70", "1869/1870", 0);
}

#[test]
fn or_alternatives() {
    // N14: python-edtf silently keeps the first; we report both.
    ambiguous("1863 or 1864", &["1863", "1864"]);
}

#[test]
fn unknown_is_no_match() {
    // N12: the form decides what "unknown" means.
    no_match("unknown");
    no_match("undated");
    no_match("n.d.");
    no_match("");
    no_match("   ");
}

#[test]
fn no_substring_extraction() {
    // N11: whole-input only; extraction is the caller's job.
    no_match("this isn't a date");
    no_match("23rd Dynasty");
    no_match("1851-1852; printed 1853-1854");
    no_match("1863, printed 1870");
    no_match("notcirca 1860");
    no_match("attica 1802");
    no_match("birthday in 1872");
    no_match("90");
}

#[test]
fn valid_edtf_passes_through() {
    ok("1985-04-12", "1985-04-12", 0);
    ok("198X", "198X", 1);
    ok("2004-06~-11", "2004-06~-11", 2);
    ok("1858/1860", "1858/1860", 0); // python-edtf rewrites this to a set; we don't
    match normalize("1860?") {
        Outcome::Normalized(n) => assert!(n.notes.contains(&Note::AlreadyValidEdtf)),
        other => panic!("expected Normalized, got {other:?}"),
    }
}

#[test]
fn whole_expression_qualifier_distributes() {
    // N10: "circa 1914-1918" qualifies both endpoints.
    match normalize("circa 1914-1918") {
        Outcome::Normalized(n) => {
            assert_eq!(n.edtf, "1914~/1918~");
            assert!(n.notes.contains(&Note::QualifierDistributed));
        },
        other => panic!("expected Normalized, got {other:?}"),
    }
    ok("1868-1871?", "1868?/1871?", 1);
}

#[test]
fn negated_open_phrases() {
    // N8: the English forms the doc quotes must actually work.
    ok("no later than 1917", "../1917", 1);
    ok("no earlier than 1917", "1917/..", 1);
}

#[test]
fn range_endpoints_inherit_the_stated_year() {
    // N16: the year is elided, not missing — never XXXX inside these.
    ok("June-July 1940", "1940-06/1940-07", 0);
    ok("June to July 1940", "1940-06/1940-07", 0);
    ok("between May and June 1940", "1940-05/1940-06", 0);
    ok("12 January to 15 March 1940", "1940-01-12/1940-03-15", 0);
    ok("from January 1940 to June", "1940-01/1940-06", 0);
    // Reversed month ranges are prose errors and fail closed.
    no_match("July-June 1940");
}

#[test]
fn cross_year_winter_is_ambiguous() {
    // N17: one boundary-spanning winter, or winter through the whole end
    // year? Both readings, honestly.
    ambiguous("winter 1941-42", &["1941-24", "1941-24/1942"]);
    ambiguous("winter 1941-1942", &["1941-24", "1941-24/1942"]);
    // A non-adjacent end year is a plain range.
    ok("winter 1941-45", "1941-24/1945", 1);
}

#[test]
fn bc_starts_refuse_elided_end_years() {
    // N4: an elided end after a BC start has no deterministic reading —
    // truncating arithmetic would fabricate one.
    no_match("500 BC to 18");
    no_match("100 BC-18");
    no_match("1900 BC to 5");
    ok("500 BC to 418 BC", "-0499/-0417", 1);
}

#[test]
fn spaced_hyphens_mirror_the_unspaced_limits() {
    // N4/N13: spacing must not smuggle readings past the NNNN-NN rules.
    no_match("2000 - 05"); // 01-12 read as months, never ranges
    ambiguous("1914 - 21", &["1914-21", "1914/1921"]);
    ok("1914 - 18", "1914/1918", 0);
}

#[test]
fn dead_readings_poison_the_whole_input() {
    // N14: an impossible alternative is a prose error — never collapse to
    // the survivor.
    no_match("31 June 1940 or 1 July 1940");
    no_match("29 February 1900 or 1901");
    // Reversed decade ranges must not resolve to the century reading.
    no_match("1910s to 1900s");
    no_match("1990s-1980s");
    // Two valid alternatives still report both.
    ambiguous("1 July 1940 or 2 July 1940", &["1940-07-01", "1940-07-02"]);
}

#[test]
fn out_of_domain_default_century_is_ignored() {
    // Domain 0..=9999 (documented); larger values behave as unset.
    let opts = Options {
        default_century: Some(10000),
        ..Options::default()
    };
    assert!(matches!(
        normalize_with("the 80s", &opts),
        Outcome::Ambiguous(_)
    ));
}

#[test]
fn no_match_reasons_discriminate() {
    // N11/N12/N14: bulk-import triage routes on the reason.
    use edtf_normalize::NoMatchReason;
    assert!(matches!(
        normalize("unknown"),
        Outcome::NoMatch {
            reason: NoMatchReason::ExplicitNoDate
        }
    ));
    assert!(matches!(
        normalize("birthday in 1872"),
        Outcome::NoMatch {
            reason: NoMatchReason::OutOfGrammar
        }
    ));
    assert!(matches!(
        normalize("30/02/1985"),
        Outcome::NoMatch {
            reason: NoMatchReason::ImpossibleDate
        }
    ));
}

#[test]
fn notes_cite_decisions() {
    let Outcome::Normalized(n) = normalize("500 BC") else {
        panic!("expected Normalized");
    };
    assert!(
        n.notes
            .iter()
            .any(|note| note.decision() == Some("N3") && !note.message().is_empty())
    );
}
