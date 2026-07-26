//! Per-language pattern tables. The grammar in `engine.rs` is
//! language-neutral; everything language-specific lives in a [`Lang`] value.
//! Adding a locale means adding one `Lang` const (and its word tables), not
//! touching the grammar.
//!
//! Every table is `&'static` so the crate stays `no_std` + zero-dep; lookup
//! is linear scan, which beats hash machinery at these sizes. All entries are
//! lowercase (input is lowercased in preprocessing) and dotless where the
//! comparison strips dots (era phrases, trailing noise).

use crate::NumericOrder;

/// Which part of a century a modifier selects (N1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Span {
    /// First three decades ("early", "начало").
    Early,
    /// Middle four decades ("mid", "середина").
    Mid,
    /// Last three decades ("late", "конец").
    Late,
    /// First five decades ("first half", "первая половина").
    FirstHalf,
    /// Last five decades ("second half", "вторая половина").
    SecondHalf,
}

/// One language's complete table set.
pub(crate) struct Lang {
    /// Month names and abbreviations (all grammatical forms), → month 1–12.
    pub months: &'static [(&'static str, u8)],
    /// Season names (all grammatical forms) → sub-year codes 21–24 (N7).
    pub seasons: &'static [(&'static str, u8)],
    /// Ordinal words usable in century phrases ("nineteenth").
    pub ordinal_words: &'static [(&'static str, u32)],
    /// Ordinal suffixes strippable from digits ("19th", "19-й").
    pub ordinal_suffixes: &'static [&'static str],
    /// Leading tokens meaning approximate (`~`).
    pub approx_leading: &'static [&'static str],
    /// Attached approximate prefixes ("c1860", "ок.1920"), longest first.
    pub approx_attached: &'static [&'static str],
    /// Leading tokens meaning uncertain (`?`).
    pub uncertain_leading: &'static [&'static str],
    /// Trailing tokens meaning approximate ("1860 approx").
    pub approx_trailing: &'static [&'static str],
    /// Trailing tokens meaning uncertain ("1862 maybe").
    pub uncertain_trailing: &'static [&'static str],
    /// Leading noise dropped before matching ("the", "в").
    pub noise_leading: &'static [&'static str],
    /// Trailing noise dropped before matching, dotless ("гг" for "1918 гг.").
    pub noise_trailing: &'static [&'static str],
    /// Whole inputs that deliberately return `NoMatch` (N12).
    pub explicit_no_date: &'static [&'static str],
    /// Era phrases as dotless token sequences, longest first; `true` = BC.
    pub eras: &'static [(&'static [&'static str], bool)],
    /// Century keywords following an ordinal ("century", "век"), dotless.
    pub century_words: &'static [&'static str],
    /// Decade suffixes strippable from digits ("1860s", "1980-е").
    pub decade_suffixes: &'static [&'static str],
    /// Part-of-century modifier phrases ("early", "первая половина").
    pub modifiers: &'static [(&'static [&'static str], Span)],
    /// Phrases meaning "before X", longest first (N8).
    pub before_phrases: &'static [&'static [&'static str]],
    /// Phrases meaning "after X", longest first (N8).
    pub after_phrases: &'static [&'static [&'static str]],
    /// Words splitting "X or Y" alternatives (N14).
    pub or_words: &'static [&'static str],
    /// Range phrase pairs: optional leading marker + separator word
    /// ("from … to …", "с … по …").
    pub range_pairs: &'static [(Option<&'static str>, &'static str)],
    /// Whether centuries may be written as Roman numerals ("XIX век").
    pub roman_centuries: bool,
    /// Numeric day/month order implied by the language's convention, if any;
    /// "12.04.1985" is unambiguously day-first in Russian usage (N5).
    pub implied_numeric_order: Option<NumericOrder>,
}

/// Table set for the requested language.
pub(crate) fn lang_for(language: crate::Language) -> &'static Lang {
    match language {
        crate::Language::English => &EN,
        crate::Language::Russian => &RU,
    }
}

/// Human-readable name for a sub-year grouping code, for ambiguity readings.
pub(crate) fn season_name(code: u8) -> &'static str {
    match code {
        21 => "spring",
        22 => "summer",
        23 => "autumn",
        24 => "winter",
        _ => "sub-year grouping",
    }
}

// ---------------------------------------------------------------------------
// English

static EN: Lang = Lang {
    months: &[
        ("january", 1),
        ("jan", 1),
        ("february", 2),
        ("feb", 2),
        ("march", 3),
        ("mar", 3),
        ("april", 4),
        ("apr", 4),
        ("may", 5),
        ("june", 6),
        ("jun", 6),
        ("july", 7),
        ("jul", 7),
        ("august", 8),
        ("aug", 8),
        ("september", 9),
        ("sept", 9),
        ("sep", 9),
        ("october", 10),
        ("oct", 10),
        ("november", 11),
        ("nov", 11),
        ("december", 12),
        ("dec", 12),
    ],
    seasons: &[
        ("spring", 21),
        ("summer", 22),
        ("autumn", 23),
        ("fall", 23),
        ("winter", 24),
    ],
    ordinal_words: &[
        ("first", 1),
        ("second", 2),
        ("third", 3),
        ("fourth", 4),
        ("fifth", 5),
        ("sixth", 6),
        ("seventh", 7),
        ("eighth", 8),
        ("ninth", 9),
        ("tenth", 10),
        ("eleventh", 11),
        ("twelfth", 12),
        ("thirteenth", 13),
        ("fourteenth", 14),
        ("fifteenth", 15),
        ("sixteenth", 16),
        ("seventeenth", 17),
        ("eighteenth", 18),
        ("nineteenth", 19),
        ("twentieth", 20),
        ("twenty-first", 21),
    ],
    ordinal_suffixes: &["st", "nd", "rd", "th"],
    approx_leading: &[
        "circa",
        "ca",
        "c",
        "approx",
        "approximately",
        "about",
        "around",
        "~",
    ],
    approx_attached: &["circa", "ca.", "ca", "c.", "c", "~"],
    uncertain_leading: &["possibly", "probably", "perhaps", "maybe", "uncertain"],
    approx_trailing: &["approx", "approximately"],
    uncertain_trailing: &["maybe", "possibly", "perhaps", "uncertain", "guess"],
    noise_leading: &["the", "in", "on", "of", "year", "dated", "active"],
    noise_trailing: &[],
    explicit_no_date: &["unknown", "undated", "no date", "n.d.", "n.d"],
    eras: &[
        (&["bce"], true),
        (&["bc"], true),
        (&["ad"], false),
        (&["ce"], false),
    ],
    century_words: &["century", "centuries", "c"],
    decade_suffixes: &["'s", "s"],
    modifiers: &[
        (&["first", "half"], Span::FirstHalf),
        (&["second", "half"], Span::SecondHalf),
        (&["latter", "half"], Span::SecondHalf),
        (&["early"], Span::Early),
        (&["middle"], Span::Mid),
        (&["mid"], Span::Mid),
        (&["late"], Span::Late),
    ],
    before_phrases: &[&["earlier", "than"], &["before"], &["until"], &["by"]],
    after_phrases: &[&["later", "than"], &["after"], &["since"]],
    or_words: &["or"],
    range_pairs: &[
        (Some("from"), "to"),
        (Some("between"), "and"),
        (None, "to"),
        (None, "through"),
    ],
    roman_centuries: false,
    implied_numeric_order: None,
};

// ---------------------------------------------------------------------------
// Russian
//
// Grammatical case matters: dates appear in nominative ("январь"), genitive
// ("января 1985 года"), and instrumental ("весной 2001"). Tables carry every
// form the grammar should accept; the engine never inflects.

static RU: Lang = Lang {
    months: &[
        ("январь", 1),
        ("января", 1),
        ("янв", 1),
        ("февраль", 2),
        ("февраля", 2),
        ("фев", 2),
        ("март", 3),
        ("марта", 3),
        ("мар", 3),
        ("апрель", 4),
        ("апреля", 4),
        ("апр", 4),
        ("май", 5),
        ("мая", 5),
        ("июнь", 6),
        ("июня", 6),
        ("июн", 6),
        ("июль", 7),
        ("июля", 7),
        ("июл", 7),
        ("август", 8),
        ("августа", 8),
        ("авг", 8),
        ("сентябрь", 9),
        ("сентября", 9),
        ("сент", 9),
        ("сен", 9),
        ("октябрь", 10),
        ("октября", 10),
        ("окт", 10),
        ("ноябрь", 11),
        ("ноября", 11),
        ("нояб", 11),
        ("ноя", 11),
        ("декабрь", 12),
        ("декабря", 12),
        ("дек", 12),
    ],
    seasons: &[
        ("весна", 21),
        ("весны", 21),
        ("весной", 21),
        ("лето", 22),
        ("лета", 22),
        ("летом", 22),
        ("осень", 23),
        ("осени", 23),
        ("осенью", 23),
        ("зима", 24),
        ("зимы", 24),
        ("зимой", 24),
    ],
    ordinal_words: &[],
    ordinal_suffixes: &["-й", "-го", "-я", "-е", "-м", "-ом"],
    approx_leading: &["около", "ок", "примерно", "приблизительно", "прим", "~"],
    approx_attached: &["около", "ок.", "ок", "~"],
    uncertain_leading: &["возможно", "вероятно", "предположительно"],
    approx_trailing: &["примерно", "приблизительно"],
    uncertain_trailing: &["возможно", "вероятно", "предположительно"],
    noise_leading: &["в", "во"],
    noise_trailing: &["г", "гг", "год", "года", "году", "годы", "годов"],
    explicit_no_date: &[
        "неизвестно",
        "неизвестна",
        "без даты",
        "не датировано",
        "б.д.",
        "б.д",
    ],
    eras: &[
        (&["до", "нашей", "эры"], true),
        (&["до", "н", "э"], true),
        (&["до", "нэ"], true),
        (&["нашей", "эры"], false),
        (&["н", "э"], false),
        (&["нэ"], false),
    ],
    century_words: &["век", "века", "веке", "вв", "в", "столетие", "столетия"],
    decade_suffixes: &["-е", "-х"],
    modifiers: &[
        (&["первая", "половина"], Span::FirstHalf),
        (&["первой", "половине"], Span::FirstHalf),
        (&["первой", "половины"], Span::FirstHalf),
        (&["вторая", "половина"], Span::SecondHalf),
        (&["второй", "половине"], Span::SecondHalf),
        (&["второй", "половины"], Span::SecondHalf),
        (&["начало"], Span::Early),
        (&["начале"], Span::Early),
        (&["начала"], Span::Early),
        (&["нач"], Span::Early),
        (&["середина"], Span::Mid),
        (&["середине"], Span::Mid),
        (&["середины"], Span::Mid),
        (&["сер"], Span::Mid),
        (&["конец"], Span::Late),
        (&["конце"], Span::Late),
        (&["конца"], Span::Late),
        (&["кон"], Span::Late),
    ],
    before_phrases: &[&["не", "позднее"], &["ранее"], &["до"]],
    after_phrases: &[&["не", "ранее"], &["начиная", "с"], &["после"], &["с"]],
    or_words: &["или"],
    range_pairs: &[(Some("с"), "по"), (Some("от"), "до"), (Some("с"), "до")],
    roman_centuries: true,
    implied_numeric_order: Some(NumericOrder::DayFirst),
};
