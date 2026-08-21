// SPDX-FileCopyrightText: Copyright (c) the edtf contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! WebAssembly bindings for [`edtf_core`] — the same validator the database
//! runs, compiled for JavaScript. The app imports this in place of edtf.js.
//!
//! Exported API (JS names):
//! - `isValid(input)` → `boolean`
//! - `level(input)` → `0 | 1 | 2 | -1` (-1 = invalid)
//! - `canonical(input)` → `string | undefined` (spec-preferred form)
//! - `parse(input)` → JSON string of a [`Summary`] or `undefined`
//! - `relation(a, b)` → JSON string of a [`RelationSummary`] or `undefined`
//! - `normalize(input, options?)` → JSON string of a [`NormalizeSummary`]
//!   (always returns; `{"kind":"noMatch"}` when the prose is outside the
//!   grammar). `options` is a JSON string: `{"language": "en"|"ru",
//!   "numericOrder": "dayFirst"|"monthFirst", "defaultCentury": 1900}` — all
//!   fields optional.

use edtf_core::{Bound, Edtf, Relation};
use edtf_normalize::{Language, NoMatchReason, NumericOrder, Options, Outcome, normalize_with};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::wasm_bindgen;

/// The JSON shape `parse` returns; one object per valid expression.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    /// The canonical (spec-preferred) rendering of the input.
    pub canonical: String,
    /// Minimum EDTF conformance level (0, 1 or 2).
    pub level: u8,
    /// `"date" | "datetime" | "interval" | "set"`.
    pub kind: &'static str,
    /// `"year" | "month" | "season" | "day" | null` (dates/datetimes only).
    pub precision: Option<&'static str>,
    /// Earliest calendar day: `"YYYY-MM-DD"`, `"-infinity"`, or null (unknown).
    pub earliest: Option<String>,
    /// Latest calendar day: `"YYYY-MM-DD"`, `"infinity"`, or null (unknown).
    pub latest: Option<String>,
    /// Any component marked uncertain (`?` or `%`).
    pub uncertain: bool,
    /// Any component marked approximate (`~` or `%`).
    pub approximate: bool,
    /// Any component with unspecified digits (`X`).
    pub unspecified: bool,
}

fn bound_json(b: Bound) -> Option<String> {
    match b {
        Bound::Date(d) => Some(d.to_string()),
        Bound::NegativeInfinity => Some("-infinity".into()),
        Bound::PositiveInfinity => Some("infinity".into()),
        Bound::Unknown => None,
    }
}

/// Build the [`Summary`] for an input, if it is valid EDTF.
#[must_use]
pub fn summarize(input: &str) -> Option<Summary> {
    let parsed = Edtf::parse(input).ok()?;
    let bounds = parsed.bounds();
    let (kind, precision) = match &parsed {
        Edtf::Date(d) => ("date", Some(precision_str(d))),
        Edtf::DateTime(dt) => ("datetime", Some(precision_str(&dt.date))),
        Edtf::Interval(_) => ("interval", None),
        Edtf::Set(_) => ("set", None),
    };
    Some(Summary {
        canonical: parsed.to_string(),
        level: parsed.level(),
        kind,
        precision,
        earliest: bound_json(bounds.earliest),
        latest: bound_json(bounds.latest),
        uncertain: parsed.is_uncertain(),
        approximate: parsed.is_approximate(),
        unspecified: parsed.has_unspecified(),
    })
}

fn precision_str(d: &edtf_core::Date) -> &'static str {
    match d.precision() {
        edtf_core::Precision::Year => "year",
        edtf_core::Precision::Month => "month",
        edtf_core::Precision::Season => "season",
        edtf_core::Precision::Day => "day",
    }
}

/// The JSON shape `relation` returns.
///
/// The modality of each of the six coarsened Allen relations between the
/// two inputs, each `"impossible" | "possible" | "definite"`. Semantics:
/// `docs/spec-notes.md` D23 (possible-completions over bounds regions;
/// Unknown bounds are possible-everything, never definite).
#[derive(Debug, Serialize)]
pub struct RelationSummary {
    /// A ends before B starts.
    pub before: &'static str,
    /// A starts after B ends.
    pub after: &'static str,
    /// Partial overlap on opposite sides.
    pub overlaps: &'static str,
    /// B lies within A without being equal.
    pub contains: &'static str,
    /// A lies within B without being equal.
    pub within: &'static str,
    /// A and B cover exactly the same days.
    pub equal: &'static str,
}

/// Build the [`RelationSummary`] for two inputs, if both are valid EDTF.
#[must_use]
pub fn relate(a: &str, b: &str) -> Option<RelationSummary> {
    let rel = Edtf::parse(a).ok()?.relation(&Edtf::parse(b).ok()?);
    let m = |r: Relation| rel.modality(r).as_str();
    Some(RelationSummary {
        before: m(Relation::Before),
        after: m(Relation::After),
        overlaps: m(Relation::Overlaps),
        contains: m(Relation::Contains),
        within: m(Relation::Within),
        equal: m(Relation::Equal),
    })
}

/// The JSON shape `normalize` returns, discriminated by `kind`:
/// `"normalized"` | `"ambiguous"` | `"noMatch"`. Semantics and the N-decision
/// ids inside notes: `docs/normalize-notes.md`.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum NormalizeSummary {
    /// One deterministic answer.
    #[serde(rename_all = "camelCase")]
    Normalized {
        /// Canonical EDTF, guaranteed valid.
        edtf: String,
        /// Its conformance level (0, 1 or 2).
        level: u8,
        /// Why the output looks the way it does.
        notes: Vec<NoteJson>,
    },
    /// More than one plausible reading; the form should ask, not pick.
    #[serde(rename_all = "camelCase")]
    Ambiguous {
        /// Every plausible reading, in table order.
        interpretations: Vec<InterpretationJson>,
    },
    /// No answer; `reason` is the triage discriminator:
    /// `"outOfGrammar"` (N11) | `"explicitNoDate"` (N12) |
    /// `"impossibleDate"` (N14).
    #[serde(rename_all = "camelCase")]
    NoMatch {
        /// Why nothing was produced.
        reason: &'static str,
        /// The governing N-decision in docs/normalize-notes.md.
        decision: &'static str,
    },
}

/// One note: the governing N-decision (or null) and a human-readable message.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteJson {
    /// N-decision id in docs/normalize-notes.md, e.g. "N3".
    pub decision: Option<&'static str>,
    /// Short explanation of the mapping.
    pub message: &'static str,
}

/// One reading of an ambiguous input.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InterpretationJson {
    /// Canonical EDTF for this reading.
    pub edtf: String,
    /// Its conformance level.
    pub level: u8,
    /// Which reading this is ("day-month-year", "century (19XX)", …).
    pub reading: String,
    /// Why this reading exists.
    pub notes: Vec<NoteJson>,
}

/// JSON options accepted by `normalize` (all fields optional). Unknown keys
/// are rejected, matching the invalid-value behavior — a typo'd option must
/// never silently fall back to defaults.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
struct NormalizeOptionsJson {
    language: Option<String>,
    numeric_order: Option<String>,
    default_century: Option<u16>,
}

fn parse_options(options: Option<&str>) -> Option<Options> {
    let Some(raw) = options.filter(|s| !s.trim().is_empty()) else {
        return Some(Options::default());
    };
    let parsed: NormalizeOptionsJson = serde_json::from_str(raw).ok()?;
    let language = match parsed.language.as_deref() {
        None | Some("en") => Language::English,
        Some("ru") => Language::Russian,
        Some(_) => return None,
    };
    let numeric_order = match parsed.numeric_order.as_deref() {
        None => None,
        Some("dayFirst") => Some(NumericOrder::DayFirst),
        Some("monthFirst") => Some(NumericOrder::MonthFirst),
        Some(_) => return None,
    };
    // Out-of-domain values are option errors (undefined to JS), never a
    // fabricated noMatch for in-grammar prose.
    if parsed.default_century.is_some_and(|c| c > 9999) {
        return None;
    }
    Some(Options {
        language,
        numeric_order,
        default_century: parsed.default_century,
    })
}

fn notes_json(notes: &[edtf_normalize::Note]) -> Vec<NoteJson> {
    notes
        .iter()
        .map(|n| NoteJson {
            decision: n.decision(),
            message: n.message(),
        })
        .collect()
}

/// Build the [`NormalizeSummary`] for an input. `None` only for invalid
/// options JSON.
#[must_use]
pub fn normalize_summary(input: &str, options: Option<&str>) -> Option<NormalizeSummary> {
    let opts = parse_options(options)?;
    Some(match normalize_with(input, opts) {
        Outcome::Normalized(n) => NormalizeSummary::Normalized {
            level: n.value.level(),
            notes: notes_json(&n.notes),
            edtf: n.edtf,
        },
        Outcome::Ambiguous(a) => NormalizeSummary::Ambiguous {
            interpretations: a
                .interpretations
                .into_iter()
                .map(|i| InterpretationJson {
                    level: i.value.level(),
                    notes: notes_json(&i.notes),
                    edtf: i.edtf,
                    reading: i.reading,
                })
                .collect(),
        },
        Outcome::NoMatch { reason } => NormalizeSummary::NoMatch {
            reason: match reason {
                NoMatchReason::OutOfGrammar => "outOfGrammar",
                NoMatchReason::ExplicitNoDate => "explicitNoDate",
                NoMatchReason::ImpossibleDate => "impossibleDate",
            },
            decision: reason.decision(),
        },
    })
}

/// Deterministic prose-date normalization ("1980s" → 198X) as a JSON string.
///
/// Always returns a value for valid options (a `noMatch` object with a
/// `reason` when nothing can be produced); `undefined` only for invalid
/// options — malformed JSON, unknown keys, or out-of-domain values. An
/// empty/whitespace options string means defaults. See [`NormalizeSummary`]
/// for the object shape.
#[wasm_bindgen]
#[must_use]
#[expect(
    clippy::needless_pass_by_value,
    reason = "wasm-bindgen passes JS strings owned, not as Option<&str>"
)]
pub fn normalize(input: &str, options: Option<String>) -> Option<String> {
    let summary = normalize_summary(input, options.as_deref())?;
    serde_json::to_string(&summary).ok()
}

/// True if `input` is valid EDTF (levels 0–2).
#[wasm_bindgen(js_name = isValid)]
#[must_use]
pub fn is_valid(input: &str) -> bool {
    edtf_core::is_valid(input)
}

/// Minimum conformance level of `input`: 0, 1 or 2; -1 if invalid.
#[wasm_bindgen]
pub fn level(input: &str) -> i32 {
    edtf_core::level(input).map_or(-1, i32::from)
}

/// The canonical (spec-preferred) form of `input`, or `undefined` if invalid.
#[wasm_bindgen]
#[must_use]
pub fn canonical(input: &str) -> Option<String> {
    Some(Edtf::parse(input).ok()?.to_string())
}

/// Full parse summary as a JSON string, or `undefined` if invalid.
/// See [`Summary`] for the object shape.
#[wasm_bindgen]
#[must_use]
pub fn parse(input: &str) -> Option<String> {
    let summary = summarize(input)?;
    serde_json::to_string(&summary).ok()
}

/// Three-valued temporal relation between `a` and `b` as a JSON string, or
/// `undefined` if either input is invalid. See [`RelationSummary`] for the
/// object shape.
#[wasm_bindgen]
#[must_use]
pub fn relation(a: &str, b: &str) -> Option<String> {
    serde_json::to_string(&relate(a, b)?).ok()
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        reason = "test code: a panic here is the failure signal, not a crash path"
    )]

    use super::*;

    #[test]
    fn summary_shape() {
        let s = summarize("2004-06~-11").unwrap();
        assert_eq!(s.canonical, "2004-06~-11");
        assert_eq!(s.level, 2);
        assert_eq!(s.kind, "date");
        assert_eq!(s.precision, Some("day"));
        assert_eq!(s.earliest.as_deref(), Some("2004-06-11"));
        assert_eq!(s.latest.as_deref(), Some("2004-06-11"));
        assert!(s.approximate);
        assert!(!s.uncertain);
        assert!(!s.unspecified);
    }

    #[test]
    fn open_interval_summary() {
        let s = summarize("1985-04-12/..").unwrap();
        assert_eq!(s.kind, "interval");
        assert_eq!(s.earliest.as_deref(), Some("1985-04-12"));
        assert_eq!(s.latest.as_deref(), Some("infinity"));
    }

    #[test]
    fn unknown_bound_is_null() {
        let s = summarize("1986-04/").unwrap();
        assert_eq!(s.latest, None);
    }

    #[test]
    fn invalid_yields_none() {
        assert!(summarize("1985-02-30").is_none());
        assert_eq!(level("1985-02-30"), -1);
        assert!(is_valid("1985-04-12"));
    }

    #[test]
    fn relation_shape() {
        let r = relate("1985~", "199X").unwrap();
        assert_eq!(r.before, "definite");
        assert_eq!(r.after, "impossible");
        assert_eq!(r.equal, "impossible");
        let j = relation("198X", "1985").unwrap();
        assert_eq!(
            j,
            "{\"before\":\"possible\",\"after\":\"possible\",\
                \"overlaps\":\"possible\",\"contains\":\"possible\",\
                \"within\":\"possible\",\"equal\":\"possible\"}"
        );
        assert!(relation("junk", "1985").is_none());
        assert!(relation("1985", "junk").is_none());
    }

    #[test]
    fn normalize_shapes() {
        let j = normalize("circa 1920", None).unwrap();
        assert!(j.contains("\"kind\":\"normalized\""), "{j}");
        assert!(j.contains("\"edtf\":\"1920~\""), "{j}");
        assert!(j.contains("\"level\":1"), "{j}");

        let j = normalize("12/04/1985", None).unwrap();
        assert!(j.contains("\"kind\":\"ambiguous\""), "{j}");
        assert!(j.contains("\"1985-04-12\""), "{j}");
        assert!(j.contains("\"1985-12-04\""), "{j}");
        assert!(j.contains("\"decision\":\"N5\""), "{j}");

        let j = normalize("no idea", None).unwrap();
        assert_eq!(
            j,
            "{\"kind\":\"noMatch\",\"reason\":\"outOfGrammar\",\"decision\":\"N11\"}"
        );
        let j = normalize("unknown", None).unwrap();
        assert!(j.contains("\"reason\":\"explicitNoDate\""), "{j}");
        assert!(j.contains("\"decision\":\"N12\""), "{j}");
        let j = normalize("30/02/1985", None).unwrap();
        assert!(j.contains("\"reason\":\"impossibleDate\""), "{j}");
    }

    #[test]
    fn normalize_options() {
        let ru = Some(String::from("{\"language\":\"ru\"}"));
        let j = normalize("около 1920 г.", ru).unwrap();
        assert!(j.contains("\"edtf\":\"1920~\""), "{j}");

        let df = Some(String::from("{\"numericOrder\":\"dayFirst\"}"));
        let j = normalize("12/04/1985", df).unwrap();
        assert!(j.contains("\"kind\":\"normalized\""), "{j}");
        assert!(j.contains("\"edtf\":\"1985-04-12\""), "{j}");

        let dc = Some(String::from("{\"defaultCentury\":1900}"));
        let j = normalize("the 80s", dc).unwrap();
        assert!(j.contains("\"edtf\":\"198X\""), "{j}");

        assert!(normalize("1985", Some(String::from("{\"language\":\"tlh\"}"))).is_none());
        assert!(normalize("1985", Some(String::from("not json"))).is_none());
        // A typo'd KEY must be an option error, not a silent default (a
        // misspelled "lang" would otherwise silently run English tables).
        assert!(normalize("около 1920", Some(String::from("{\"lang\":\"ru\"}"))).is_none());
        // Out-of-domain defaultCentury is an option error, not a noMatch.
        assert!(normalize("the 80s", Some(String::from("{\"defaultCentury\":12345}"))).is_none());
    }

    #[test]
    fn summary_kinds_and_precisions() {
        let s = summarize("../1985").unwrap();
        assert_eq!(s.earliest.as_deref(), Some("-infinity"));
        let s = summarize("1985-04-12T10:00:00Z").unwrap();
        assert_eq!(s.kind, "datetime");
        assert_eq!(s.precision, Some("day"));
        let s = summarize("[1985,1987]").unwrap();
        assert_eq!(s.kind, "set");
        assert_eq!(s.precision, None);
        for (input, precision) in [
            ("1985", "year"),
            ("1985-04", "month"),
            ("2001-21", "season"),
        ] {
            assert_eq!(summarize(input).unwrap().precision, Some(precision));
        }
    }

    #[test]
    fn month_first_option_and_invalid_order() {
        let mf = Some(String::from("{\"numericOrder\":\"monthFirst\"}"));
        let j = normalize("12/04/1985", mf).unwrap();
        assert!(j.contains("\"edtf\":\"1985-12-04\""), "{j}");
        // An out-of-domain value is an option error, never a noMatch.
        let bad = Some(String::from("{\"numericOrder\":\"yearFirst\"}"));
        assert!(normalize("12/04/1985", bad).is_none());
    }

    #[test]
    fn canonical_form() {
        assert_eq!(canonical("?2004-?06-?11").as_deref(), Some("2004-06-11?"));
        assert!(canonical("1985-02-30").is_none());
    }

    #[test]
    fn json_is_camel_case() {
        let j = parse("1985").unwrap();
        assert!(j.contains("\"canonical\":\"1985\""));
        assert!(!j.contains("\"level\":1"), "1985 is level 0: {j}");
        assert!(j.contains("\"level\":0"));
    }
}
