// SPDX-FileCopyrightText: Copyright (c) the edtf contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Per-language pattern tables. The grammar in `engine.rs` is
//! language-neutral; everything language-specific lives in a [`Lang`] value.
//! Adding a locale means adding one `Lang` const (and its word tables), not
//! touching the grammar.
//!
//! Every table is `&'static` so the crate stays `no_std` + zero-dep; lookup
//! is linear scan, which beats hash machinery at these sizes. All entries are
//! lowercase (input is lowercased in preprocessing) and dotless where the
//! comparison strips dots (era phrases, trailing noise).

#![expect(
    clippy::redundant_pub_crate,
    reason = "pub(crate) states the intended visibility even where the module tree makes it redundant today"
)]
#![expect(
    clippy::missing_docs_in_private_items,
    reason = "the module-level //! block carries this file's design; per-item docs on small private helpers named for what they do would restate it"
)]
#![expect(
    clippy::single_call_fn,
    reason = "a named helper used once is extraction for readability, which is the opposite of a defect; several are also the named steps the module docs describe"
)]

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
pub(crate) const fn season_name(code: u8) -> &'static str {
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
    before_phrases: &[
        &["no", "later", "than"],
        &["earlier", "than"],
        &["before"],
        &["until"],
        &["by"],
    ],
    after_phrases: &[
        &["no", "earlier", "than"],
        &["later", "than"],
        &["after"],
        &["since"],
    ],
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
// ("января 1985 года"), prepositional ("в мае 1945"), and instrumental
// ("весной 2001"). Tables carry every form the grammar should accept; the
// engine never inflects.

static RU: Lang = Lang {
    months: &[
        ("\u{44f}\u{43d}\u{432}\u{430}\u{440}\u{44c}", 1),
        ("\u{44f}\u{43d}\u{432}\u{430}\u{440}\u{44f}", 1),
        ("\u{44f}\u{43d}\u{432}\u{430}\u{440}\u{435}", 1),
        ("\u{44f}\u{43d}\u{432}", 1),
        ("\u{444}\u{435}\u{432}\u{440}\u{430}\u{43b}\u{44c}", 2),
        ("\u{444}\u{435}\u{432}\u{440}\u{430}\u{43b}\u{44f}", 2),
        ("\u{444}\u{435}\u{432}\u{440}\u{430}\u{43b}\u{435}", 2),
        ("\u{444}\u{435}\u{432}", 2),
        ("\u{43c}\u{430}\u{440}\u{442}", 3),
        ("\u{43c}\u{430}\u{440}\u{442}\u{430}", 3),
        ("\u{43c}\u{430}\u{440}\u{442}\u{435}", 3),
        ("\u{43c}\u{430}\u{440}", 3),
        ("\u{430}\u{43f}\u{440}\u{435}\u{43b}\u{44c}", 4),
        ("\u{430}\u{43f}\u{440}\u{435}\u{43b}\u{44f}", 4),
        ("\u{430}\u{43f}\u{440}\u{435}\u{43b}\u{435}", 4),
        ("\u{430}\u{43f}\u{440}", 4),
        ("\u{43c}\u{430}\u{439}", 5),
        ("\u{43c}\u{430}\u{44f}", 5),
        ("\u{43c}\u{430}\u{435}", 5),
        ("\u{438}\u{44e}\u{43d}\u{44c}", 6),
        ("\u{438}\u{44e}\u{43d}\u{44f}", 6),
        ("\u{438}\u{44e}\u{43d}\u{435}", 6),
        ("\u{438}\u{44e}\u{43d}", 6),
        ("\u{438}\u{44e}\u{43b}\u{44c}", 7),
        ("\u{438}\u{44e}\u{43b}\u{44f}", 7),
        ("\u{438}\u{44e}\u{43b}\u{435}", 7),
        ("\u{438}\u{44e}\u{43b}", 7),
        ("\u{430}\u{432}\u{433}\u{443}\u{441}\u{442}", 8),
        ("\u{430}\u{432}\u{433}\u{443}\u{441}\u{442}\u{430}", 8),
        ("\u{430}\u{432}\u{433}\u{443}\u{441}\u{442}\u{435}", 8),
        ("\u{430}\u{432}\u{433}", 8),
        (
            "\u{441}\u{435}\u{43d}\u{442}\u{44f}\u{431}\u{440}\u{44c}",
            9,
        ),
        (
            "\u{441}\u{435}\u{43d}\u{442}\u{44f}\u{431}\u{440}\u{44f}",
            9,
        ),
        (
            "\u{441}\u{435}\u{43d}\u{442}\u{44f}\u{431}\u{440}\u{435}",
            9,
        ),
        ("\u{441}\u{435}\u{43d}\u{442}", 9),
        ("\u{441}\u{435}\u{43d}", 9),
        ("\u{43e}\u{43a}\u{442}\u{44f}\u{431}\u{440}\u{44c}", 10),
        ("\u{43e}\u{43a}\u{442}\u{44f}\u{431}\u{440}\u{44f}", 10),
        ("\u{43e}\u{43a}\u{442}\u{44f}\u{431}\u{440}\u{435}", 10),
        ("\u{43e}\u{43a}\u{442}", 10),
        ("\u{43d}\u{43e}\u{44f}\u{431}\u{440}\u{44c}", 11),
        ("\u{43d}\u{43e}\u{44f}\u{431}\u{440}\u{44f}", 11),
        ("\u{43d}\u{43e}\u{44f}\u{431}\u{440}\u{435}", 11),
        ("\u{43d}\u{43e}\u{44f}\u{431}", 11),
        ("\u{43d}\u{43e}\u{44f}", 11),
        ("\u{434}\u{435}\u{43a}\u{430}\u{431}\u{440}\u{44c}", 12),
        ("\u{434}\u{435}\u{43a}\u{430}\u{431}\u{440}\u{44f}", 12),
        ("\u{434}\u{435}\u{43a}\u{430}\u{431}\u{440}\u{435}", 12),
        ("\u{434}\u{435}\u{43a}", 12),
    ],
    seasons: &[
        ("\u{432}\u{435}\u{441}\u{43d}\u{430}", 21),
        ("\u{432}\u{435}\u{441}\u{43d}\u{44b}", 21),
        ("\u{432}\u{435}\u{441}\u{43d}\u{43e}\u{439}", 21),
        ("\u{43b}\u{435}\u{442}\u{43e}", 22),
        ("\u{43b}\u{435}\u{442}\u{430}", 22),
        ("\u{43b}\u{435}\u{442}\u{43e}\u{43c}", 22),
        ("\u{43e}\u{441}\u{435}\u{43d}\u{44c}", 23),
        ("\u{43e}\u{441}\u{435}\u{43d}\u{438}", 23),
        ("\u{43e}\u{441}\u{435}\u{43d}\u{44c}\u{44e}", 23),
        ("\u{437}\u{438}\u{43c}\u{430}", 24),
        ("\u{437}\u{438}\u{43c}\u{44b}", 24),
        ("\u{437}\u{438}\u{43c}\u{43e}\u{439}", 24),
    ],
    ordinal_words: &[],
    ordinal_suffixes: &[
        "-\u{439}",
        "-\u{433}\u{43e}",
        "-\u{44f}",
        "-\u{435}",
        "-\u{43c}",
        "-\u{43e}\u{43c}",
    ],
    approx_leading: &[
        "\u{43e}\u{43a}\u{43e}\u{43b}\u{43e}",
        "\u{43e}\u{43a}",
        "\u{43f}\u{440}\u{438}\u{43c}\u{435}\u{440}\u{43d}\u{43e}",
        "\u{43f}\u{440}\u{438}\u{431}\u{43b}\u{438}\u{437}\u{438}\u{442}\u{435}\u{43b}\u{44c}\u{43d}\u{43e}",
        "\u{43f}\u{440}\u{438}\u{43c}",
        "~",
    ],
    approx_attached: &[
        "\u{43e}\u{43a}\u{43e}\u{43b}\u{43e}",
        "\u{43e}\u{43a}.",
        "\u{43e}\u{43a}",
        "~",
    ],
    uncertain_leading: &[
        "\u{432}\u{43e}\u{437}\u{43c}\u{43e}\u{436}\u{43d}\u{43e}",
        "\u{432}\u{435}\u{440}\u{43e}\u{44f}\u{442}\u{43d}\u{43e}",
        "\u{43f}\u{440}\u{435}\u{434}\u{43f}\u{43e}\u{43b}\u{43e}\u{436}\u{438}\u{442}\u{435}\u{43b}\u{44c}\u{43d}\u{43e}",
    ],
    approx_trailing: &[
        "\u{43f}\u{440}\u{438}\u{43c}\u{435}\u{440}\u{43d}\u{43e}",
        "\u{43f}\u{440}\u{438}\u{431}\u{43b}\u{438}\u{437}\u{438}\u{442}\u{435}\u{43b}\u{44c}\u{43d}\u{43e}",
    ],
    uncertain_trailing: &[
        "\u{432}\u{43e}\u{437}\u{43c}\u{43e}\u{436}\u{43d}\u{43e}",
        "\u{432}\u{435}\u{440}\u{43e}\u{44f}\u{442}\u{43d}\u{43e}",
        "\u{43f}\u{440}\u{435}\u{434}\u{43f}\u{43e}\u{43b}\u{43e}\u{436}\u{438}\u{442}\u{435}\u{43b}\u{44c}\u{43d}\u{43e}",
    ],
    noise_leading: &["\u{432}", "\u{432}\u{43e}"],
    noise_trailing: &[
        "\u{433}",
        "\u{433}\u{433}",
        "\u{433}\u{43e}\u{434}",
        "\u{433}\u{43e}\u{434}\u{430}",
        "\u{433}\u{43e}\u{434}\u{443}",
        "\u{433}\u{43e}\u{434}\u{44b}",
        "\u{433}\u{43e}\u{434}\u{43e}\u{432}",
        "\u{433}\u{43e}\u{434}\u{430}\u{445}",
    ],
    explicit_no_date: &[
        "\u{43d}\u{435}\u{438}\u{437}\u{432}\u{435}\u{441}\u{442}\u{43d}\u{43e}",
        "\u{43d}\u{435}\u{438}\u{437}\u{432}\u{435}\u{441}\u{442}\u{43d}\u{430}",
        "\u{431}\u{435}\u{437} \u{434}\u{430}\u{442}\u{44b}",
        "\u{43d}\u{435} \u{434}\u{430}\u{442}\u{438}\u{440}\u{43e}\u{432}\u{430}\u{43d}\u{43e}",
        "\u{431}.\u{434}.",
        "\u{431}.\u{434}",
    ],
    eras: &[
        (
            &[
                "\u{434}\u{43e}",
                "\u{43d}\u{430}\u{448}\u{435}\u{439}",
                "\u{44d}\u{440}\u{44b}",
            ],
            true,
        ),
        (&["\u{434}\u{43e}", "\u{43d}", "\u{44d}"], true),
        (&["\u{434}\u{43e}", "\u{43d}\u{44d}"], true),
        (
            &[
                "\u{43d}\u{430}\u{448}\u{435}\u{439}",
                "\u{44d}\u{440}\u{44b}",
            ],
            false,
        ),
        (&["\u{43d}", "\u{44d}"], false),
        (&["\u{43d}\u{44d}"], false),
    ],
    century_words: &[
        "\u{432}\u{435}\u{43a}",
        "\u{432}\u{435}\u{43a}\u{430}",
        "\u{432}\u{435}\u{43a}\u{435}",
        "\u{432}\u{435}\u{43a}\u{43e}\u{432}",
        "\u{432}\u{435}\u{43a}\u{430}\u{445}",
        "\u{432}\u{432}",
        "\u{432}",
        "\u{441}\u{442}\u{43e}\u{43b}\u{435}\u{442}\u{438}\u{435}",
        "\u{441}\u{442}\u{43e}\u{43b}\u{435}\u{442}\u{438}\u{44f}",
        "\u{441}\u{442}\u{43e}\u{43b}\u{435}\u{442}\u{438}\u{438}",
        "\u{441}\u{442}\u{43e}\u{43b}\u{435}\u{442}\u{438}\u{439}",
        "\u{441}\u{442}",
    ],
    decade_suffixes: &["-\u{435}", "-\u{445}"],
    modifiers: &[
        (
            &[
                "\u{43f}\u{435}\u{440}\u{432}\u{430}\u{44f}",
                "\u{43f}\u{43e}\u{43b}\u{43e}\u{432}\u{438}\u{43d}\u{430}",
            ],
            Span::FirstHalf,
        ),
        (
            &[
                "\u{43f}\u{435}\u{440}\u{432}\u{43e}\u{439}",
                "\u{43f}\u{43e}\u{43b}\u{43e}\u{432}\u{438}\u{43d}\u{435}",
            ],
            Span::FirstHalf,
        ),
        (
            &[
                "\u{43f}\u{435}\u{440}\u{432}\u{43e}\u{439}",
                "\u{43f}\u{43e}\u{43b}\u{43e}\u{432}\u{438}\u{43d}\u{44b}",
            ],
            Span::FirstHalf,
        ),
        (
            &[
                "\u{432}\u{442}\u{43e}\u{440}\u{430}\u{44f}",
                "\u{43f}\u{43e}\u{43b}\u{43e}\u{432}\u{438}\u{43d}\u{430}",
            ],
            Span::SecondHalf,
        ),
        (
            &[
                "\u{432}\u{442}\u{43e}\u{440}\u{43e}\u{439}",
                "\u{43f}\u{43e}\u{43b}\u{43e}\u{432}\u{438}\u{43d}\u{435}",
            ],
            Span::SecondHalf,
        ),
        (
            &[
                "\u{432}\u{442}\u{43e}\u{440}\u{43e}\u{439}",
                "\u{43f}\u{43e}\u{43b}\u{43e}\u{432}\u{438}\u{43d}\u{44b}",
            ],
            Span::SecondHalf,
        ),
        (&["\u{43d}\u{430}\u{447}\u{430}\u{43b}\u{43e}"], Span::Early),
        (&["\u{43d}\u{430}\u{447}\u{430}\u{43b}\u{435}"], Span::Early),
        (&["\u{43d}\u{430}\u{447}\u{430}\u{43b}\u{430}"], Span::Early),
        (&["\u{43d}\u{430}\u{447}"], Span::Early),
        (
            &["\u{441}\u{435}\u{440}\u{435}\u{434}\u{438}\u{43d}\u{430}"],
            Span::Mid,
        ),
        (
            &["\u{441}\u{435}\u{440}\u{435}\u{434}\u{438}\u{43d}\u{435}"],
            Span::Mid,
        ),
        (
            &["\u{441}\u{435}\u{440}\u{435}\u{434}\u{438}\u{43d}\u{44b}"],
            Span::Mid,
        ),
        (&["\u{441}\u{435}\u{440}"], Span::Mid),
        (&["\u{43a}\u{43e}\u{43d}\u{435}\u{446}"], Span::Late),
        (&["\u{43a}\u{43e}\u{43d}\u{446}\u{435}"], Span::Late),
        (&["\u{43a}\u{43e}\u{43d}\u{446}\u{430}"], Span::Late),
        (&["\u{43a}\u{43e}\u{43d}"], Span::Late),
    ],
    before_phrases: &[
        &[
            "\u{43d}\u{435}",
            "\u{43f}\u{43e}\u{437}\u{434}\u{43d}\u{435}\u{435}",
        ],
        &["\u{440}\u{430}\u{43d}\u{435}\u{435}"],
        &["\u{434}\u{43e}"],
    ],
    after_phrases: &[
        &["\u{43d}\u{435}", "\u{440}\u{430}\u{43d}\u{435}\u{435}"],
        &[
            "\u{43d}\u{430}\u{447}\u{438}\u{43d}\u{430}\u{44f}",
            "\u{441}",
        ],
        &["\u{43f}\u{43e}\u{441}\u{43b}\u{435}"],
        &["\u{441}"],
    ],
    or_words: &["\u{438}\u{43b}\u{438}"],
    range_pairs: &[
        (Some("\u{441}"), "\u{43f}\u{43e}"),
        (Some("\u{43e}\u{442}"), "\u{434}\u{43e}"),
        (Some("\u{441}"), "\u{434}\u{43e}"),
    ],
    roman_centuries: true,
    implied_numeric_order: Some(NumericOrder::DayFirst),
};
