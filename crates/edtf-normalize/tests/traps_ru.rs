//! Russian-language acceptance traps: the same N-decisions exercised through
//! the RU table set — Roman-numeral centuries, multi-token eras ("до н. э."),
//! grammatical-case month/season forms, year-marker noise ("гг."), and the
//! locale-implied day-first numeric order (N5).

use edtf_core::Edtf;
use edtf_normalize::{Language, NoMatchReason, Note, Options, Outcome, normalize_with};

fn ru() -> Options {
    Options {
        language: Language::Russian,
        ..Options::default()
    }
}

#[track_caller]
fn ok(input: &str, expected: &str, level: u8) {
    match normalize_with(input, &ru()) {
        Outcome::Normalized(n) => {
            assert_eq!(n.edtf, expected, "input: {input:?}");
            let parsed = Edtf::parse(&n.edtf).expect("output must parse in core");
            assert_eq!(parsed, n.value, "value/edtf mismatch for {input:?}");
            assert_eq!(parsed.level(), level, "level mismatch for {input:?}");
        },
        other => panic!("expected Normalized for {input:?}, got {other:?}"),
    }
}

#[track_caller]
fn ambiguous(input: &str, expected: &[&str]) {
    match normalize_with(input, &ru()) {
        Outcome::Ambiguous(a) => {
            let got: Vec<&str> = a.interpretations.iter().map(|i| i.edtf.as_str()).collect();
            assert_eq!(got, expected, "input: {input:?}");
        },
        other => panic!("expected Ambiguous for {input:?}, got {other:?}"),
    }
}

#[track_caller]
fn no_match(input: &str) {
    assert!(
        matches!(normalize_with(input, &ru()), Outcome::NoMatch { .. }),
        "expected NoMatch for {input:?}"
    );
}

#[test]
fn years_with_markers() {
    ok("1985", "1985", 0);
    ok("1985 г.", "1985", 0);
    ok("в 1985 году", "1985", 0);
    ok("1985 год", "1985", 0);
}

#[test]
fn qualifiers() {
    ok("около 1920", "1920~", 1);
    ok("около 1920 г.", "1920~", 1);
    ok("ок. 1920", "1920~", 1);
    ok("ок.1920", "1920~", 1);
    ok("примерно 1920", "1920~", 1);
    ok("возможно 1920", "1920?", 1);
    ok("возможно около 1920", "1920%", 1);
    ok("1920?", "1920?", 1); // valid EDTF passthrough
}

#[test]
fn bc_years_are_astronomical() {
    // N3: 500 до н. э. = astronomical -0499.
    ok("500 до н. э.", "-0499", 1);
    ok("500 до н.э.", "-0499", 1);
    ok("500 до нашей эры", "-0499", 1);
    ok("79 н. э.", "0079", 0);
    ok("около 64 до н. э.", "-0063~", 1);
}

#[test]
fn roman_centuries() {
    ok("XIX век", "18XX", 1);
    ok("XIX в.", "18XX", 1);
    ok("xix век", "18XX", 1);
    ok("19 век", "18XX", 1);
    ok("19-й век", "18XX", 1);
    ok("I век", "00XX", 1);
    // Cyrillic lookalike letters (Х is Cyrillic here).
    ok("ХIХ век", "18XX", 1);
    // BC centuries are exact intervals (N2).
    ok("II век до н. э.", "-0199/-0100", 1);
    // Numerals STARTING with a subtractive pair (N15 — the underflow trap).
    ok("IV век", "03XX", 1);
    ok("IX век", "08XX", 1);
    ok("XL век", "39XX", 1);
    ok("IX-X вв.", "08XX/09XX", 2);
    // Roman provenance is citable (N15).
    match normalize_with("XIX век", &ru()) {
        Outcome::Normalized(n) => {
            assert!(n.notes.contains(&Note::RomanCentury));
            assert!(n.notes.iter().any(|note| note.decision() == Some("N15")));
        },
        other => panic!("expected Normalized, got {other:?}"),
    }
}

#[test]
fn century_word_forms() {
    // Genitive plural, prepositional, and the "ст." abbreviation.
    ok("XIX-XX веков", "18XX/19XX", 2);
    ok("конец XIX - начало XX веков", "18XX/19XX", 2);
    ok("в XIX столетии", "18XX", 1);
    ok("XIX ст.", "18XX", 1);
    ok("в XIX-XX веках", "18XX/19XX", 2);
}

#[test]
fn prepositional_months() {
    // "в мае 1945 года" is the most common Russian month-year prose form.
    ok("в мае 1945 года", "1945-05", 0);
    ok("в январе 1985 году", "1985-01", 0);
    ok("в сентябре 1939 г.", "1939-09", 0);
    ok("в августе 1991", "1991-08", 0);
}

#[test]
fn year_marker_before_era() {
    // "44 г. до н. э." — the standard textbook form (N3).
    ok("44 г. до н. э.", "-0043", 1);
    ok("44 г. до н.э.", "-0043", 1);
    ok("79 г. н. э.", "0079", 0);
    ok("около 44 г. до н.э.", "-0043~", 1);
    ok("в 44 году до н. э.", "-0043", 1);
}

#[test]
fn decade_of_century() {
    // "60-е годы XIX века" — the idiom behind «шестидесятники» (N6).
    ok("60-е годы XIX века", "186X", 1);
    ok("20-е годы XX века", "192X", 1);
    ok("в 60-х годах XIX века", "186X", 1);
    ok("60-е гг. XIX в.", "186X", 1);
}

#[test]
fn century_parts() {
    // N1: начало/середина/конец are decade-rounded thirds; halves are halves.
    ok("начало XIX века", "1801/1830", 0);
    ok("в начале XIX века", "1801/1830", 0);
    ok("середина XIX века", "1831/1870", 0);
    ok("конец XIX века", "1871/1900", 0);
    ok("первая половина XVIII века", "1701/1750", 0);
    ok("вторая половина XVIII века", "1751/1800", 0);
}

#[test]
fn decades() {
    ok("1980-е", "198X", 1);
    ok("1980-е годы", "198X", 1);
    ok("1980-х годов", "198X", 1);
    ok("около 1920-х", "192X~", 1);
    ambiguous("80-е", &["188X", "198X"]);
    ambiguous("1900-е", &["190X", "19XX"]);
}

#[test]
fn months_and_days() {
    ok("январь 1985", "1985-01", 0);
    ok("января 1985", "1985-01", 0);
    ok("12 апреля 1985", "1985-04-12", 0);
    ok("12-го апреля 1985 года", "1985-04-12", 0);
    ok("апрель", "XXXX-04", 2);
}

#[test]
fn seasons() {
    ok("весна 2001", "2001-21", 1);
    ok("весной 2001", "2001-21", 1);
    ok("летом 1872", "1872-22", 1);
    ok("осенью 1916", "1916-23", 1);
    ok("зимой 1872", "1872-24", 1);
}

#[test]
fn numeric_dates_use_locale_day_first() {
    // N5: DD.MM.YYYY is the Russian convention — resolved, with the note.
    match normalize_with("12.04.1985", &ru()) {
        Outcome::Normalized(n) => {
            assert_eq!(n.edtf, "1985-04-12");
            assert!(n.notes.contains(&Note::NumericResolvedByLocale));
        },
        other => panic!("expected Normalized, got {other:?}"),
    }
    // An explicit option still overrides the locale.
    let opts = Options {
        language: Language::Russian,
        numeric_order: Some(edtf_normalize::NumericOrder::MonthFirst),
        ..Options::default()
    };
    match normalize_with("12.04.1985", &opts) {
        Outcome::Normalized(n) => assert_eq!(n.edtf, "1985-12-04"),
        other => panic!("expected Normalized, got {other:?}"),
    }
}

#[test]
fn ranges() {
    ok("1914-1918", "1914/1918", 0);
    ok("1914\u{2013}1918 гг.", "1914/1918", 0);
    ok("с 1914 по 1918", "1914/1918", 0);
    ok("с 1914 по 1918 год", "1914/1918", 0);
    ok("XVII-XIX вв.", "16XX/18XX", 2);
    // Modifiers on range endpoints collapse to the whole century (N1 scope).
    ok("конец XIX - начало XX века", "18XX/19XX", 2);
}

#[test]
fn before_after() {
    ok("до 1917", "../1917", 1);
    ok("до 1917 года", "../1917", 1);
    ok("после 1917", "1917/..", 1);
    ok("не позднее 1917", "../1917", 1);
    ok("не ранее 1917", "1917/..", 1);
    // "до" as before-marker vs "до н. э." as era: both at once.
    ok("до 500 до н. э.", "../-0499", 1);
}

#[test]
fn alternatives() {
    ambiguous("1863 или 1864", &["1863", "1864"]);
}

#[test]
fn explicit_no_date() {
    // N12: the form decides what "unknown" means — and the reason lets it.
    no_match("неизвестно");
    no_match("без даты");
    no_match("б.д.");
    no_match("не датировано");
    assert!(matches!(
        normalize_with("без даты", &ru()),
        Outcome::NoMatch {
            reason: NoMatchReason::ExplicitNoDate
        }
    ));
}

#[test]
fn ranges_inherit_years_and_winters_stay_honest() {
    // N16 / N17 through the Russian tables.
    ok("с июня по июль 1940", "1940-06/1940-07", 0);
    ambiguous("зима 1941-1942 гг.", &["1941-24", "1941-24/1942"]);
    // N4: BC starts refuse elided end years in Russian too.
    no_match("с 500 до н. э. по 18");
}

#[test]
fn no_substring_extraction() {
    no_match("это не дата");
    no_match("1863, напечатано в 1870");
}
