#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) the edtf contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
# Consume the just-published artifacts the way a stranger would.
#
# Everything before this proves the bytes are what we built and signed. This
# proves they actually work: resolved from the registry, in a scratch
# CARGO_HOME so nothing is satisfied from this run's own caches or the local
# workspace. A crate can upload perfectly and still be unusable — a missing
# file in the package, a path dependency that did not translate to a version
# requirement — and that failure only ever shows up on a fresh consumer.
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

# Scratch CARGO_HOME: forces a real registry fetch rather than reusing
# anything this job already has.
export CARGO_HOME="${scratch}/cargo"

# Library crates: resolve and compile against the published versions.
# Resolution alone is retried: the sparse index can lag a just-published
# version by a few seconds, and that lag is the registry's to spend, not a
# defect in the release. A compile failure is never retried — it would fail
# identically every time and the retry would only blur the report.
cargo new --lib "${scratch}/probe" > /dev/null
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
