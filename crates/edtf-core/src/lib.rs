// SPDX-FileCopyrightText: Copyright (c) the edtf contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! EDTF (Extended Date/Time Format, ISO 8601-2:2019 Annex A) parsing,
//! validation, level classification, calendar bounds, three-valued temporal
//! relations, value enumeration, and canonical formatting — conformance
//! levels 0–2, complete.
//!
//! `#![no_std]` (requires `alloc`), zero runtime dependencies; JSON support
//! behind the optional `serde` feature.
//!
//! ```
//! use edtf_core::{Bound, Edtf};
//!
//! assert!(edtf_core::is_valid("1985-04-12")); // level 0
//! assert!(edtf_core::is_valid("2004-06~-11")); // level 2 group qualification
//! assert!(!edtf_core::is_valid("1985-02-30")); // no such calendar day
//!
//! let d = Edtf::parse("1985-04-12?").unwrap();
//! assert_eq!(d.level(), 1);
//! assert!(d.is_uncertain());
//!
//! // Every expression maps to earliest/latest calendar-day bounds:
//! let decade = Edtf::parse("156X").unwrap().bounds();
//! assert_eq!(
//!     format!(
//!         "{}",
//!         match decade.earliest {
//!             Bound::Date(d) => d,
//!             _ => panic!(),
//!         }
//!     ),
//!     "1560-01-01"
//! );
//! assert_eq!(
//!     format!(
//!         "{}",
//!         match decade.latest {
//!             Bound::Date(d) => d,
//!             _ => panic!(),
//!         }
//!     ),
//!     "1569-12-31"
//! );
//!
//! // Display renders the canonical (spec-preferred) form:
//! let messy = Edtf::parse("?2004-?06-?11").unwrap();
//! assert_eq!(messy.to_string(), "2004-06-11?");
//!
//! // Three-valued comparison under uncertainty (see docs/spec-notes.md D23):
//! use edtf_core::Relation;
//! let a = Edtf::parse("1985~").unwrap();
//! let b = Edtf::parse("199X").unwrap();
//! assert_eq!(a.relation(&b).definite(), Some(Relation::Before));
//!
//! // Enumerate the values an expression denotes (see D24-D29):
//! let set = Edtf::parse("{1667,1668,1670..1672}").unwrap();
//! let years: Vec<String> = set.values().unwrap().map(|v| v.to_string()).collect();
//! assert_eq!(years, ["1667", "1668", "1670", "1671", "1672"]);
//! ```
//!
//! The grammar and every validation decision are documented with ISO section
//! citations in `docs/spec-notes.md` at the repository root.
#![no_std]
#![expect(
    clippy::min_ident_chars,
    reason = "y, m and d are the universal notation for a date's components, and this crate is about little else"
)]

extern crate alloc;

mod bounds;
mod display;
mod enumerate;
mod parser;
mod relation;
mod types;

pub use bounds::{Bound, BoundDate, Bounds};
pub use enumerate::{Unenumerable, Values};
pub use relation::{Modality, Relation, Relations};
pub use types::{
    Date, DateField, DateTime, Edtf, Interval, IntervalEndpoint, ParseError, Precision, Qualifier,
    Set, SetElement, SetKind, Time, TimeShift, Year, YearKind,
};

/// Returns true if `input` is a valid EDTF string (levels 0–2).
#[must_use]
pub fn is_valid(input: &str) -> bool {
    Edtf::parse(input).is_ok()
}

/// Parse `input` and return its minimum EDTF conformance level (0, 1 or 2),
/// or `None` if it is not valid EDTF.
#[must_use]
pub fn level(input: &str) -> Option<u8> {
    Edtf::parse(input).ok().map(|e| e.level())
}
