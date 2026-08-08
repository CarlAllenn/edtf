# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

A spec-exact Rust implementation of EDTF (ISO 8601-2:2019 Annex A), levels 0–2
complete. One `#![no_std]`, zero-dependency core (`edtf-core`) compiled into
every deployment shape: a WebAssembly bundle (`edtf-wasm`), a Postgres
extension via pgrx (`edtf-postgres`), and a CLI (`edtf-cli`); plus
`edtf-normalize` (prose → EDTF, English + Russian tables) and
`edtf-calendars` (Julian → Gregorian) sitting on the same core.

The scanned ISO texts live in `spec/`. Every parsing judgement call is a
numbered D-decision in `docs/spec-notes.md`; every normaliser judgement call
is an N-decision in `docs/normalize-notes.md`. When behaviour is in question,
those documents are the authority — cite and extend them rather than deciding
ad hoc in code comments.

## Commands

Tooling is pinned by mise (`mise.toml`/`mise.lock`); tasks run via
[Task](Taskfile.yml). Git hooks (lefthook) run the full gate on every commit
and push — fixing tool output is never optional; there are no bypass flags.

- `task ci` — the full gate, identical to GitHub CI (lint + test)
- `task test` — `cargo test --all-features` (fingerprinted; `task --force` to override)
- `cargo test -p edtf-core parser::` — a single crate / test filter
- `task lint` — every linter (clippy, deny, taplo, yamllint, markdownlint,
  codespell, ec, actionlint+zizmor, shellcheck+shfmt, machete, fuzz-tree lint …)
- `task fmt` — apply all formatters; rustfmt uses a **pinned nightly**
  (`RUSTFMT_TOOLCHAIN` in Taskfile.yml; `task fmt:toolchain` installs it)
- `task semver` — cargo-semver-checks against the published crates.io baseline
- `task wasm:pack` — npm-ready wasm package

### Postgres extension

`edtf-postgres` is a workspace member but **not a default member**: bare
`cargo build`/`test`/`clippy` skip it (building it needs `cargo pgrx init`,
and its `pg14`–`pg18` features are mutually exclusive, so `--all-features`
across it is a hard error). Reach it explicitly:

- `task pg:lint` / `task pg:test` / `task pg:package`
- `task pg:schema-snapshot` — diff the generated SQL surface against the snapshot

`fuzz/` is likewise excluded from the workspace (nightly + libFuzzer);
`task lint:fuzz` is the only push-time gate that touches it.

## Architecture notes

- `edtf-core/src`: `parser.rs` (grammar), `types.rs` (model), `bounds.rs`
  (earliest/latest calendar days — the primitive for range queries),
  `relation.rs` (three-valued Allen relations), `enumerate.rs` (lazy value
  enumeration), `display.rs` (canonical form). All downstream crates construct
  values through this model, so outputs are valid by construction.
- `edtf-normalize` builds EDTF via `edtf-core` and re-parses its own output;
  ambiguous input (e.g. `12/04/1985`) returns every reading, never a guess.
- `tests/fixtures/legacy` pins the old edtf.js engine's verdicts as an
  oracle; where it disagrees with the Annex A reading, the spec wins and the
  divergence gets a note in `docs/spec-notes.md`.
- Lint canon (workspace `[lints]`, rustfmt config, hooks) is templated from
  the `CarlAllenn/renovate-config` repo — don't hand-tweak those blocks here;
  drift is audited.

## Conventions

- Spelling registers: **en-US in code and API identifiers** (`normalize`),
  **en-GB in prose/docs** (`normaliser`).
- One PR per issue; conventional commits enforced by hooks; commits are
  SSH-signed (a signed-commit ruleset is active on the repo).
- Branch from `origin/main`; `git add` explicit paths only, never `-A`.
- Releases go through release-plz → `publish.yml`; the runbook is
  `docs/release-runbook.md`. On a release branch, `task release:prepare`
  does the three things release-plz can't (extension upgrade SQL,
  `fuzz/Cargo.lock`, `CITATION.cff`).
