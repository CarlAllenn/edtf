// SPDX-FileCopyrightText: Copyright (c) the edtf contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Three-valued temporal relations between EDTF expressions.
//!
//! [`Edtf::relation`] answers "how does A relate to B in time?" honestly
//! under uncertainty: each of the six coarsened Allen relations (*before /
//! after / overlaps / contains / within / equal*) is reported as
//! [`Modality::Impossible`], [`Modality::Possible`] (holds for some
//! completion) or [`Modality::Definite`] (holds for every completion).
//!
//! Semantics (decision D23 in `docs/spec-notes.md`): an expression denotes
//! some nonempty day-interval lying within its [`Edtf::bounds`] region —
//! the "sometime during" reading. `1985` is a value falling somewhere
//! within the calendar year, not necessarily spanning the whole of it.
//! Consequences, documented rather than hidden:
//!
//! - Qualification (`?~%`) never moves bounds (ISO 8601-2 §8.4.2 NOTE), so
//!   `1985?` relates exactly as `1985`.
//! - Only *before*, *after* and *equal* can ever be Definite: any region wider
//!   than one day admits single-day completions, so containment or overlap can
//!   never be forced.
//! - Interval endpoint linkage is coarsened away: `2004/2005` vs `2004-06`
//!   reports possibly-before, although every true completion of the interval
//!   straddles June 2004. Everything flows through the bounds region.
//! - Unknown bounds propagate as possible-everything, never Definite — even
//!   where the other endpoint could in principle constrain them.
//! - Bounds are day-granular (time of day refines within a day), so same-day
//!   datetimes are definitely equal.

#![expect(
    clippy::min_ident_chars,
    reason = "y, m and d are the universal notation for a date's components, and this crate is about little else"
)]
#![expect(
    clippy::missing_inline_in_public_items,
    reason = "inlining is the compiler's call across a crate boundary, and the release profile enables fat LTO — annotating every public item would assert a decision this crate has not measured"
)]
#![expect(
    clippy::arithmetic_side_effects,
    reason = "every flagged operation is bounded where it stands: slice indices by a length guard on the line above, digit values to 0-9 by the match arm that binds them, and the JDN forms by being computed in i128 so any i64 year fits. The operations that genuinely could leave range already use checked_/saturating_ and return an error rather than wrapping"
)]
#![expect(
    clippy::absolute_paths,
    reason = "a one-use std path written in full at the call site is clearer than an import that only appears once"
)]
#![expect(
    clippy::single_call_fn,
    reason = "a named helper used once is extraction for readability, which is the opposite of a defect; several are also the named steps the module docs describe"
)]
#![expect(
    clippy::exhaustive_enums,
    reason = "these enums enumerate what ISO 8601-2 defines; the spec fixes the variants, so non_exhaustive would promise additions the format cannot make"
)]
#![expect(
    clippy::wildcard_enum_match_arm,
    reason = "the wildcard covers variants the arm above has already narrowed by construction; naming them would be dead code the compiler cannot check"
)]
#![expect(
    clippy::too_long_first_doc_paragraph,
    reason = "the items are crate-private, so these paragraphs render in no rustdoc summary; they are written to be read next to the code they describe"
)]

use crate::{
    bounds::{Bound, BoundDate, is_leap, last_day},
    types::Edtf,
};

/// One of the six coarsened Allen relations between two time regions.
///
/// The six are exhaustive and mutually exclusive over concrete
/// day-intervals: disjoint pairs are `Before`/`After`, coincident pairs are
/// `Equal`, proper containment (including a shared endpoint) is
/// `Contains`/`Within`, and partial overlap is `Overlaps`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Relation {
    /// A ends before B starts (no shared day).
    Before,
    /// A starts after B ends (no shared day).
    After,
    /// A and B share days but each also has days the other lacks on
    /// opposite sides (partial overlap).
    Overlaps,
    /// B lies within A without being equal to it.
    Contains,
    /// A lies within B without being equal to it.
    Within,
    /// A and B cover exactly the same days.
    Equal,
}

impl Relation {
    /// All six relations, in canonical order.
    pub const ALL: [Self; 6] = [
        Self::Before,
        Self::After,
        Self::Overlaps,
        Self::Contains,
        Self::Within,
        Self::Equal,
    ];

    /// The relation that holds of (B, A) whenever `self` holds of (A, B).
    #[must_use]
    pub const fn converse(self) -> Self {
        match self {
            Self::Before => Self::After,
            Self::After => Self::Before,
            Self::Contains => Self::Within,
            Self::Within => Self::Contains,
            Self::Overlaps | Self::Equal => self,
        }
    }

    /// Lower-case name: `"before"`, `"after"`, `"overlaps"`, `"contains"`,
    /// `"within"` or `"equal"`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Before => "before",
            Self::After => "after",
            Self::Overlaps => "overlaps",
            Self::Contains => "contains",
            Self::Within => "within",
            Self::Equal => "equal",
        }
    }

    /// The relation's index into a modality table.
    const fn idx(self) -> usize {
        match self {
            Self::Before => 0,
            Self::After => 1,
            Self::Overlaps => 2,
            Self::Contains => 3,
            Self::Within => 4,
            Self::Equal => 5,
        }
    }
}

/// How firmly a relation holds across the completions of two expressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Modality {
    /// Holds for no pair of completions.
    Impossible,
    /// Holds for some pair of completions, but not all.
    Possible,
    /// Holds for every pair of completions.
    Definite,
}

impl Modality {
    /// Lower-case name: `"impossible"`, `"possible"` or `"definite"`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Impossible => "impossible",
            Self::Possible => "possible",
            Self::Definite => "definite",
        }
    }
}

/// The modality of every [`Relation`] between one pair of expressions, as
/// computed by [`Edtf::relation`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Relations {
    /// One modality per Allen relation, in `Relation`'s own order.
    modalities: [Modality; 6],
}

impl Relations {
    /// Every relation possible, none definite — the honest answer when a
    /// bound is unknown.
    const ALL_POSSIBLE: Self = Self {
        modalities: [Modality::Possible; 6],
    };

    /// A possible-set becomes modalities: the sole possible relation (when
    /// there is exactly one) holds for *every* completion pair, because the
    /// six relations are exhaustive and mutually exclusive.
    fn from_possible(possible: [bool; 6]) -> Self {
        let count = possible.iter().filter(|p| **p).count();
        let mut modalities = [Modality::Impossible; 6];
        for (m, p) in modalities.iter_mut().zip(possible) {
            if p {
                *m = if count == 1 {
                    Modality::Definite
                } else {
                    Modality::Possible
                };
            }
        }
        Self { modalities }
    }

    /// The modality of one relation.
    #[must_use]
    pub const fn modality(self, r: Relation) -> Modality {
        self.modalities[r.idx()]
    }

    /// True if the relation holds for at least one completion pair
    /// (i.e. its modality is `Possible` or `Definite`).
    #[must_use]
    pub fn is_possible(self, r: Relation) -> bool {
        self.modality(r) != Modality::Impossible
    }

    /// True if the relation holds for every completion pair.
    #[must_use]
    pub fn is_definite(self, r: Relation) -> bool {
        self.modality(r) == Modality::Definite
    }

    /// True if the relation holds for no completion pair.
    #[must_use]
    pub fn is_impossible(self, r: Relation) -> bool {
        self.modality(r) == Modality::Impossible
    }

    /// The one relation that definitely holds, if any. At most one relation
    /// can be definite; only `Before`, `After` and `Equal` ever are.
    #[must_use]
    pub fn definite(self) -> Option<Relation> {
        Relation::ALL.into_iter().find(|r| self.is_definite(*r))
    }

    /// The relations that hold for at least one completion pair, in
    /// canonical order. Never empty.
    pub fn possible(self) -> impl Iterator<Item = Relation> {
        Relation::ALL
            .into_iter()
            .filter(move |r| self.is_possible(*r))
    }
}

impl core::fmt::Display for Relations {
    /// Comma-separated non-impossible relations, e.g. `definitely before`
    /// or `possibly before, possibly overlaps, possibly within`.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut first = true;
        for r in self.possible() {
            if !first {
                write!(f, ", ")?;
            }
            first = false;
            let adverb = match self.modality(r) {
                Modality::Definite => "definitely",
                _ => "possibly",
            };
            write!(f, "{adverb} {}", r.as_str())?;
        }
        Ok(())
    }
}

/// A bound on the day axis with infinities ordered around concrete days.
/// `Unknown` is handled before conversion, so no variant is needed for it.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Ext {
    /// Earlier than every day: an unbounded start.
    NegInf,
    /// A concrete calendar day.
    Day(BoundDate),
    /// Later than every day: an unbounded end.
    PosInf,
}

/// Lift a bound onto the extended day line, or `None` if unknown.
const fn ext(b: Bound) -> Option<Ext> {
    match b {
        Bound::NegativeInfinity => Some(Ext::NegInf),
        Bound::Date(d) => Some(Ext::Day(d)),
        Bound::PositiveInfinity => Some(Ext::PosInf),
        Bound::Unknown => None,
    }
}

/// The calendar day after `e`; year overflow saturates to `PosInf`, which
/// only under-reports overlap feasibility at the edge of representable time.
fn succ(e: Ext) -> Ext {
    let Ext::Day(d) = e else { return e };
    if d.day < last_day(d.month, is_leap(d.year)) {
        Ext::Day(BoundDate {
            day: d.day + 1,
            ..d
        })
    } else if d.month < 12 {
        Ext::Day(BoundDate {
            year: d.year,
            month: d.month + 1,
            day: 1,
        })
    } else {
        d.year.checked_add(1).map_or(Ext::PosInf, |year| {
            Ext::Day(BoundDate {
                year,
                month: 1,
                day: 1,
            })
        })
    }
}

/// The calendar day before `e`; year underflow saturates to `NegInf`.
fn pred(e: Ext) -> Ext {
    let Ext::Day(d) = e else { return e };
    if d.day > 1 {
        Ext::Day(BoundDate {
            day: d.day - 1,
            ..d
        })
    } else if d.month > 1 {
        let month = d.month - 1;
        Ext::Day(BoundDate {
            year: d.year,
            month,
            day: last_day(month, is_leap(d.year)),
        })
    } else {
        d.year.checked_sub(1).map_or(Ext::NegInf, |year| {
            Ext::Day(BoundDate {
                year,
                month: 12,
                day: 31,
            })
        })
    }
}

/// Can some completion of A start strictly before some completion of B and
/// end inside it (`a1 < b1 <= a2 < b2`)? Setting `p` to the shared boundary
/// day (A's end, B's start), feasibility is exactly
/// `∃p: lo_a < p <= hi_a ∧ lo_b <= p < hi_b`.
fn half_overlap(lo_a: Ext, hi_a: Ext, lo_b: Ext, hi_b: Ext) -> bool {
    succ(lo_a).max(lo_b) <= hi_a.min(pred(hi_b))
}

impl Edtf {
    /// The three-valued temporal relation between this expression and
    /// `other`, computed over the two [`Edtf::bounds`] regions (see the
    /// semantics note in this module's documentation).
    ///
    /// ```
    /// use edtf_core::{Edtf, Modality, Relation};
    ///
    /// let a = Edtf::parse("1985~").unwrap();
    /// let b = Edtf::parse("199X").unwrap();
    /// assert_eq!(a.relation(&b).definite(), Some(Relation::Before));
    ///
    /// let c = Edtf::parse("198X").unwrap();
    /// let d = Edtf::parse("1985").unwrap();
    /// // 198X may fall before, on, after or around 1985 — nothing asserted.
    /// assert_eq!(c.relation(&d).definite(), None);
    /// assert!(c.relation(&d).is_possible(Relation::Before));
    /// assert!(c.relation(&d).is_possible(Relation::After));
    /// assert!(c.relation(&d).is_possible(Relation::Contains));
    /// ```
    #[must_use]
    pub fn relation(&self, other: &Self) -> Relations {
        let a = self.bounds();
        let b = other.bounds();
        let (Some(a1), Some(a2), Some(b1), Some(b2)) = (
            ext(a.earliest),
            ext(a.latest),
            ext(b.earliest),
            ext(b.latest),
        ) else {
            return Relations::ALL_POSSIBLE;
        };
        let intersects = a1 <= b2 && b1 <= a2;
        Relations::from_possible([
            a1 < b2, // Before: some a ends before some b starts.
            b1 < a2, // After, mirrored.
            half_overlap(a1, a2, b1, b2) || half_overlap(b1, b2, a1, a2), // Overlaps.
            intersects && a1 < a2, // Contains: a day of B inside a wider A.
            intersects && b1 < b2, // Within, mirrored.
            intersects, // Equal: a shared day serves both.
        ])
    }
}
