//! EDTF (Extended Date/Time Format, ISO 8601-2:2019 Annex A) parsing and
//! validation, conformance levels 0–2. Zero runtime dependencies; JSON
//! support behind the optional `serde` feature.
//!
//! ```
//! use edtf_core::Edtf;
//!
//! assert!(edtf_core::is_valid("1985-04-12"));           // level 0
//! assert!(edtf_core::is_valid("2004-06~-11"));          // level 2 group qualification
//! assert!(!edtf_core::is_valid("1985-02-30"));          // no such calendar day
//!
//! let d = Edtf::parse("1985-04-12?").unwrap();
//! assert_eq!(d.level(), 1);
//! ```
//!
//! The grammar and every validation decision are documented with ISO section
//! citations in `docs/spec-notes.md` at the repository root.

mod parser;
mod types;

pub use types::{
    Date, DateField, DateTime, Edtf, Interval, IntervalEndpoint, ParseError, Precision, Qualifier,
    Set, SetElement, SetKind, Time, TimeShift, Year, YearKind,
};

/// Returns true if `input` is a valid EDTF string (levels 0–2).
pub fn is_valid(input: &str) -> bool {
    Edtf::parse(input).is_ok()
}

/// Parse `input` and return its minimum EDTF conformance level (0, 1 or 2),
/// or `None` if it is not valid EDTF.
pub fn level(input: &str) -> Option<u8> {
    Edtf::parse(input).ok().map(|e| e.level())
}
