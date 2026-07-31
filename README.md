# edtf

[![CI](https://github.com/CarlAllenn/edtf/actions/workflows/ci.yml/badge.svg)](https://github.com/CarlAllenn/edtf/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/edtf-core.svg)](https://crates.io/crates/edtf-core)
[![docs.rs](https://img.shields.io/docsrs/edtf-core)](https://docs.rs/edtf-core)
[![npm](https://img.shields.io/npm/v/edtf-wasm.svg)](https://www.npmjs.com/package/edtf-wasm)
[![OpenSSF Scorecard](https://api.scorecard.dev/projects/github.com/CarlAllenn/edtf/badge)](https://scorecard.dev/viewer/?uri=github.com/CarlAllenn/edtf)
[![license](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE-MIT)

A complete, spec-exact Rust implementation of **EDTF** — the Extended
Date/Time Format, standardized as the profile in **ISO 8601-2:2019 Annex A**
— covering conformance **levels 0, 1 and 2 in full**.

One zero-dependency core, compiled into every layer that needs it, so a date
that is valid in your application is valid in your database — always:

| Crate | What it is |
| --- | --- |
| **`edtf-core`** | The implementation. `#![no_std]`, zero runtime dependencies. Parsing, validation, level classification, calendar bounds, three-valued temporal relations, value enumeration, canonical formatting, positioned errors. Optional `serde` feature. |
| **`edtf-normalize`** | Deterministic prose-date → EDTF normalizer at the human input boundary: `"1980s"` → `198X`, `"circa 1920"` → `1920~`, `"около 1920 г."` → `1920~`. Honest ambiguity (`"12/04/1985"` returns both readings, never a guess), every output valid canonical EDTF by construction. English and Russian pattern tables; `no_std` + `alloc`, no third-party dependencies (only `edtf-core`). |
| **`edtf-calendars`** | Proleptic Julian (Old Style) → Gregorian conversion at the ingest boundary: day precision converts exactly, year/month precision returns honest earliest/latest spans. `#![no_std]`, no third-party dependencies (only `edtf-core`). |
| **`edtf-wasm`** | WebAssembly bindings for JavaScript: `isValid`, `level`, `canonical`, `parse` (JSON summary), `relation`, and `normalize` (prose → EDTF via `edtf-normalize`). |
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
- **Value enumeration**: `values()` lazily yields the concrete calendar
  values an expression denotes — `{1667,1668,1670..1672}` expands exactly
  as ISO 8601-2 §6.4 does, `156X` yields its ten years, `1985-0X-31` its
  five valid months, `XXXX-XX-XX` streams ~3.65M days without allocating.
  Intervals and `..`-open set elements are honestly `Unenumerable`
  (semantics: D24–D29 in the spec notes).
- **Canonical formatting**: `Display` renders the spec-preferred form
  (ISO 8601-2 §8.2.4) — `?2004-?06-?11` normalizes to `2004-06-11?`.
- **Strict profile boundaries**: everything Annex A excludes is rejected —
  basic format (`19850412`), week/ordinal dates, durations, and the entire
  explicit-form designator system are not EDTF and do not parse.

Every grammar production and every judgment call is documented with ISO
section citations in [docs/spec-notes.md](docs/spec-notes.md), including 29
resolved decisions (D1–D29) and one identified erratum in the ISO text
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

### Installing the Postgres extension

Prebuilt, attested tarballs are attached to each `edtf-postgres-v*` release
from **v1.1.0** onward, so installing needs no Rust toolchain and no
`cargo-pgrx`. **Verify before extracting** — the archive unpacks into `/` as
root, so checking the signature afterwards is a postmortem, not verification.

```bash
VERSION=1.1.0
PG=18                 # 14, 15, 16, 17 or 18
ARCH=amd64            # amd64 or arm64 — dpkg's spelling, not uname's
FILE="edtf_postgres-${VERSION}-pg${PG}-linux-${ARCH}.tar.gz"

gh release download "edtf-postgres-v${VERSION}" --repo CarlAllenn/edtf \
  --pattern "${FILE}" --pattern SHA256SUMS

gh attestation verify "${FILE}" --repo CarlAllenn/edtf \
  --source-ref "refs/tags/v${VERSION}" \
  --signer-workflow CarlAllenn/edtf/.github/workflows/publish.yml

sha256sum --check --ignore-missing SHA256SUMS

sudo tar -xzf "${FILE}" -C /
```

Then, as any user with `CREATE` on the database — the extension is marked
`trusted`, so superuser is not required:

```sql
CREATE EXTENSION edtf_postgres;
```

Upgrading a previously installed copy: extract the new tarball over the old
one, then `ALTER EXTENSION edtf_postgres UPDATE;`. Extracting without
running the update leaves the new library registered under the old version.
If you are running `pg_upgrade`, install the extension into the **new**
cluster before upgrading.

#### Support matrix

| Postgres | `amd64` | `arm64` | glibc floor |
| --- | --- | --- | --- |
| 14, 15, 16, 17, 18 | ✅ | ✅ | 2.36 |

Built inside `postgres:<major>-bookworm` against that image's pgdg
`pg_config`, and installed-and-exercised on both bookworm and trixie before
release. The tested set and the shipped set are the same set.

glibc 2.36 is a floor, not a target: Debian 12, Debian 13 and Ubuntu 24.04
are all fine, Ubuntu 22.04 (2.35) is not. musl/Alpine is not covered — a
different libc entirely. The tarball assumes the Debian pgdg layout
(`/usr/lib/postgresql/<major>/lib`, `/usr/share/postgresql/<major>`); a
Postgres built from source under a different prefix needs the files placed
by hand from `pg_config --pkglibdir` and `--sharedir`.

A major is dropped from the matrix as it reaches end of life; Postgres 14
reaches EOL in November 2026.

JavaScript, via the wasm package:

```js
import { isValid, parse, normalize } from "edtf-wasm";
isValid("2004-06~-11");            // true
JSON.parse(parse("1985-04-12/.."));// { kind: "interval", earliest: "1985-04-12", latest: "infinity", … }
JSON.parse(normalize("circa 1920"));// { kind: "normalized", edtf: "1920~", … }
```

## Development

Toolchain and every linter are pinned via [mise](https://mise.jdx.dev) with a
checksum lockfile; git hooks (lefthook) and CI run the same gauntlet:

```sh
mise install     # pinned Rust + all linters
task ci          # fmt + clippy + cargo-deny + taplo + codespell + ec + tests
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
