//! WebAssembly bindings for [`edtf_core`] — the same validator the database
//! runs, compiled for JavaScript. The app imports this in place of edtf.js.
//!
//! Exported API (JS names):
//! - `isValid(input)` → `boolean`
//! - `level(input)` → `0 | 1 | 2 | -1` (-1 = invalid)
//! - `canonical(input)` → `string | undefined` (spec-preferred form)
//! - `parse(input)` → JSON string of a [`Summary`] or `undefined`

use edtf_core::{Bound, Edtf};
use serde::Serialize;
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

/// True if `input` is valid EDTF (levels 0–2).
#[wasm_bindgen(js_name = isValid)]
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
pub fn canonical(input: &str) -> Option<String> {
    Some(Edtf::parse(input).ok()?.to_string())
}

/// Full parse summary as a JSON string, or `undefined` if invalid.
/// See [`Summary`] for the object shape.
#[wasm_bindgen]
pub fn parse(input: &str) -> Option<String> {
    let summary = summarize(input)?;
    serde_json::to_string(&summary).ok()
}

#[cfg(test)]
mod tests {
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
    fn json_is_camel_case() {
        let j = parse("1985").unwrap();
        assert!(j.contains("\"canonical\":\"1985\""));
        assert!(!j.contains("\"level\":1"), "1985 is level 0: {j}");
        assert!(j.contains("\"level\":0"));
    }
}
