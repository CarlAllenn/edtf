#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) the edtf contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
# Publish all six crates to crates.io, resumably.
#
# Per-crate guard, not all-or-nothing. `cargo publish --workspace` resolves
# ordering itself, but it is not resumable: if a release dies after three of
# the five workspace crates, re-running it fails on the three already
# published and the release stays stuck. That is exactly the situation
# v1.0.0 was in, so the resume path has to actually work (issue #54).
#
# Order is explicit rather than delegated:
#   edtf-core -> edtf-calendars, edtf-normalize -> edtf-wasm, edtf-cli
# (verified against each manifest's [dependencies]). edtf-postgres is last
# and separate — its own workspace, resolving edtf-core from the registry.
set -euo pipefail

VERSION="${VERSION:?VERSION must be set}"

# Dependency order. edtf-postgres is deliberately NOT in this list: it is
# published below with its own manifest path and flags.
ORDERED_CRATES=(edtf-core edtf-calendars edtf-normalize edtf-wasm edtf-cli)
ALL_CRATES=("${ORDERED_CRATES[@]}" edtf-postgres)

# Sets PUBLISHED to "yes" or "no" for $1.
#
# Invoked as a plain statement and read afterwards, never used directly as a
# condition: under `set -e`, shellcheck (.shellcheckrc enable=all, SC2310)
# rejects functions used as predicates because the flag is suspended inside
# them — which would silently swallow a real curl failure and read it as
# "not published", causing a double-publish attempt.
PUBLISHED=""
check_published() {
  local name="$1"
  local body=""
  body=$(curl -sfA 'edtf-release-workflow' --retry 3 --retry-delay 5 \
    "https://crates.io/api/v1/crates/${name}/${VERSION}" 2> /dev/null) || body=""
  if [[ ${body} == *'"num"'* ]]; then
    PUBLISHED="yes"
  else
    PUBLISHED="no"
  fi
}

for name in "${ORDERED_CRATES[@]}"; do
  check_published "${name}"
  if [[ ${PUBLISHED} == "yes" ]]; then
    echo "::notice::${name} ${VERSION} already published; skipping"
    continue
  fi
  echo "publishing ${name} ${VERSION}"
  cargo publish -p "${name}"
done

# edtf-postgres last: it resolves edtf-core from the registry, so the five
# above must be live first. --no-verify because the verify build runs pgrx's
# build script, which needs an initialized $PGRX_HOME this runner does not
# have; ci.yml's postgres job runs it on runners that do.
check_published edtf-postgres
if [[ ${PUBLISHED} == "yes" ]]; then
  echo "::notice::edtf-postgres ${VERSION} already published; skipping"
else
  echo "publishing edtf-postgres ${VERSION}"
  cargo publish -p edtf-postgres --no-verify
fi

# All six must now exist, whether this run published them or a prior one
# did. Anything less means the release is incomplete and must not reach
# attestation.
missing=()
for name in "${ALL_CRATES[@]}"; do
  check_published "${name}"
  if [[ ${PUBLISHED} != "yes" ]]; then
    missing+=("${name}")
  fi
done

if [[ ${#missing[@]} -gt 0 ]]; then
  echo "::error::not on crates.io after publish: ${missing[*]}"
  exit 1
fi

echo "::notice::all six crates present on crates.io at ${VERSION}"
