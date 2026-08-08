#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) the edtf contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
# Consume the just-published artifacts the way a stranger would.
#
# Everything before this proves the bytes are what we built and signed. This
# proves they actually work: resolved from the registry, in a job that has
# built nothing, so nothing is satisfied from this run's own caches or the
# local workspace. A crate can upload perfectly and still be unusable — a
# missing file in the package, a path dependency that did not translate to a
# version requirement — and that failure only ever shows up on a fresh
# consumer. See the note on CARGO_HOME below for where that freshness now
# comes from.
set -euo pipefail

VERSION="${VERSION:?VERSION must be set}"

# Every probe here is a fresh network consumer, and none of them carries a
# deadline of its own — cargo and npm will wait on a stalled transfer
# indefinitely. The v1.2.0 release died in this script: ~40 minutes of
# silence, then the runner itself was lost and the logs with it. Two bounds
# fix that. CARGO_HTTP_TIMEOUT aborts a transfer that stops making progress
# (cargo's per-connection stall detector); the `timeout` wrappers cap each
# probe's total wall clock so no single hang can outlive the step. The caps
# are generous multiples of observed time (whole script: ~2m at v1.1.2) —
# they exist to convert a hang into a failure with logs, not to race the
# canary.
export CARGO_HTTP_TIMEOUT=60

# Timestamped phase markers, streamed. The v1.2.0 runner death also
# destroyed the archived step log, but lines already STREAMED to the
# Actions page survive — these heartbeats are what make a dead runner's
# last position knowable next time.
beat() {
  local ts
  ts=$(date -u +%H:%M:%S) || ts="--:--:--"
  echo "[${ts}Z] $*"
}

scratch=$(mktemp -d)
trap 'rm -rf "${scratch}"' EXIT

# NO scratch CARGO_HOME. There used to be one here, to force a real
# registry fetch rather than reusing anything this job already had — a
# correct goal, for a shape that no longer exists.
#
# It dated from when this script ran inside the publish job, which had just
# packaged and published these very crates and therefore had them warm.
# Since #130 the canary runs in `finish`, a job whose entire history is
# checkout, mise, and an artifact download: it never builds, so its cargo
# home has no edtf crate in it. The job boundary supplies the coldness the
# repoint was buying.
#
# And the repoint was not merely redundant, it was the bug. mise installs
# Rust through rustup and provisions the toolchain AGAINST CARGO_HOME
# (mise.jdx.dev/lang/rust.html); pointing that variable at an empty
# directory and then invoking the toolchain stalls the first cargo call
# indefinitely. That is what killed v1.2.0 through v1.2.2 — five silent
# deaths, no output, because the stall lands before this script's first
# heartbeat. Reproduced in edtf-release-lab (rustup-repro): empty
# CARGO_HOME stalls until the bound fires, the real one passes in 25s,
# and RUSTUP_* being set or unset makes no difference.
#
# If the publish and finish jobs are ever merged back into one, the
# coldness guarantee goes with the split and this needs rethinking — but
# not by repointing CARGO_HOME.

# The scaffold is the first cargo invocation in the job, and until v1.2.2
# it was the one call here with neither a heartbeat before it nor a bound
# around it — so a stall produced a step that ran to the job timeout having
# printed nothing at all. Four v1.2.x releases died in this step with an
# empty log, and the absence of any heartbeat was read as "the script never
# started" when it is equally consistent with "the script stopped here".
# It is local work and should take milliseconds (0.03s at v1.1.2), but the
# whole point of the bounds is that the cheap calls get them too.
beat "probe 0/4: scaffolding the consumer crate"
timeout 60 cargo new --lib "${scratch}/probe" > /dev/null

# Library crates: resolve and compile against the published versions.
# Resolution alone is retried: the sparse index can lag a just-published
# version by a few seconds, and that lag is the registry's to spend, not a
# defect in the release. A compile failure is never retried — it would fail
# identically every time and the retry would only blur the report.
resolved=0
for attempt in 1 2 3; do
  beat "probe 1/4: resolving library crates from crates.io (attempt ${attempt})"
  if (
    cd "${scratch}/probe"
    timeout 300 cargo add "edtf-core@=${VERSION}" > /dev/null
    timeout 300 cargo add "edtf-calendars@=${VERSION}" > /dev/null
    timeout 300 cargo add "edtf-normalize@=${VERSION}" > /dev/null
  ); then
    resolved=1
    break
  fi
  echo "resolution attempt ${attempt} failed; index may be lagging, retrying in 30s"
  sleep 30
done
if [[ ${resolved} -ne 1 ]]; then
  echo "::error::library crates did not resolve after 3 attempts"
  exit 1
fi
beat "probe 2/4: compiling against the published library crates"
(
  cd "${scratch}/probe"
  timeout 600 cargo build --quiet
)
echo "ok  edtf-core, edtf-calendars, edtf-normalize resolve and compile"

# Binary crate: the [[bin]] is named `edtf`, not `edtf-cli`.
beat "probe 3/4: cargo install edtf-cli from crates.io"
timeout 900 cargo install "edtf-cli@${VERSION}" --root "${scratch}/cli" --quiet
"${scratch}/cli/bin/edtf" --version
echo "ok  edtf-cli installs and runs"

# npm package: fetch the published tarball as a consumer would.
beat "probe 4/4: npm pack edtf-wasm from the npm registry"
timeout 300 npm pack "edtf-wasm@${VERSION}" --pack-destination "${scratch}" > /dev/null
echo "ok  edtf-wasm fetches from npm"

# edtf-postgres is not exercised here, but it is no longer unexercised.
# Consuming it needs a running Postgres, which this script has no business
# starting, so its canary is a separate step: canary-extension.sh downloads
# the tarball attached to the release, checks it against SHA256SUMS and
# installs it into a clean Postgres. That runs after the assets are
# attached, which is necessarily later than this script (issue #55).

echo "::notice::canary passed — published artifacts consume cleanly"
