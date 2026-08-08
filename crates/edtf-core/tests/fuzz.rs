// SPDX-FileCopyrightText: Copyright (c) the edtf contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Deterministic fuzz tests: the parser must never panic on any input, and
//! every accepted input must survive the canonicalize-reparse round trip.
//!
//! Uses a fixed-seed xorshift generator so failures are reproducible; no
//! nightly toolchain or external fuzzer needed, so this guards every CI run.
//! Iteration counts scale up under `--release` (and via `EDTF_FUZZ_ITERS`).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test/bench code: a panic here is the failure signal, not a crash path"
)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "generator and oracle ranges are bounded by construction"
)]

use edtf_core::Edtf;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64* — plenty for fuzzing, fully deterministic.
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn iterations(default: usize) -> usize {
    if let Ok(v) = std::env::var("EDTF_FUZZ_ITERS") {
        if let Ok(n) = v.parse() {
            return n;
        }
    }
    if cfg!(debug_assertions) {
        default
    } else {
        default * 10
    }
}

/// Characters that actually appear in the EDTF grammar, so random inputs
/// exercise deep parser paths instead of failing on byte one.
const ALPHABET: &[u8] = b"0123456789XYESTZ?~%-+:./,{}[]";

fn check(input: &str) {
    // Any panic aborts the test; any Ok must round-trip semantically.
    if let Ok(parsed) = Edtf::parse(input) {
        // Bounds must be total too (found overflowing on Y9E18S1 by fuzzing).
        let _ = parsed.bounds();
        let rendered = parsed.to_string();
        let reparsed = Edtf::parse(&rendered).unwrap_or_else(|e| {
            panic!("accepted {input:?}, but canonical form {rendered:?} fails to reparse: {e}")
        });
        assert_eq!(
            parsed, reparsed,
            "round trip changed meaning for {input:?} (canonical {rendered:?})"
        );
    }
}

#[test]
fn random_grammar_soup_never_panics() {
    let mut rng = Rng(0x0EDF_0EDF_0EDF_0EDF);
    let mut buf = Vec::new();
    for _ in 0..iterations(200_000) {
        buf.clear();
        let len = rng.below(24) + 1;
        for _ in 0..len {
            buf.push(ALPHABET[rng.below(ALPHABET.len())]);
        }
        check(core::str::from_utf8(&buf).expect("alphabet is ASCII"));
    }
}

#[test]
fn random_bytes_never_panic() {
    let mut rng = Rng(0xBA_D5EE_DBAD_5EED);
    let mut buf = Vec::new();
    for _ in 0..iterations(50_000) {
        buf.clear();
        let len = rng.below(32);
        for _ in 0..len {
            buf.push((rng.next() & 0xFF) as u8);
        }
        if let Ok(s) = core::str::from_utf8(&buf) {
            check(s);
        }
    }
}

#[test]
fn mutated_valid_inputs_never_panic() {
    let seeds: &[&str] = &[
        "1985-04-12",
        "2004-06~-11",
        "?2004-06-~11",
        "Y3388E2S3",
        "156X-12-25",
        "1985-04-12T23:20:30+04:30",
        "..2004-06-01/2004-06-20..",
        "{..1983-12-31,1984-10-10..1984-11-01,1984-11-05..}",
        "[1667,1760-12]",
        "1985-04-12~/",
        "2001-24",
        "X*", // near-miss of explicit-form syntax
    ];
    let mut rng = Rng(0xC0FF_EE00_C0FF_EE00);
    for _ in 0..iterations(20_000) {
        let seed = seeds[rng.below(seeds.len())];
        let mut bytes = seed.as_bytes().to_vec();
        for _ in 0..=rng.below(3) {
            match rng.below(3) {
                0 if !bytes.is_empty() => {
                    // replace
                    let i = rng.below(bytes.len());
                    bytes[i] = ALPHABET[rng.below(ALPHABET.len())];
                },
                1 => {
                    // insert
                    let i = rng.below(bytes.len() + 1);
                    bytes.insert(i, ALPHABET[rng.below(ALPHABET.len())]);
                },
                _ if !bytes.is_empty() => {
                    // delete
                    let i = rng.below(bytes.len());
                    bytes.remove(i);
                },
                _ => {},
            }
        }
        if let Ok(s) = core::str::from_utf8(&bytes) {
            check(s);
        }
    }
}
