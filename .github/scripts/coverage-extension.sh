#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) the edtf contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
# Line + branch coverage for the Postgres extension, measured where its tests
# actually run.
#
# edtf-postgres used to sit outside the coverage metric because the coverage
# job installs neither cargo-pgrx nor a Postgres to run against. Moving the
# measurement to the job that already has both is what folds it back in
# (issue #153): every test job emits a coverage fragment, and one job merges
# and publishes. The alternative — teaching the coverage job to build a
# Postgres — would have cost it `disable-sudo: true` and paid for pgrx twice.
#
# THE INSTRUMENTED RUN IS THE ONLY RUN. This replaces the plain
# `cargo pgrx test` rather than following it: instrumentation changes speed,
# not semantics, and running the matrix's most expensive step twice on five
# legs buys very little.
#
# Requires an initialised $PGRX_HOME, so it runs in ci.yml's postgres job and
# locally via `task coverage:pg` — which is also what supplies $NIGHTLY, so
# the pinned toolchain is written down once (Taskfile.yml).
set -euo pipefail

PG="${PG:?PG must be set (e.g. 18)}"
NIGHTLY="${NIGHTLY:?NIGHTLY must be set (RUSTFMT_TOOLCHAIN in Taskfile.yml)}"

CRATE_DIR="crates/edtf-postgres"
# Fixed names, not knobs: .gitignore, the artifact upload and
# `task coverage:check`'s lcov*.info glob all name these.
LCOV="lcov-postgres.info"
TABLE="coverage-postgres.txt"

# Same dance as `task coverage`: `--branch` is nightly-only, and cargo-llvm-cov
# has to be handed that rustc directly — under `rustup run` its own shim lands
# where RUSTC is expected and the build fails, while mise's stable rustc
# otherwise shadows the rustup proxy.
bin="$(rustup run "${NIGHTLY}" rustc --print sysroot)/bin"
export RUSTC="${bin}/rustc"
export PATH="${bin}:${PATH}"

# `cargo pgrx test` is not `cargo test`, so it cannot be run under
# `cargo llvm-cov -- ...`; show-env exports the same instrumentation into the
# environment instead, and pgrx's own build inherits it. The Postgres backend
# processes then emit .profraw like any other instrumented binary.
#
# Captured before the eval, not inside it: a failed show-env inside `$( )`
# would have its exit status swallowed and this would run uninstrumented.
cov_env="$(cargo llvm-cov show-env --sh)"
set -a
eval "${cov_env}"
set +a

# show-env has no `--branch` equivalent, so the flag goes on by hand. The
# wrapper reads its RUSTFLAGS as a 0x1f-separated list, not a shell string.
branch_flag="$(printf '\037')-Zcoverage-options=branch"
export __CARGO_LLVM_COV_RUSTC_WRAPPER_RUSTFLAGS="${__CARGO_LLVM_COV_RUSTC_WRAPPER_RUSTFLAGS}${branch_flag}"

# Stale .profraw from an earlier `task coverage` in the same tree would be
# merged into this report and silently overstate it.
cargo llvm-cov clean --workspace

(
  cd "${CRATE_DIR}"
  cargo pgrx test "pg${PG}" --no-default-features --features "pg${PG}"
)

# edtf-core is instrumented too (the extension calls into it), and its lines
# as exercised by *these* tests are not its coverage — the workspace run
# measures that. Leaving them in would put a second, far emptier edtf-core
# section into the merged lcov, and the union gate reads every section.
REPORT_ARGS=(report --branch --ignore-filename-regex 'crates/edtf-core/')

cargo llvm-cov "${REPORT_ARGS[@]}" --lcov --output-path "${LCOV}"
cargo llvm-cov "${REPORT_ARGS[@]}" | tee "${TABLE}"

# An empty fragment satisfies every count below, so it is ruled out first:
# "no uncovered lines" and "no lines" are the same number.
if ! grep -q '^SF:' "${LCOV}"; then
  echo "::error::no coverage records for ${CRATE_DIR} — the tests ran uninstrumented"
  exit 1
fi

# The regex above enumerates what to drop, so a new workspace dependency of
# this crate would leak into the fragment. That fails the union gate rather
# than passing quietly — but it fails it somewhere unhelpful, so name it here.
if grep '^SF:' "${LCOV}" | grep -qv "${CRATE_DIR}/"; then
  echo "::error::the fragment carries files outside ${CRATE_DIR}:"
  grep '^SF:' "${LCOV}" | grep -v "${CRATE_DIR}/"
  echo "::error::extend the --ignore-filename-regex in $0"
  exit 1
fi

# The same binary invariant `task coverage:check` applies to the rest of the
# workspace, applied here per pg major. pg18 exports the fragment that the
# coverage job merges; the other four legs assert on their own copy, which is
# what keeps "it holds on more than pg18" a standing CI property rather than a
# one-off measurement (issue #153).
lines=$(grep -c '^DA:[0-9]*,0$' "${LCOV}" || true)
sides=$(grep -c '^BRDA:.*,0$' "${LCOV}" || true)
echo "pg${PG} — uncovered lines: ${lines}; untaken branch sides: ${sides}"
test "${lines}" -eq 0 && test "${sides}" -eq 0
