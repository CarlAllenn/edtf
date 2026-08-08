// SPDX-FileCopyrightText: Copyright (c) the edtf contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Criterion benchmarks over representative inputs: parse, canonicalize
//! (Display) and bounds, per conformance level. Not a CI gate — run with
//! `task bench` (or `cargo bench -p edtf-core`) and publish the numbers in
//! the README on release.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test/bench code: a panic here is the failure signal, not a crash path"
)]

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use edtf_core::Edtf;

/// (label, input) pairs chosen to cover the grammar, not to flatter it:
/// level 0 calendar forms, level 1 qualification/masks, and the level 2
/// shapes (per-component qualifiers, exponential years, sets) that take the
/// deepest parser paths.
const CASES: &[(&str, &str)] = &[
    ("L0/date", "1985-04-12"),
    ("L0/year", "1985"),
    ("L0/datetime-shift", "1985-04-12T23:20:30+04:30"),
    ("L0/interval", "1964-06-02/2008-08-01"),
    ("L1/qualified", "2004-06~"),
    ("L1/masked", "201X"),
    ("L1/open-interval", "1985-04-12/.."),
    ("L2/component-qualified", "?2004-06-~11"),
    ("L2/masked-day", "156X-12-25"),
    ("L2/exp-year-sig", "Y-17E7S3"),
    (
        "L2/set-ranges",
        "{..1983-12-31,1984-10-10..1984-11-01,1984-11-05..}",
    ),
    ("L2/list", "[1667,1668,1670..1672]"),
];

/// Inputs the parser must reject; error paths are hot in validation
/// workloads (form input, bulk ingest), so they get measured too.
const INVALID: &[(&str, &str)] = &[
    ("calendar", "1985-02-30"),
    ("basic-format", "19850412"),
    ("truncated-set", "{1984-10-10..1984-11-01"),
];

#[allow(
    clippy::significant_drop_tightening,
    reason = "false positive: criterion's finish() consumes the group; the lint misreads the guard"
)]
fn bench_parse(c: &mut Criterion) {
    let mut g = c.benchmark_group("parse");
    for (label, input) in CASES {
        g.bench_function(*label, |b| b.iter(|| Edtf::parse(black_box(input))));
    }
    g.finish();

    let mut g = c.benchmark_group("reject");
    for (label, input) in INVALID {
        g.bench_function(*label, |b| b.iter(|| Edtf::parse(black_box(input))));
    }
    g.finish();
}

fn bench_canonicalize(c: &mut Criterion) {
    let mut g = c.benchmark_group("canonicalize");
    for (label, input) in CASES {
        let parsed = Edtf::parse(input).expect("bench case parses");
        g.bench_function(*label, |b| b.iter(|| black_box(&parsed).to_string()));
    }
    g.finish();
}

fn bench_bounds(c: &mut Criterion) {
    let mut g = c.benchmark_group("bounds");
    for (label, input) in CASES {
        let parsed = Edtf::parse(input).expect("bench case parses");
        g.bench_function(*label, |b| b.iter(|| black_box(&parsed).bounds()));
    }
    g.finish();
}

criterion_group!(benches, bench_parse, bench_canonicalize, bench_bounds);
criterion_main!(benches);
