# edtf-postgres

Postgres extension (pgrx) exposing
[`edtf-core`](https://crates.io/crates/edtf-core) in SQL — the same
validator the application runs via WebAssembly, so the two layers can never
diverge.

SQL surface: `edtf_valid(text)`, `edtf_level(text)`,
`edtf_canonical(text)`, `edtf_min(text)` / `edtf_max(text)` (index-friendly
date bounds), and `edtf_relation(text, text)` (three-valued temporal
relations).

Its contract is the SQL surface, not a Rust API. Part of the
[`edtf`](https://github.com/CarlAllenn/edtf) crate family.
