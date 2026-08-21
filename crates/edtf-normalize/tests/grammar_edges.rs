// SPDX-FileCopyrightText: Copyright (c) the edtf contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Edge-of-grammar inputs: each test pins a refusal or acceptance path that
//! the trap list does not reach — attached modifiers, malformed numerics,
//! range endpoints that must fail closed, and preprocessing quirks. Same
//! N-decision authority as `traps.rs`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test/bench code: a panic here is the failure signal, not a crash path"
)]
#![expect(
    clippy::non_ascii_literal,
    reason = "Russian prose dates are the input this normaliser exists to read; Cyrillic literals are the subject under test, not stray non-ASCII"
)]
#![expect(
    clippy::tests_outside_test_module,
    reason = "an integration test under tests/ is compiled as its own crate whose every item is test support, so there is no non-test code for a mod tests to separate it from"
)]
#![expect(
    clippy::min_ident_chars,
    reason = "the test bodies use the same y/m/d date-component names as the code they exercise"
)]

use edtf_core::Edtf;
use edtf_normalize::{Language, Options, Outcome, normalize, normalize_with};

fn ru() -> Options {
    Options {
        language: Language::Russian,
        ..Options::default()
    }
}

/// Assert a Normalized outcome with the given canonical EDTF.
#[track_caller]
fn ok_with(input: &str, opts: Options, expected: &str) {
    match normalize_with(input, opts) {
        Outcome::Normalized(n) => {
            assert_eq!(n.edtf, expected, "input: {input:?}");
            assert_eq!(Edtf::parse(&n.edtf).expect("output must parse"), n.value);
        }
        other => panic!("expected Normalized for {input:?}, got {other:?}"),
    }
}

#[track_caller]
fn ok(input: &str, expected: &str) {
    ok_with(input, Options::default(), expected);
}

/// Assert an Ambiguous outcome offering exactly these EDTF readings, in order.
#[track_caller]
fn ambiguous(input: &str, expected: &[&str]) {
    match normalize(input) {
        Outcome::Ambiguous(a) => {
            let got: Vec<&str> = a.interpretations.iter().map(|i| i.edtf.as_str()).collect();
            assert_eq!(got, expected, "input: {input:?}");
        }
        other => panic!("expected Ambiguous for {input:?}, got {other:?}"),
    }
}

#[track_caller]
fn no_match_with(input: &str, opts: Options) {
    assert!(
        matches!(normalize_with(input, opts), Outcome::NoMatch { .. }),
        "expected NoMatch for {input:?}"
    );
}

#[track_caller]
fn no_match(input: &str) {
    no_match_with(input, Options::default());
}

// ---------------------------------------------------------------------------
// Years and eras

#[test]
fn bc_years_beyond_the_edtf_year_range_fail_closed() {
    // 20000 BC would be astronomical -19999, outside the four-digit model.
    no_match("20000 BC");
}

#[test]
fn era_qualified_non_years_fail_closed() {
    no_match("19th BC"); // era after something that is not a plain year
}

// ---------------------------------------------------------------------------
// Roman numerals (Cyrillic lookalikes included, N15)

#[test]
fn cyrillic_lookalike_roman_numerals() {
    // A real Russian keyboard produces Cyrillic Х/І in "ХІХ век" (N15).
    ok_with("Х\u{456}Х век", ru(), "18XX");
    // The full lookalike alphabet maps (л=L, с=C, д=D, м=M) but values
    // above 99 are not centuries — refuse, never guess.
    no_match_with("см век", ru());
    no_match_with("дм век", ru());
    // L (50) alone is a syntactically fine century ordinal.
    ok_with("хл век", ru(), "39XX");
}

// ---------------------------------------------------------------------------
// Decades and modifiers

#[test]
fn right_single_quote_marks_a_bare_decade() {
    // U+2019 is what word processors type for the apostrophe in "’80s".
    ambiguous("\u{2019}80s", &["188X", "198X"]);
}

#[test]
fn attached_approximate_prefix_requires_digits() {
    // "c" reads as circa only before a digit; "cat" is not "c. at".
    no_match("cat");
}

#[test]
fn decade_suffix_needs_digits_and_a_whole_decade() {
    no_match("abcs"); // suffix without digits
    no_match("180s"); // three-digit "decade" is neither decade nor century
}

#[test]
fn attached_modifiers() {
    // "mid-1930s": attached single-word modifier, dropped over a decade (N1).
    ok("mid-1930s", "193X");
    // A dangling hyphen after the modifier is not an attached form.
    no_match("mid- 1930s");
}

#[test]
fn modifier_over_an_ambiguous_decade_keeps_both_readings() {
    // "early 1900s": the modifier is dropped (N1) but the decade/century
    // ambiguity (N6) survives.
    ambiguous("early 1900s", &["190X", "19XX"]);
}

// ---------------------------------------------------------------------------
// Numeric tokens

#[test]
fn mixed_numeric_separators_fail_closed() {
    no_match("12/04-1985");
}

#[test]
fn impossible_on_both_numeric_readings_fails_closed() {
    // 13.13.1985: neither day-first nor month-first can place a 13th month.
    no_match("13.13.1985");
}

#[test]
fn year_then_single_digit_month() {
    ok("1985/4", "1985-04");
}

#[test]
fn hyphenated_year_pair_rejects_non_ranges() {
    no_match("1914-13"); // not a month, not a later elided year
    no_match("1914-100"); // three digits: neither a month nor an elided year
}

#[test]
fn season_range_collisions_cover_every_season_name() {
    // N13: 21-24 collide with the four season codes.
    ambiguous("1915-21", &["1915-21", "1915/1921"]);
    ambiguous("1915-22", &["1915-22", "1915/1922"]);
    ambiguous("1915-23", &["1915-23", "1915/1923"]);
    ambiguous("1915-24", &["1915-24", "1915/1924"]);
}

// ---------------------------------------------------------------------------
// Open-ended ranges, alternatives

#[test]
fn open_range_over_a_collision_keeps_only_date_readings() {
    // "before 1914-21": the elided-range reading cannot be an interval
    // endpoint, so the sub-year reading survives alone (N8, N13).
    ok("before 1914-21", "../1914-21");
}

#[test]
fn or_needs_an_expression_on_both_sides() {
    no_match("1863 or");
    no_match("or 1863");
}

#[test]
fn or_refuses_nested_ambiguity() {
    // "1900s" is itself two readings: a 4-way product would be a guess (N14).
    no_match("1863 or 1900s");
}

// ---------------------------------------------------------------------------
// Two-sided ranges

#[test]
fn dangling_hyphen_is_not_a_range() {
    no_match("1914 -");
}

#[test]
fn bare_ordinal_left_endpoint_requires_a_century_right() {
    // The right side must be a century mask for "17-..." to inherit
    // century-ness; an ambiguous decade is not one.
    no_match("17-1900s");
}

#[test]
fn single_letter_roman_century_works_as_a_left_endpoint() {
    // A lone "х" is too weak to parse alone (N15) but inherits century-ness
    // from the right endpoint: X-XIX centuries.
    ok_with("х-х\u{456}х вв.", ru(), "09XX/18XX");
}

#[test]
fn ambiguous_left_endpoint_with_unparseable_right_fails_closed() {
    no_match("1900s-45");
}

#[test]
fn unparseable_endpoints_fail_closed() {
    no_match("abc-def");
}

#[test]
fn ambiguous_endpoint_ranges_keep_every_reading() {
    // Left ambiguous, right fixed.
    ambiguous("the 80s to 1990", &["188X/1990", "198X/1990"]);
    // Right ambiguous, left fixed.
    ambiguous("1750 to the 80s", &["1750/188X", "1750/198X"]);
}

#[test]
fn ambiguous_endpoint_with_one_reversed_reading_fails_closed() {
    // "the 80s to 1950": the 1980s reading is a reversed interval; a prose
    // error poisons the whole input rather than promoting the survivor (N14).
    assert!(matches!(
        normalize("the 80s to 1950"),
        Outcome::NoMatch { .. }
    ));
}

#[test]
fn doubly_ambiguous_ranges_fail_closed() {
    // Two ambiguous endpoints would be a 4-way product — refuse (N14).
    no_match("the 80s to the 90s");
}

// ---------------------------------------------------------------------------
// Qualifiers and empty inputs

#[test]
fn spaced_trailing_question_mark() {
    ok("1860 ?", "1860?");
}

#[test]
fn a_lone_question_mark_is_not_a_date() {
    no_match("?");
}

#[test]
fn non_decade_years_with_a_decade_suffix_fail_closed() {
    no_match("1985s"); // only whole decades take the plural suffix
}

#[test]
fn era_years_outside_the_model_fail_closed() {
    no_match("0 AD"); // no year zero in era phrasing
    no_match("10000 AD"); // five digits only ever fit BC (astronomical -9999)
}

// ---------------------------------------------------------------------------
// Token-classifier extremes

#[test]
fn oversized_and_empty_numbers_fail_closed() {
    no_match("600000 BC"); // six digits never form a year token
    no_match("s"); // a decade suffix with no digits at all
}

#[test]
fn numeric_field_widths_are_enforced() {
    no_match("32/04/1985"); // 32 can only be a day, and no month fits
    no_match("123/4/1985"); // three-digit first field
    no_match("12/345/1985"); // three-digit second field
    no_match("004.1985"); // three-digit month
    no_match("13.2008"); // 13 is not a month
    no_match("1914.1918"); // year pairs only elide across '-'
}

#[test]
fn top_level_collision_requires_digits_on_both_sides() {
    no_match("1985-2a");
}

#[test]
fn sub_year_codes_that_cannot_elide_pass_through_as_edtf() {
    // 1985-21 reads only as spring 1985: 1921 < 1985 is no range (N13).
    ok("1985-21", "1985-21");
}

// ---------------------------------------------------------------------------
// Range word-splits and endpoint shapes

#[test]
fn range_words_need_both_sides() {
    no_match("to 1900");
    no_match("1900 to");
    no_match("- 1914"); // leading hyphen leaves an empty left endpoint
}

#[test]
fn spaced_elision_only_collides_on_bare_year_left_endpoints() {
    // A month (or day) on the left kills the sub-year reading: only the
    // elided range survives (N13 scope).
    ok("may 1915 - 21", "1915-05/1921");
    ok("12 may 1915 - 21", "1915-05-12/1921");
}

#[test]
fn range_endpoints_refuse_interval_readings() {
    // "1915-21" as an endpoint offers a sub-year date AND a year range; the
    // range reading cannot nest inside another range, so the pair fails
    // closed rather than promoting the survivor (N14).
    no_match("1900 to 1915-21");
    no_match("1915-21 to 1990");
}

#[test]
fn winter_needs_a_bare_successor_year_for_the_cross_year_reading() {
    // A month or day on the right endpoint forces the plain-range reading:
    // no one-winter interpretation exists (N17 scope).
    ok("winter 1941 - may 1942", "1941-24/1942-05");
    ok("winter 1941 - 12 may 1942", "1941-24/1942-05-12");
}

// ---------------------------------------------------------------------------
// Decade-of-century phrasing

#[test]
fn decade_of_century_in_both_registers() {
    // Noise between decade and century, Roman century (N6/N15)…
    ok_with("60-е годы XIX века", ru(), "186X");
    // …and the Arabic-ordinal variant.
    ok_with("60-е 19 века", ru(), "186X");
}

#[test]
fn bare_decade_of_bare_roman_century() {
    // No noise words at all between decade and century.
    ok_with("60-е XIX", ru(), "186X");
}

#[test]
fn collision_check_needs_digits_in_the_year_too() {
    no_match("198a-21");
}
