# edtf-core

A complete, spec-exact Rust implementation of **EDTF** — the Extended
Date/Time Format, standardized as the profile in **ISO 8601-2:2019
Annex A** — covering conformance **levels 0, 1 and 2 in full**.
`#![no_std]`, zero runtime dependencies.

- Parsing and validation with positioned errors — `1985-02-30` is rejected,
  `2000-02-29` is accepted and `1900-02-29` is not, `1985-02-3X` is rejected
  because no February day starts with 3.
- Level classification (minimum conformance level of an expression).
- Calendar bounds: every expression maps to earliest/latest calendar days
  (`156X` → 1560-01-01…1569-12-31), the primitive for range queries.
- Three-valued temporal relations under uncertainty — is `1985~` before
  `199X`? Definitely. Is `198X` before `1985`? Possibly.
- Canonical (spec-preferred) formatting.
- Optional `serde` feature for the model types.

```rust
use edtf_core::Edtf;

let d: Edtf = "1985-04-XX".parse()?;
assert_eq!(d.to_string(), "1985-04-XX");
# Ok::<(), edtf_core::Error>(())
```

This is the core of a small family — CLI (`edtf-cli`), WebAssembly/npm
(`edtf-wasm`), Postgres extension (`edtf-postgres`) and Julian-calendar
ingest conversion (`edtf-calendars`) — all compiled from this one
implementation, so a date that is valid in your application is valid in
your database. Full documentation, the spec-coverage notes and the whole
family live in the [repository](https://github.com/CarlAllenn/edtf).

## License

MIT OR Apache-2.0, at your option.
