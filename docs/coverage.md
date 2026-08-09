# Coverage — what is measured, with which instrument, and why

Coverage here is **visibility, not a gate**. There is no threshold to fail
(issue #8): the number is published on every PR and a drop is read by a human.
This document exists because the number needs a reading, and because two
instruments that both claim to measure "coverage" disagree about this
repository by roughly half a percent. Neither is lying; they count different
things, and knowing which is which is the difference between an honest 100%
and a fudged one.

## What is measured

**The whole workspace**, in two pieces, because it is built and tested in two
harnesses.

| piece | harness | measured by | written to |
| --- | --- | --- | --- |
| everything but `edtf-postgres` | `cargo test` | `task coverage` | `lcov.info` |
| `edtf-postgres` | `cargo pgrx test`, pg14–pg18 | `task coverage:pg` | `lcov-postgres.info` |

`fuzz/` is outside the workspace and outside the metric — it is a corpus
generator, not a test suite.

The split is a fact about where the tests run, not a carve-out.
`cargo llvm-cov` over the workspace never executes the extension's tests —
they need a Postgres to run against — so its source is excluded from *that*
invocation (`--ignore-filename-regex 'crates/edtf-postgres/'`) and measured by
the second one instead. Both files are then read together: `task coverage:check`
gates on every `lcov*.info`, and the coverage job hands Codecov both.

### Why it is measured in the matrix job

Until issue #153 the extension was outside the number entirely, on the stated
grounds that llvm-cov **cannot instrument** the pgrx harness. That was false,
and testing it rather than inheriting it is what closed the issue: the Postgres
backend processes emit `.profraw` like any other instrumented binary (66 files
from a single `pg18` run). What was true is narrower — the coverage job has no
Postgres and no `cargo-pgrx`.

So the measurement moved to the job that has both, rather than the job growing
a Postgres. That is also the general shape worth keeping: **every test job
emits a coverage fragment; one job merges and publishes.** Teaching the
coverage job to build a Postgres would have cost it `disable-sudo: true`, added
the postgresql.org egress hosts, and paid for the pgrx build a second time —
to measure code another job had already tested.

Consequences, all deliberate:

- The coverage job `needs: postgres`, so the number lands after the matrix and
  a red leg means no coverage report. A matrix that did not finish has not
  measured anything.
- All five majors run instrumented and each asserts its own union is clean, so
  "it holds on more than pg18" is a standing property, not a one-off check.
  Only pg18 exports the fragment — it is the default feature, the same reason
  it is the leg that governs the SQL snapshot. Concatenating all five would
  make a line pg14 covers and pg18 does not read as *uncovered*, since the gate
  reads every section it is given.
- The instrumented run is the only run: it replaces `cargo pgrx test` rather
  than following it. Instrumentation changes speed, not semantics.

### What folding it in cost

One test. The extension was at 132 lines / 0 missed, which is what the issue
had measured — but at **3 of 4 branch sides**. `to_pg_date`'s range guard
(`d.year < -4712 || d.year > 5_874_897`) was exercised at the upper end only,
by `edtf_min('Y17E7')`; nothing in the suite reached a year before 4713 BC. So
the documented "outside the Postgres date range → NULL" contract was pinned at
one end and assumed at the other, and a lines-only reading could not see it.
`edtf_min('Y-17E7')` is the mirror, added to both callers of the shared corpus.

## Running it

```bash
task coverage         # instrumented run; writes lcov.info
task coverage:pg      # the same for the extension; needs `cargo pgrx init`
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

`task coverage:pg` adds a third: `cargo pgrx test` is not `cargo test`, so it
cannot run *under* `cargo llvm-cov`. `cargo llvm-cov show-env` exports the
instrumentation into the environment and pgrx's own build inherits it — with
`-Zcoverage-options=branch` appended by hand, because `show-env` has no
`--branch` equivalent and the wrapper reads its flags as a 0x1f-separated list.
`.github/scripts/coverage-extension.sh` is that recipe.

A laptop without an initialised `$PGRX_HOME` can still run `task coverage` and
`task coverage:check`; the gate then reads the workspace file alone. CI cannot
fall into that half-check quietly — there the fragment is a downloaded
artifact, and a missing artifact fails the download step first.

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
