# Coverage — what is measured, with which instrument, and why

Coverage here is **visibility, not a gate**. There is no threshold to fail
(issue #8): the number is published on every PR and a drop is read by a human.
This document exists because the number needs a reading, and because two
instruments that both claim to measure "coverage" disagree about this
repository by roughly half a percent. Neither is lying; they count different
things, and knowing which is which is the difference between an honest 100%
and a fudged one.

## What is measured

The workspace **minus `edtf-postgres`**
(`--ignore-filename-regex 'crates/edtf-postgres/'`). The extension's tests run
inside the pgrx harness across the pg14–pg18 matrix, which llvm-cov cannot
instrument; counting its source would report a false gap, not a real one.

`fuzz/` is outside the workspace and outside the metric — it is a corpus
generator, not a test suite.

## The two instruments

### 1. The per-file summary table (lines; stable toolchain; CI)

```bash
cargo llvm-cov --all-features --ignore-filename-regex 'crates/edtf-postgres/'
```

This is what the `coverage` job pipes into the GitHub step summary. It is
convenient and it is what most people mean by "the coverage number", but for
this repository it **under-reports** — see the artifact below.

### 2. The union view (lines and branch sides; the lcov export)

```bash
cargo llvm-cov report --lcov --output-path lcov.info \
  --ignore-filename-regex 'crates/edtf-postgres/'
grep -c '^DA:[0-9]*,0$' lcov.info    # uncovered lines
grep -c '^BRDA:.*,0$' lcov.info      # untaken branch sides
```

The per-line `DA:` and per-branch `BRDA:` records answer the question the word
"coverage" is actually asking: *was this line ever executed by the suite, by
any test, in any build of the crate?* This is the authoritative reading, and
it is the artefact Codecov consumes.

Branch sides need `--branch`, which is nightly-gated, so CI (stable) reports
lines only. Locally, mise's stable `rustc` shadows the rustup proxy, so the
pinned nightly has to be put on `PATH` explicitly — the toolchain pin is
`RUSTFMT_TOOLCHAIN` in `Taskfile.yml`:

```bash
NIGHTLY="$HOME/.rustup/toolchains/nightly-2026-07-20-$(rustc -vV | sed -n 's/host: //p')/bin"
RUSTC="$NIGHTLY/rustc" PATH="$NIGHTLY:$PATH" \
  cargo llvm-cov --all-features --branch \
  --ignore-filename-regex 'crates/edtf-postgres/' \
  --lcov --output-path lcov.info
```

## The instantiation artifact

A crate that has **both** integration tests (`crates/<name>/tests/`) and inline
`#[cfg(test)]` unit tests is compiled twice: once as a plain rlib, which the
integration tests link, and once with `--cfg test` for the unit-test binary.
llvm-cov keeps those as separate instantiations and its per-file summary does
**not** union them — so a private guard reachable only from a unit test is
counted as missed in the table even though the merged view has it covered.

`bounds.rs` shows the shape outright. Two records, one branch:

```text
Branch (125:13): [True: 2.19k, False: 0]   # plain rlib, via the integration tests
Branch (125:13): [True: 0,     False: 1]   # --cfg test build, via the guard unit test
```

Only crates with both kinds of test split this way, which is why
`edtf-normalize` (integration tests only) and `edtf-wasm` (unit tests only)
still read 100% in the table while `edtf-core` does not.

There is no llvm-cov flag that changes the merge. The consequence is
structural, so it is stated rather than worked around:

> **Since issue #106, the union view has no uncovered line and no untaken
> branch side. The summary table will not read 100% for `edtf-core`,
> `edtf-cli`, or `edtf-calendars`, and that shortfall is the artifact above,
> not a gap.**

The CI job publishes the union count next to the table for exactly this
reason.

## The standing rule for defensive guards

`edtf-core` is total by construction: every field of the model is public and
there are no validating constructors, so a downstream user can hand-build a
value the parser would never produce — a `Big` year carrying a month, a month
field valued 21, a February 30. The library answers those gracefully
(`Bound::Unknown`, an empty candidate list, an `Err`) or panics loudly where
answering at all would mean guessing. Both are contracts, and both are pinned
by white-box tests in `#[cfg(test)]` modules beside the guard, with
`#[should_panic(expected = "…")]` naming the arm's real message.

Two things are **not** how this repository reaches 100%:

- `#[coverage(off)]` — hides the number instead of earning it, and its
  function-level granularity forces contortions.
- Deleting a guard because it is hard to reach. The fail-closed and totality
  behaviour is the point; the coverage is downstream of it.

If a guard is genuinely unreachable through any call — the invariant holds
locally, not just by caller discipline — the fix is a minimal,
behaviour-preserving extraction so the check can be called directly
(`check_plain_month` in `parser.rs` is the worked example), never a deletion.
