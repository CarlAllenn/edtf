// SPDX-FileCopyrightText: Copyright (c) the edtf contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Postgres extension exposing [`edtf_core`] in SQL — the same validator the
//! application runs via WebAssembly, so the two layers can never diverge.
//!
//! SQL surface:
//! - `edtf_valid(text) → boolean`
//! - `edtf_level(text) → integer` (0/1/2, NULL if invalid)
//! - `edtf_canonical(text) → text` (spec-preferred form, NULL if invalid)
//! - `edtf_min(text) → date` / `edtf_max(text) → date` — earliest/latest
//!   calendar day. Open ends map to `-infinity`/`infinity`; unknown ends and
//!   years outside the Postgres date range map to NULL.
//! - `edtf_relation(text, text) → text[]` — three-valued temporal relation: one
//!   entry per non-impossible relation, `definitely_<r>` when it holds for
//!   every completion, `possibly_<r>` otherwise.

use edtf_core::{Bound, Edtf, Modality};
use pgrx::{datetime::Date, prelude::*};

::pgrx::pg_module_magic!(name, version);

/// True if the input is valid EDTF (ISO 8601-2:2019 Annex A, levels 0-2).
#[pg_extern(immutable, parallel_safe, strict)]
fn edtf_valid(input: &str) -> bool {
    edtf_core::is_valid(input)
}

/// Minimum EDTF conformance level (0, 1 or 2); NULL if invalid.
#[pg_extern(immutable, parallel_safe, strict)]
fn edtf_level(input: &str) -> Option<i32> {
    edtf_core::level(input).map(i32::from)
}

/// Canonical (spec-preferred) rendering; NULL if invalid.
#[pg_extern(immutable, parallel_safe, strict)]
fn edtf_canonical(input: &str) -> Option<String> {
    Some(Edtf::parse(input).ok()?.to_string())
}

/// Postgres `date` range is 4713-01-01 BC .. 5874897-12-31 AD. In the
/// astronomical numbering edtf-core uses (year 0 exists), 4713 BC is -4712.
fn to_pg_date(b: Bound) -> Option<Date> {
    match b {
        Bound::NegativeInfinity => Some(Date::negative_infinity()),
        Bound::PositiveInfinity => Some(Date::positive_infinity()),
        Bound::Unknown => None,
        Bound::Date(d) => {
            if d.year < -4712 || d.year > 5_874_897 {
                return None;
            }
            let year = i32::try_from(d.year).ok()?;
            Date::new(year, d.month, d.day).ok()
        },
    }
}

/// Earliest calendar day the expression touches; `-infinity` for open
/// starts; NULL when invalid, unknown, or before the Postgres date range.
#[pg_extern(immutable, parallel_safe, strict)]
fn edtf_min(input: &str) -> Option<Date> {
    to_pg_date(Edtf::parse(input).ok()?.bounds().earliest)
}

/// Latest calendar day the expression touches; `infinity` for open ends;
/// NULL when invalid, unknown, or beyond the Postgres date range.
#[pg_extern(immutable, parallel_safe, strict)]
fn edtf_max(input: &str) -> Option<Date> {
    to_pg_date(Edtf::parse(input).ok()?.bounds().latest)
}

/// Three-valued temporal relation between two EDTF expressions (semantics:
/// `docs/spec-notes.md` D23). One entry per relation that at least some
/// completion pair satisfies, in canonical order (before, after, overlaps,
/// contains, within, equal): `definitely_<r>` when every completion pair
/// satisfies it, else `possibly_<r>`. Unknown bounds yield all six as
/// `possibly_`. NULL if either input is invalid. A consistency rule reads:
/// `NOT ('definitely_after' = ANY(edtf_relation(born, died)))`.
#[pg_extern(immutable, parallel_safe, strict)]
fn edtf_relation(a: &str, b: &str) -> Option<Vec<String>> {
    let rel = Edtf::parse(a).ok()?.relation(&Edtf::parse(b).ok()?);
    Some(
        rel.possible()
            .map(|r| {
                let adverb = match rel.modality(r) {
                    Modality::Definite => "definitely",
                    _ => "possibly",
                };
                format!("{adverb}_{}", r.as_str())
            })
            .collect(),
    )
}

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test code: a panic here is the failure signal, not a crash path"
)]
mod tests {
    use pgrx::prelude::*;

    fn q_bool(sql: &str) -> bool {
        Spi::get_one(sql).expect("SPI").expect("non-null")
    }

    fn q_text(sql: &str) -> Option<String> {
        Spi::get_one(sql).expect("SPI")
    }

    #[pg_test]
    fn valid_and_invalid() {
        assert!(q_bool("SELECT edtf_valid('1985-04-12')"));
        assert!(q_bool("SELECT edtf_valid('2004-06~-11')"));
        assert!(q_bool("SELECT edtf_valid('{1667,1668,1670..1672}')"));
        assert!(!q_bool("SELECT edtf_valid('1985-02-30')"));
        assert!(!q_bool("SELECT edtf_valid('19850412')"));
        assert!(!q_bool("SELECT edtf_valid('2004/2003')"));
    }

    #[pg_test]
    fn levels() {
        assert_eq!(
            Spi::get_one::<i32>("SELECT edtf_level('1985-04-12')").unwrap(),
            Some(0)
        );
        assert_eq!(
            Spi::get_one::<i32>("SELECT edtf_level('1985~')").unwrap(),
            Some(1)
        );
        assert_eq!(
            Spi::get_one::<i32>("SELECT edtf_level('156X-12-25')").unwrap(),
            Some(2)
        );
        assert_eq!(
            Spi::get_one::<i32>("SELECT edtf_level('junk')").unwrap(),
            None
        );
    }

    #[pg_test]
    fn canonicalization() {
        assert_eq!(
            q_text("SELECT edtf_canonical('?2004-?06-?11')").as_deref(),
            Some("2004-06-11?")
        );
    }

    #[pg_test]
    fn bounds_as_dates() {
        assert_eq!(
            q_text("SELECT edtf_min('1985')::text").as_deref(),
            Some("1985-01-01")
        );
        assert_eq!(
            q_text("SELECT edtf_max('1985')::text").as_deref(),
            Some("1985-12-31")
        );
        assert_eq!(
            q_text("SELECT edtf_max('2003-24')::text").as_deref(),
            Some("2004-02-29") // northern winter wraps into leap year
        );
        assert_eq!(
            q_text("SELECT edtf_max('1985-04-12/..')::text").as_deref(),
            Some("infinity")
        );
        assert_eq!(
            q_text("SELECT edtf_min('../1985')::text").as_deref(),
            Some("-infinity")
        );
        // Unknown end and out-of-range years are NULL.
        assert_eq!(q_text("SELECT edtf_max('1986-04/')::text"), None);
        assert_eq!(q_text("SELECT edtf_min('Y17E7')::text"), None);
    }

    #[pg_test]
    fn relations() {
        assert!(q_bool(
            "SELECT edtf_relation('1985~', '199X') = ARRAY['definitely_before']"
        ));
        assert!(q_bool(
            "SELECT edtf_relation('198X', '1985') = ARRAY[\
                'possibly_before','possibly_after','possibly_overlaps',\
                'possibly_contains','possibly_within','possibly_equal']"
        ));
        // Unknown interval end: possible-everything, never definite.
        assert!(q_bool(
            "SELECT NOT ('definitely_after' = ANY(edtf_relation('1985/', '../1980')))"
        ));
        // The issue's consistency-rule shape: born must not be after died.
        assert!(q_bool(
            "SELECT NOT ('definitely_after' = ANY(edtf_relation('1890~', '1976-01-12')))"
        ));
        assert!(q_bool("SELECT edtf_relation('junk', '1985') IS NULL"));
    }

    #[pg_test]
    fn range_query_shape() {
        // The intended usage pattern: index-friendly range overlap.
        assert!(q_bool(concat!(
            "SELECT daterange(edtf_min('156X'), edtf_max('156X'), '[]') ",
            "@> DATE '1965-06-15' IS FALSE",
        )));
        assert!(q_bool(
            "SELECT daterange(edtf_min('196X'), edtf_max('196X'), '[]') @> DATE '1965-06-15'"
        ));
    }

    /// The shared conformance corpus (`tests/corpus.sql`), which the
    /// prebuilt-tarball smoke test also runs with `psql -f` against a real
    /// install. Running it from here too is what keeps the binary held to
    /// the same standard as the source: one set of assertions, two callers,
    /// nothing to drift. Each check is a plpgsql `ASSERT`, so a failure
    /// surfaces as an SPI error carrying the message.
    #[pg_test]
    fn shared_corpus() {
        Spi::run(include_str!("../tests/corpus.sql")).expect("shared corpus");
    }
}

/// Standard pgrx test harness plumbing.
#[cfg(test)]
pub mod pg_test {
    /// No per-test setup needed.
    pub fn setup(_options: Vec<&str>) {}

    /// No extra postgresql.conf settings needed.
    #[must_use]
    pub fn postgresql_conf_options() -> Vec<&'static str> {
        Vec::new()
    }
}
