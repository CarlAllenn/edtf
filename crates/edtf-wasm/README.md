# edtf-wasm

WebAssembly bindings for [`edtf-core`](https://crates.io/crates/edtf-core)
and [`edtf-normalize`](https://crates.io/crates/edtf-normalize): EDTF
(ISO 8601-2:2019 Annex A) validation, canonicalization, level
classification, bounds, three-valued temporal relations, and deterministic
prose-date normalization ("circa 1920" → `1920~`) for JavaScript — the same
validator the Rust and Postgres layers run, so the stack can never diverge.

Published on npm as `edtf-wasm` (built with wasm-pack). Part of the
[`edtf`](https://github.com/monumental-archive/edtf) crate family.
