# edtf

[![CI](https://github.com/CarlAllenn/edtf/actions/workflows/ci.yml/badge.svg)](https://github.com/CarlAllenn/edtf/actions/workflows/ci.yml)

A complete, spec-exact Rust implementation of **EDTF** — the Extended
Date/Time Format, standardized as the profile in **ISO 8601-2:2019 Annex A**
— covering conformance **levels 0, 1 and 2 in full**.

One zero-dependency core, compiled into every layer that needs it, so a date
that is valid in your application is valid in your database — always:

| Crate | What it is |
| --- | --- |
| **`edtf-core`** | The implementation. `#![no_std]`, zero runtime dependencies. Parsing, validation, level classification, calendar bounds, three-valued temporal relations, canonical formatting, positioned errors. Optional `serde` feature. |
| **`edtf-calendars`** | Proleptic Julian (Old Style) → Gregorian conversion at the ingest boundary: day precision converts exactly, year/month precision returns honest earliest/latest spans. `#![no_std]`, zero dependencies. |
| **`edtf-wasm`** | WebAssembly bindings for JavaScript (~61 KB): `isValid`, `level`, `canonical`, `parse` (JSON summary), `relation`. |
| **`edtf-postgres`** | Postgres extension (via [pgrx], Postgres 14–18): `edtf_valid()`, `edtf_level()`, `edtf_canonical()`, `edtf_min()`, `edtf_max()`, `edtf_relation()` as SQL functions. |
| **`edtf-cli`** | The `edtf` command-line tool: `validate` / `canonical` / `level` / `info` over arguments or stdin, plus `relation` (three-valued comparison of two expressions) and `from-julian` (Old Style → Gregorian EDTF). Installable anywhere via `cargo install edtf-cli` (or pin it with mise: `"cargo:edtf-cli"`). |

[pgrx]: https://github.com/pgcentralfoundation/pgrx

## What "complete" means here

- **Level 0**: calendar dates, reduced precision, date-times with UTC/shift,
  date intervals.
- **Level 1**: letter-prefixed (`Y…`) and negative years, seasons 21–24,
  whole-expression `?~%` qualification, the level 1 unspecified-digit shapes
  (`201X`, `2004-XX`, `1985-04-XX`, …), open (`..`) and unknown (empty)
  interval ends, qualified interval endpoints.
- **Level 2**: exponential years, significant digits (`S`), sub-year codes
  25–41, sets (`{…}`/`[…]`) with `..` ranges, group and individual
  qualification, unspecified digits anywhere, `..`-bounded interval endpoints.
- **Real calendar validation**: `1985-02-30` is rejected; `2000-02-29` is
  accepted and `1900-02-29` is not; `1985-02-3X` is rejected because no
  February day starts with 3; `X900-02-29` is rejected because no year ending
  in 900 is a leap year.
- **Bounds**: every expression maps to earliest/latest calendar days
  (`156X` → 1560-01-01…1569-12-31), the primitive for range queries.
- **Relations**: three-valued comparison under uncertainty — is `1985~`
  before `199X`? Definitely; is `198X` before `1985`? Possibly. Six
  coarsened Allen relations, each impossible / possible / definite, never
  over-asserting (semantics: D23 in the spec notes).
- **Canonical formatting**: `Display` renders the spec-preferred form
  (ISO 8601-2 §8.2.4) — `?2004-?06-?11` normalizes to `2004-06-11?`.
- **Strict profile boundaries**: everything Annex A excludes is rejected —
  basic format (`19850412`), week/ordinal dates, durations, and the entire
  explicit-form designator system are not EDTF and do not parse.

Every grammar production and every judgment call is documented with ISO
section citations in [docs/spec-notes.md](docs/spec-notes.md), including 23
resolved decisions (D1–D23) and one identified erratum in the ISO text
itself (Annex A.6.3 Example 2, which contradicts its own normative clause).

## Usage

```rust
use edtf_core::{Edtf, Bound};

assert!(edtf_core::is_valid("2004-06~-11"));      // level 2, June ~2004, day 11
assert!(!edtf_core::is_valid("1985-02-30"));      // no such day

let d = Edtf::parse("156X-12-25").unwrap();
assert_eq!(d.level(), 2);
assert!(d.has_unspecified());
let b = d.bounds();                                // 1560-12-25 ..= 1569-12-25

use edtf_core::Relation;
let a = Edtf::parse("1985~").unwrap();
assert_eq!(a.relation(&Edtf::parse("199X").unwrap()).definite(),
           Some(Relation::Before));                // ~ never moves bounds
```

SQL, after `CREATE EXTENSION edtf_postgres`:

```sql
SELECT edtf_valid('1985-24');                        -- true (winter 1985)
SELECT edtf_min('156X'), edtf_max('156X');           -- 1560-01-01, 1569-12-31
SELECT edtf_relation('1985~', '199X');               -- {definitely_before}
-- e.g. as a consistency rule: born must not be after died
-- CHECK (NOT ('definitely_after' = ANY(edtf_relation(born, died))))
SELECT daterange(edtf_min(production_date),
                 edtf_max(production_date), '[]') @> DATE '1965-06-15'
FROM artworks;
```

JavaScript, via the wasm package:

```js
import { isValid, parse } from "edtf-wasm";
isValid("2004-06~-11");            // true
JSON.parse(parse("1985-04-12/.."));// { kind: "interval", earliest: "1985-04-12", latest: "infinity", … }
```

## Development

Toolchain and every linter are pinned via [mise](https://mise.jdx.dev) with a
checksum lockfile; git hooks (lefthook) and CI run the same gauntlet:

```sh
mise install     # pinned Rust + all linters
task check       # fmt + clippy + cargo-deny + taplo + codespell + ec + tests
task wasm        # build the WebAssembly artifact
task pg:test     # Postgres extension tests (needs `cargo pgrx init` once)
```

The test suite includes a 63-case conformance corpus derived from every
Annex A example plus adversarial cases, every example from the Library of
Congress EDTF specification page as an interop cross-check
(`tests/fixtures/loc`), a second interop corpus harvested from the edtf.js
and python-edtf test suites (`tests/fixtures/interop` — shared verdicts
enforced, implementation extensions pinned as must-rejects, every
divergence a documented decision), ~200 spec-derived assertions, bounds
verification, a canonical-form round-trip property over all fixtures,
model-side property tests (proptest generates random valid values as
structured data and checks round-trip identity, level stability and bounds
ordering), and a deterministic fuzz harness (hundreds of thousands of
hostile inputs per run; the parser must never panic, and everything it
accepts must round-trip). On top of that, coverage-guided fuzzing
(`fuzz/`, cargo-fuzz) runs nightly in CI against the same never-panic and
round-trip properties.
Parse errors carry the byte offset of the problem:
`1985-02-30` → `invalid EDTF at offset 8: day is out of range for the month`.

### Performance

Criterion benchmarks live in `crates/edtf-core/benches/core.rs` (`task bench`).
Representative numbers, Apple M1 Pro, rustc 1.97.1, `--release`:

| Input | parse | canonicalize | bounds |
| --- | --- | --- | --- |
| `1985-04-12` | 187 ns | 283 ns | 190 ns |
| `1985-04-12T23:20:30+04:30` | 150 ns | 458 ns | 209 ns |
| `2004-06~` | 106 ns | 203 ns | 139 ns |
| `?2004-06-~11` | 192 ns | 295 ns | 199 ns |
| `156X-12-25` | 185 ns | 284 ns | 173 ns |
| `Y-17E7S3` | 74 ns | 178 ns | 15 ns |
| `{..1983-12-31,1984-10-10..1984-11-01,1984-11-05..}` | 1.25 µs | 1.20 µs | 795 ns |
| `1985-02-30` (rejected) | 189 ns | — | — |

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
