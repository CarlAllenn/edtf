#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) the edtf contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
# Prove the rustup-init pin in build-extension-inner.sh is internally
# consistent: the two per-architecture SHA256s must be the published
# checksums FOR the pinned version.
#
# Why this exists: RUSTUP_VERSION is kept current by the "rustup-init pin"
# custom manager in renovate.json, but Renovate can only bump the version
# line — it cannot compute the two SHA256 lines that must move with it. A
# version bump with stale checksums would sail through every static gate
# and fail at the worst possible moment: inside the release matrix, on the
# publish path. This check runs on every PR, so a Renovate bump goes red
# here — cheaply, before merge — until the checksums are updated to match,
# and a typo'd checksum is caught the same way.
#
# Fetching the published .sha256 files does NOT weaken the pin. The pin's
# security comes from the checked-in checksums freezing the exact binary
# the build may execute; this comparison only asserts that what is frozen
# is the genuine published value for the pinned version, using the same
# TLS-authenticated host the release build fetches the binary from.
set -euo pipefail

SCRIPT=".github/scripts/build-extension-inner.sh"

extract() {
  # $1: variable name pinned in the build script. Anchored and unique.
  local line=""
  line=$(grep -E "^${1}=" "${SCRIPT}")
  echo "${line#*=}"
}

VERSION=""
VERSION=$(extract RUSTUP_VERSION)
PINNED_AMD64=""
PINNED_AMD64=$(extract RUSTUP_SHA256_AMD64)
PINNED_ARM64=""
PINNED_ARM64=$(extract RUSTUP_SHA256_ARM64)

if [[ -z ${VERSION} || -z ${PINNED_AMD64} || -z ${PINNED_ARM64} ]]; then
  echo "::error::could not extract the rustup pins from ${SCRIPT}"
  exit 1
fi

fail=0
for pair in "x86_64-unknown-linux-gnu ${PINNED_AMD64}" \
  "aarch64-unknown-linux-gnu ${PINNED_ARM64}"; do
  target="${pair%% *}"
  pinned="${pair##* }"

  published=""
  published=$(curl --proto '=https' --tlsv1.2 -sSf \
    "https://static.rust-lang.org/rustup/archive/${VERSION}/${target}/rustup-init.sha256" \
    | cut -d' ' -f1)

  if [[ -z ${published} ]]; then
    echo "::error::no published checksum for rustup ${VERSION} on ${target}"
    fail=1
  elif [[ ${published} != "${pinned}" ]]; then
    echo "::error::rustup ${VERSION} ${target}: pinned ${pinned}, published ${published}"
    echo "::error::RUSTUP_VERSION moved without its SHA256 lines — update all three together"
    fail=1
  else
    echo "ok  rustup ${VERSION} ${target} checksum matches the published value"
  fi
done

exit "${fail}"
