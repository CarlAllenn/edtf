# CLAUDE.md

Guidance for Claude Code (claude.ai/code) working in this repository.

## What this is

A spec-exact Rust implementation of EDTF (ISO 8601-2:2019 Annex A), levels
0–2 complete. One `#![no_std]`, zero-dependency core (`edtf-core`) compiled
into every deployment shape: a WebAssembly bundle (`edtf-wasm`), a Postgres
extension via pgrx (`edtf-postgres`), and a CLI (`edtf-cli`); plus
`edtf-normalize` (prose → EDTF, English + Russian tables) and
`edtf-calendars` (Julian → Gregorian) sitting on the same core.

The scanned ISO texts live in `spec/` (never committed). Every parsing
judgement call is a numbered D-decision in `docs/spec-notes.md`; every
normaliser judgement call is an N-decision in `docs/normalize-notes.md`.
When behaviour is in question those documents are the authority — cite and
extend them rather than deciding ad hoc in a code comment.

## The org toolbelt owns the tooling

This repository is part of `monumental-archive` and conforms to it. The
linters, their configs and the CI gate are **not defined here**: they arrive
from the org toolbelt (`monumental-archive/.github`), pinned by SHA in
`.github/workflows/ci.yml`. Read that repository's CLAUDE.md and
`docs/migration-playbook.md` before changing anything under `.github/`.

Consequences worth knowing before you reach for a knob:

- `mise run ci` is the whole gate, and is exactly what CI runs — same tools,
  same versions, same order, from the same lockfile.
- There is **no repo-local clippy or rustfmt config**, deliberately.
  `lint:rust` refuses one: the knobs are org policy, delivered at run time.
  A setting this repo genuinely needs is argued into the canon.
- Suppressions are `#[expect(..., reason = "…")]` at the narrowest scope the
  reason is true at, never `#[allow]` and never a whole clippy group. An
  `#[expect]` errors the moment it stops being needed, which is the point.
- `mise.toml` holds only what is repo-shaped: the toolchain pins, the build
  inputs each release class needs, and this repo's own tasks.

## Commands

- `mise run ci` — the full gate (every `lint:*`, then `test`)
- `mise run test` — `cargo test --workspace --locked --all-features
  --exclude edtf-postgres`
- `mise run fix` — every write-mode fixer (`fix:rust`, `fix:markdown`, …)
- `cargo test -p edtf-core parser::` — a single crate or test filter
- `mise run coverage:check` — the committed ratchet in `.coverage-floor`
- `mise run bench` — the criterion benchmarks the README quotes
- `mise run wasm` / `mise run wasm:pack` — the wasm artifact, and the
  npm-ready package

### Postgres extension

`edtf-postgres` is a workspace member but **not a default member**: bare
`cargo build`/`test`/`clippy` skip it, because building it needs
`cargo pgrx init` and its `pg14`–`pg18` features are mutually exclusive, so
`--all-features` across it is a hard error. It is excluded from `test`,
`coverage:check` and `lint:rust` for that reason (`COVERAGE_EXCLUDE` and
`CLIPPY_EXCLUDE` in `mise.toml`), and its real coverage is the release: the
pgrx class builds and tests it in containers, per major, on every release.

- `mise run pg:lint` / `pg:test` / `pg:package`
- `mise run pg:schema-snapshot` — diff the generated SQL surface against
  `schema.snapshot.sql`
- `mise run lint:pg-upgrade-path` — every published extension version must
  still reach the current one by `ALTER EXTENSION UPDATE`. This one IS in
  the gate: it is a pure filename-graph walk, no pgrx and no database.

`fuzz/` is excluded from the workspace (nightly + libFuzzer). The gate's
`lint:fuzz-build` proves the targets still compile on stable; the Monday
`audit:fuzz` runs them under AddressSanitizer on the dated nightly declared
as `FUZZ_TOOLCHAIN`.

## Architecture notes

- `edtf-core/src`: `parser.rs` (grammar), `types.rs` (model), `bounds.rs`
  (earliest/latest calendar days — the primitive for range queries),
  `relation.rs` (three-valued Allen relations), `enumerate.rs` (lazy value
  enumeration), `display.rs` (canonical form). Every module is private and
  the crate exports named types only; downstream crates construct values
  through this model, so outputs are valid by construction.
- `edtf-normalize` builds EDTF via `edtf-core` and re-parses its own output;
  ambiguous input (e.g. `12/04/1985`) returns every reading, never a guess.
- `edtf-calendars` keeps the published Julian-Day-Number algorithms' own
  variable names and computes in `i128`, so any `i64` year is safe.
- `tests/fixtures/legacy` pins the old edtf.js engine's verdicts as an
  oracle; where it disagrees with the Annex A reading the spec wins, and the
  divergence gets a note in `docs/spec-notes.md`.

## Conventions

- Spelling registers: **en-US in code and API identifiers** (`normalize`),
  **en-GB in prose and docs** (`normaliser`).
- Conventional commits, imperative and lowercase, enforced by the org's
  `committed` config at commit-msg, pre-push and in CI. Every commit carries
  a DCO sign-off (`git commit -s`); the gate refuses one without.
- Branch from `origin/main`; PRs are squash-merged, so the PR title and body
  become the permanent commit.
- Releases go through the org's two-phase path: `release.yml` derives the
  version and opens a Release PR, and merging it mints the tag, which
  triggers `publish.yml`. The classes this repo publishes are declared on
  one line in `publish.yml`. Nothing here runs `cargo publish` by hand.
