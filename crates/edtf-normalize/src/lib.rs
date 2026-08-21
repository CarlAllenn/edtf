// SPDX-FileCopyrightText: Copyright (c) the edtf contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Deterministic prose-date → EDTF normalizer (GitHub issue #20).
//!
//! Real people typing dates into forms need instant, offline, deterministic
//! normalization — "1980s" → `198X` on every keystroke, the same answer every
//! time. This crate is a bounded pattern grammar over date prose: values are
//! constructed through the [`edtf_core`] model and rendered via its canonical
//! `Display`, then re-parsed, so every output is valid, canonical EDTF by
//! construction. Anything outside the grammar is [`Outcome::NoMatch`] — never
//! a silent guess. Genuinely ambiguous inputs ("12/04/1985") report every
//! reading instead of picking one.
//!
//! Pattern tables are per-language ([`Language::English`] and
//! [`Language::Russian`] today); adding a locale means adding one table
//! struct, not touching the grammar. Every judgement call is a numbered
//! N-decision in `docs/normalize-notes.md`, cited from the [`Note`] values
//! attached to results.
//!
//! ```
//! use edtf_normalize::{Outcome, normalize};
//!
//! let Outcome::Normalized(n) = normalize("circa 1920") else {
//!     panic!()
//! };
//! assert_eq!(n.edtf, "1920~");
//!
//! // "12/04/1985" is DD/MM or MM/DD — the form gets both readings, not a guess.
//! assert!(matches!(normalize("12/04/1985"), Outcome::Ambiguous(_)));
//! ```

#![no_std]

extern crate alloc;

use alloc::{string::String, vec::Vec};

use edtf_core::Edtf;

mod engine;
mod tables;

/// Which pattern-table set to match against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    /// English prose ("circa 1920", "19th century", "the 1980s").
    #[default]
    English,
    /// Russian prose ("около 1920 г.", "XIX век", "1980-е").
    Russian,
}

/// Resolution order for all-numeric dates where both readings are plausible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumericOrder {
    /// `12/04/1985` reads as 12 April (DD/MM/YYYY).
    DayFirst,
    /// `12/04/1985` reads as December 4 (MM/DD/YYYY).
    MonthFirst,
}

/// Caller-supplied context. Everything is optional; the defaults never guess.
#[derive(Debug, Clone, Copy, Default)]
pub struct Options {
    /// Pattern-table language. Defaults to English.
    pub language: Language,
    /// Resolves the DD/MM vs MM/DD ambiguity (N5). Overrides any
    /// locale-implied order. `None` (default) reports both readings.
    pub numeric_order: Option<NumericOrder>,
    /// Century base year for bare decades: `Some(1900)` reads "the 80s" as
    /// `198X` (N6). `None` (default) reports the plausible readings.
    /// Domain is `0..=9999` (the value's century is used); values above 9999
    /// are ignored as if unset.
    pub default_century: Option<u16>,
}

/// Why an output looks the way it does. Each variant cites the N-decision in
/// `docs/normalize-notes.md` that governs it, so a form can explain the
/// conversion to the person typing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Note {
    /// The input was already valid EDTF; only canonicalized.
    AlreadyValidEdtf,
    /// early/mid/late or half-of-century mapped to a decade-rounded interval
    /// (N1).
    CenturyPartInterval,
    /// An early/mid/late modifier was dropped as false precision and recorded
    /// (N1).
    ModifierDropped,
    /// "19th century" → `18XX`: centuries run 1801–1900 (N2).
    CenturyMask,
    /// BC centuries have no digit mask (negative years cannot carry `X`);
    /// emitted as exact intervals like `-0199/-0100` (N2).
    BcCenturyInterval,
    /// BC year converted to astronomical numbering: year 0 exists, so
    /// 500 BC → `-0499` (N3).
    AstronomicalYear,
    /// "1914-18": the end year inherits the start's century (N4).
    ElidedEndYear,
    /// Numeric date order was provable from the input: a year-first layout,
    /// or a field whose value exceeds 12 (N5).
    NumericUnambiguous,
    /// Numeric date order resolved by [`Options::numeric_order`] (N5).
    NumericResolvedByOption,
    /// Numeric date order implied by the language's convention (N5).
    NumericResolvedByLocale,
    /// Day and month were equal, so the order cannot matter (N5).
    NumericOrderIrrelevant,
    /// Both numeric readings are plausible; each is reported (N5).
    NumericOrderAmbiguous,
    /// A decade form with more than one reading ("1900s", "the 80s") (N6).
    DecadeAmbiguity,
    /// Bare decade resolved by [`Options::default_century`] (N6).
    DefaultCenturyApplied,
    /// Season name mapped to an ISO 8601-2 sub-year grouping code (N7).
    SeasonCode,
    /// before/after rendered as an open interval `../X` or `X/..` (N8).
    OpenInterval,
    /// A date with no year: the year is masked as `XXXX` (N9).
    MissingYearMasked,
    /// A whole-expression qualifier was distributed over interval endpoints
    /// (N10).
    QualifierDistributed,
    /// "X or Y": each alternative reported instead of picking one (N14).
    OrAlternatives,
    /// `NNNN-NN` collides between an EDTF sub-year code and an elided year
    /// range; both readings reported (N13).
    SeasonRangeCollision,
    /// Century read from a Roman numeral, Cyrillic lookalike letters
    /// tolerated ("XIX век", "ХІХ век") (N15).
    RomanCentury,
    /// A bare decade tied to an explicit century ("60-е годы XIX века" →
    /// `186X`) (N6).
    DecadeOfCentury,
    /// A range endpoint without a year inherited the other endpoint's stated
    /// year ("June-July 1940" → `1940-06/1940-07`) (N16).
    EndpointYearDistributed,
    /// "winter 1941-42" may be one boundary-spanning winter or a
    /// season-to-year range; both readings reported (N17).
    CrossYearSeason,
}

impl Note {
    /// The N-decision in `docs/normalize-notes.md` this note cites, if any.
    #[must_use]
    pub const fn decision(self) -> Option<&'static str> {
        match self {
            Self::AlreadyValidEdtf => None,
            Self::CenturyPartInterval | Self::ModifierDropped => Some("N1"),
            Self::CenturyMask | Self::BcCenturyInterval => Some("N2"),
            Self::AstronomicalYear => Some("N3"),
            Self::ElidedEndYear => Some("N4"),
            Self::NumericUnambiguous
            | Self::NumericResolvedByOption
            | Self::NumericResolvedByLocale
            | Self::NumericOrderIrrelevant
            | Self::NumericOrderAmbiguous => Some("N5"),
            Self::DecadeAmbiguity | Self::DefaultCenturyApplied | Self::DecadeOfCentury => {
                Some("N6")
            }
            Self::SeasonCode => Some("N7"),
            Self::OpenInterval => Some("N8"),
            Self::MissingYearMasked => Some("N9"),
            Self::QualifierDistributed => Some("N10"),
            Self::SeasonRangeCollision => Some("N13"),
            Self::OrAlternatives => Some("N14"),
            Self::RomanCentury => Some("N15"),
            Self::EndpointYearDistributed => Some("N16"),
            Self::CrossYearSeason => Some("N17"),
        }
    }

    /// A short human-readable explanation of the note.
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::AlreadyValidEdtf => "input was already valid EDTF; canonicalized",
            Self::CenturyPartInterval => {
                "part-of-century phrase mapped to a decade-rounded year interval"
            }
            Self::ModifierDropped => {
                "early/mid/late modifier dropped (sub-decade precision would be false)"
            }
            Self::CenturyMask => "Nth century runs (N-1)01 to N00, so it masks as (N-1)XX",
            Self::BcCenturyInterval => {
                "BC centuries cannot be digit-masked; emitted as an exact year interval"
            }
            Self::AstronomicalYear => "BC year converted to astronomical numbering (year 0 exists)",
            Self::ElidedEndYear => "elided end year inherits the start year's century",
            Self::NumericUnambiguous => {
                "field order provable from the input (year-first layout or a value over 12)"
            }
            Self::NumericResolvedByOption => "field order resolved by caller options",
            Self::NumericResolvedByLocale => "field order implied by the language's convention",
            Self::NumericOrderIrrelevant => "day and month are equal; order cannot matter",
            Self::NumericOrderAmbiguous => "day/month order unknowable from the input",
            Self::DecadeAmbiguity => "decade form has more than one plausible reading",
            Self::DefaultCenturyApplied => "bare decade resolved by the configured century",
            Self::SeasonCode => "season mapped to ISO 8601-2 sub-year grouping code",
            Self::OpenInterval => "before/after expressed as an open interval",
            Self::MissingYearMasked => "no year given; year masked as XXXX",
            Self::QualifierDistributed => {
                "whole-expression qualifier applied to every interval endpoint"
            }
            Self::OrAlternatives => "alternatives reported instead of picking one",
            Self::SeasonRangeCollision => {
                "NNNN-NN is both an EDTF sub-year code and a plausible year range"
            }
            Self::RomanCentury => {
                "century read from a Roman numeral (Cyrillic lookalike letters tolerated)"
            }
            Self::DecadeOfCentury => "decade tied to the explicitly named century",
            Self::EndpointYearDistributed => {
                "endpoint without a year inherited the other endpoint's stated year"
            }
            Self::CrossYearSeason => {
                "season-year-pair prose may name one boundary-spanning season or a range"
            }
        }
    }
}

/// A successful, unambiguous normalization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Normalized {
    /// The canonical EDTF string. Guaranteed to parse in [`edtf_core`].
    pub edtf: String,
    /// The parsed value behind [`Normalized::edtf`] (always
    /// `Edtf::parse(&edtf).unwrap()`).
    pub value: Edtf,
    /// Why the output looks the way it does.
    pub notes: Vec<Note>,
}

/// One plausible reading of an ambiguous input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interpretation {
    /// The canonical EDTF string for this reading.
    pub edtf: String,
    /// The parsed value behind [`Interpretation::edtf`].
    pub value: Edtf,
    /// Which reading this is ("day-month-year", "century (19XX)", …).
    pub reading: String,
    /// Why this reading exists.
    pub notes: Vec<Note>,
}

/// A set of plausible readings. Always at least two; the form should ask,
/// not pick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ambiguous {
    /// Every plausible reading, in table order.
    pub interpretations: Vec<Interpretation>,
}

/// Why a [`Outcome::NoMatch`] happened — the discriminator a bulk-import
/// triage step routes on (N11/N12).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoMatchReason {
    /// The input is outside the pattern grammar (N11). Escalate to a human.
    OutOfGrammar,
    /// The input explicitly says there is no date ("unknown", "без даты")
    /// (N12). What that means — `XXXX`, an empty column, an error — is form
    /// policy.
    ExplicitNoDate,
    /// The prose matched, but every reading named an impossible or
    /// contradictory date (February 30, a reversed range, an impossible
    /// "or" alternative). A prose error: escalate, never guess (N14).
    ImpossibleDate,
}

impl NoMatchReason {
    /// The N-decision in `docs/normalize-notes.md` this reason cites.
    #[must_use]
    pub const fn decision(self) -> &'static str {
        match self {
            Self::OutOfGrammar => "N11",
            Self::ExplicitNoDate => "N12",
            Self::ImpossibleDate => "N14",
        }
    }
}

/// The honest return type: a match, several readings, or nothing — never a
/// silent guess.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// One deterministic answer.
    Normalized(Normalized),
    /// More than one plausible reading; the caller decides.
    Ambiguous(Ambiguous),
    /// No answer, with the reason a triage step needs — this crate will not
    /// guess.
    NoMatch {
        /// Why nothing was produced.
        reason: NoMatchReason,
    },
}

/// Normalize English date prose with default options.
#[must_use]
pub fn normalize(input: &str) -> Outcome {
    engine::run(input, Options::default())
}

/// Normalize with explicit [`Options`] (language, numeric order, default
/// century).
#[must_use]
pub fn normalize_with(input: &str, options: Options) -> Outcome {
    engine::run(input, options)
}
