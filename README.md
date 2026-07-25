# edtf

A complete, spec-exact Rust implementation of **EDTF** — the Extended
Date/Time Format, standardized as the profile in **ISO 8601-2:2019 Annex A**
— covering conformance **levels 0, 1 and 2 in full**.

One zero-dependency core, compiled into every layer that needs it, so a date
that is valid in your application is valid in your database — always:

| Crate | What it is |
|---|---|
| **`edtf-core`** | The implementation. `#![no_std]`, zero runtime dependencies. Parsing, validation, level classification, calendar bounds, canonical formatting. Optional `serde` feature. |
| **`edtf-wasm`** | WebAssembly bindings for JavaScript: `isValid`, `level`, `canonical`, `parse` (JSON summary). |
| **`edtf-postgres`** | Postgres extension (via [pgrx]): `edtf_valid()`, `edtf_level()`, `edtf_canonical()`, `edtf_min()`, `edtf_max()` as SQL functions. |

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
- **Canonical formatting**: `Display` renders the spec-preferred form
  (ISO 8601-2 §8.2.4) — `?2004-?06-?11` normalizes to `2004-06-11?`.
- **Strict profile boundaries**: everything Annex A excludes is rejected —
  basic format (`19850412`), week/ordinal dates, durations, and the entire
  explicit-form designator system are not EDTF and do not parse.

Every grammar production and every judgment call is documented with ISO
section citations in [docs/spec-notes.md](docs/spec-notes.md), including 19
resolved ambiguities (D1–D19) and one identified erratum in the ISO text
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
```

SQL, after `CREATE EXTENSION edtf_postgres`:

```sql
SELECT edtf_valid('1985-24');                        -- true (winter 1985)
SELECT edtf_min('156X'), edtf_max('156X');           -- 1560-01-01, 1569-12-31
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

```
mise install     # pinned Rust + all linters
task check       # fmt + clippy + cargo-deny + taplo + codespell + ec + tests
task wasm        # build the WebAssembly artifact
task pg:test     # Postgres extension tests (needs `cargo pgrx init` once)
```

The test suite includes a 63-case conformance corpus derived from every
Annex A example plus adversarial cases, ~200 spec-derived assertions, bounds
verification, and a canonical-form round-trip property over all fixtures.

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
