#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) the edtf contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
# Resolve the released tarball's SHA256 for this pg/arch cell (issue #82).
#
# The image is built from the RELEASE'S bytes, fetched by URL inside the
# Docker build — the stranger's path. The checksum BuildKit verifies against
# must therefore come from the release too: this downloads the published
# SHA256SUMS (the file the attestation covers) and extracts the one line
# that names this cell's tarball. Missing line fails loudly — an image must
# never build from a tarball the manifest does not vouch for.
set -euo pipefail

PG="${PG:?PG must be set}"
ARCH="${ARCH:?ARCH must be set}"
VERSION="${VERSION:?VERSION must be set}"
RUNNER_TEMP="${RUNNER_TEMP:?RUNNER_TEMP must be set}"
GITHUB_ENV="${GITHUB_ENV:?GITHUB_ENV must be set}"

TAG="edtf-postgres-v${VERSION}"

# Bounded: gh has no transfer deadline of its own (v1.2.0 hang lesson).
timeout 120 gh release download "${TAG}" --pattern SHA256SUMS \
  --dir "${RUNNER_TEMP}/sums" --clobber

name="edtf_postgres-${VERSION}-pg${PG}-linux-${ARCH}.tar.gz"

SHA=""
SHA=$(awk -v n="${name}" '$2 == n { print $1 }' "${RUNNER_TEMP}/sums/SHA256SUMS")

if [[ -z ${SHA} ]]; then
  echo "::error::${name} is not named by the released SHA256SUMS"
  exit 1
fi

echo "TARBALL_SHA256=${SHA}" >> "${GITHUB_ENV}"
echo "::notice::${name} pinned to ${SHA}"
