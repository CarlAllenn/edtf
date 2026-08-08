#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) the edtf contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
# Consume the artifacts the way a stranger will: from the release assets.
#
# canary.sh proves the crates and the npm package resolve and run when
# fetched from their registries. It deliberately skipped edtf-postgres,
# because consuming it needed a running Postgres and a pgrx toolchain —
# deferred to issue #55, which is this.
#
# The gap this closes is narrow but real. The matrix jobs smoke-tested the
# tarballs they built, in memory of their own filesystem. This takes the
# bytes that are actually ATTACHED TO THE RELEASES, proves they are the
# same bytes SHA256SUMS names, and installs one. A truncated upload, a
# mis-attached file, or a checksums file describing something other than
# what shipped all survive every earlier check and die here.
#
# EVERY asset is byte-verified, ONE is executed (issue #83, gap 4). The
# unproven step at this point is the upload, and upload integrity is a
# byte-level property — so every tarball on both releases (every pg major,
# both architectures, the dbgsym siblings, all four CLI targets) is
# re-downloaded and held against the released SHA256SUMS, and that manifest
# against the one this run built. Execution is a different property, and it
# was already proven per-cell on native hardware before anything was
# uploaded; byte-identity extends that proof to the published copies,
# including the arm64 legs this amd64 runner cannot execute natively. The
# one native install below re-proves the full end-to-end stranger's path.
set -euo pipefail

VERSION="${VERSION:?VERSION must be set}"
GITHUB_REPOSITORY="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY must be set}"

# The default feature, and the architecture this runner can smoke-test
# natively.
PG=18
ARCH=amd64
ASSET="edtf_postgres-${VERSION}-pg${PG}-linux-${ARCH}.tar.gz"
EXT_TAG="edtf-postgres-v${VERSION}"
CLI_TAG="edtf-cli-v${VERSION}"

scratch=$(mktemp -d)
trap 'rm -rf "${scratch}"' EXIT

# The releases are still drafts — that is why immutability works at all —
# so these are authenticated fetches rather than anonymous ones. What is
# being proved is the integrity of the attached bytes, not the anonymity of
# the fetch.
gh release download "${EXT_TAG}" --repo "${GITHUB_REPOSITORY}" \
  --pattern 'edtf_postgres-*.tar.gz' --pattern SHA256SUMS --dir "${scratch}"
gh release download "${CLI_TAG}" --repo "${GITHUB_REPOSITORY}" \
  --pattern 'edtf-cli-*.tar.gz' --dir "${scratch}"

if [[ ! -f "${scratch}/SHA256SUMS" ]]; then
  echo "::error::SHA256SUMS could not be downloaded from ${EXT_TAG}"
  exit 1
fi

# The released manifest must be the one this run built, or the release
# describes artifacts from some other run. upload-extension-assets.sh
# overwrites while the release is a draft precisely so these agree.
if ! diff -u dist/SHA256SUMS "${scratch}/SHA256SUMS"; then
  echo "::error::the released SHA256SUMS differs from the one this run built"
  exit 1
fi
echo "ok  released SHA256SUMS is identical to this run's"

# BOTH sides come from the releases: every downloaded asset against the
# downloaded manifest. `--strict` makes a malformed line fatal; the count
# assertion makes a MISSING download fatal — sha256sum -c reports absent
# files but "verified fewer artifacts than the matrix ships" must also be
# unmissable, and comparing against the manifest's own line count is what
# catches a release whose assets and checksums are BOTH short.
manifest_lines=""
manifest_lines=$(wc -l < "${scratch}/SHA256SUMS")
downloaded=""
downloaded=$(find "${scratch}" -name '*.tar.gz' | wc -l)

if [[ ${downloaded} -ne ${manifest_lines} ]]; then
  echo "::error::SHA256SUMS names ${manifest_lines} artifacts; ${downloaded} were attached to the releases"
  exit 1
fi

(cd "${scratch}" && sha256sum --check --strict --quiet SHA256SUMS)
echo "ok  all ${manifest_lines} released artifacts match the released SHA256SUMS"

# The stranger's path, end to end, on the one cell this runner can execute
# natively. Byte-identity above already ties every other cell's published
# bytes to the tarball its own matrix job installed and exercised on native
# hardware before upload.
PG="${PG}" VERSION="${VERSION}" DISTRO=trixie \
  TARBALL="${scratch}/${ASSET}" \
  .github/scripts/smoke-extension.sh

echo "::notice::canary passed — every released artifact verified, the installed tarball conforms"
