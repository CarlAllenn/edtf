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
inside the pgrx harness across the pg14–pg18 matrix rather than under
`cargo test`, so the coverage job — which installs neither `cargo-pgrx` nor a
Postgres to run against — never executes them. Counting its source there would
report a false gap, not a real one.

**The exclusion is about where those tests run, not about whether they can be
measured.** This document previously claimed llvm-cov could not instrument the
pgrx harness. That is false, and was checked rather than inherited: with
pgrx 0.19.1, running

```bash
set -a; eval "$(cargo llvm-cov show-env --sh)"; set +a
cargo llvm-cov clean --workspace
cd crates/edtf-postgres && cargo pgrx test pg18
cargo llvm-cov report
```

the Postgres backend processes emit `.profraw` like any other instrumented
binary, and `edtf-postgres/src/lib.rs` reports **132 lines, 0 missed**. Folding
it into the published metric is therefore possible but not free: the coverage
job would need `cargo-pgrx` and a built Postgres (today it sets
`MISE_DISABLE_TOOLS: cargo:cargo-pgrx`), the `pg14`–`pg18` features are
mutually exclusive so it cannot share the `--all-features` invocation, and the
result would want validating on more than one major version. Tracked
separately; do not treat the current exclusion as evidence it is impossible.

`fuzz/` is outside the workspace and outside the metric — it is a corpus
generator, not a test suite.

## Running it

```bash
task coverage         # instrumented run; writes lcov.info
task coverage:table   # re-render the per-file table (reuses the run above)
task coverage:check   # the gate: no uncovered line, no untaken branch side
```

CI runs exactly these, so there is one recipe rather than a local one and a
drifting copy in the workflow. Two details the tasks encapsulate: `--branch`
is nightly-only, so the measurement uses the same pinned nightly as rustfmt
(`RUSTFMT_TOOLCHAIN` in `Taskfile.yml` — the repo carries one nightly, not
two); and cargo-llvm-cov has to be handed that `rustc` explicitly, because
under `rustup run` its own shim lands where `RUSTC` is expected and the build
fails, while mise's stable `rustc` otherwise shadows the rustup proxy.

## The two instruments

### 1. The per-file summary table

What `task coverage:table` prints and the job pipes into the GitHub step
summary. Convenient, and what most people mean by "the coverage number" — but
for this repository it **under-reports**; see the artifact below.

### 2. The union view (the lcov export)

```bash
grep -c '^DA:[0-9]*,0$' lcov.info    # uncovered lines
grep -c '^BRDA:.*,0$' lcov.info      # untaken branch sides
```

The per-line `DA:` and per-branch `BRDA:` records answer the question the word
"coverage" is actually asking: *was this line ever executed by the suite, by
any test, in any build of the crate?* This is the authoritative reading, it is
what `task coverage:check` gates on, and it is the artefact Codecov consumes.

Note that lcov's own `LF:`/`LH:` header counters do **not** agree with the
`DA:` records they summarise — they carry the table's number, not the union.
Read the records, not the counters.

## The instantiation artifact

A crate with **both** integration tests (`crates/<name>/tests/`) and inline
`#[cfg(test)]` unit tests is compiled twice: once as a plain rlib, which the
integration tests link, and once with `--cfg test` for the unit-test binary.
llvm-cov records those separately. `bounds.rs` shows it outright — two records
for one branch, each holding half the answer:

```text
Branch (125:13): [True: 2.19k, False: 0]   # plain rlib, via the integration tests
Branch (125:13): [True: 0,     False: 1]   # --cfg test build, via the guard unit test
```

What is directly observable: every one of `bounds.rs`'s 398 lines has a
non-zero merged execution count and every `DA:` record is non-zero, yet the
table reports 384/398 — and 384 is exactly what lcov's own `LH:` counter says.
The explanation that fits is that the per-file summary reports the best single
instantiation rather than the union, so a file reads 100% only when **one**
build covers it entirely.

That also predicts which files are affected, and it holds: the shortfall is
confined to the four largest `edtf-core` modules (`bounds`, `display`,
`enumerate`, `parser`). `edtf-cli` and `edtf-calendars` carry inline tests too
and still read 100%, because they are small enough for one build to cover
alone — so "has both kinds of test" is not the rule; "no single instantiation
covers the file" is.

There is no llvm-cov flag that changes it (`--show-instantiations` leaves the
numbers identical). The consequence is structural, so it is stated rather than
worked around:

> **Since issue #106, the union view has no uncovered line and no untaken
> branch side. The summary table will not read 100% for `edtf-core`'s four
> largest modules, and that shortfall is the artifact above, not a gap.**

The CI job gates on the union count and publishes it next to the table for
exactly this reason.

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
