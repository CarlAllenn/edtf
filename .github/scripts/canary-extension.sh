#!/usr/bin/env bash
# Consume the extension the way a stranger will: from the release asset.
#
# canary.sh proves the crates and the npm package resolve and run when
# fetched from their registries. It deliberately skipped edtf-postgres,
# because consuming it needed a running Postgres and a pgrx toolchain —
# deferred to issue #55, which is this.
#
# The gap this closes is narrow but real. The matrix jobs smoke-tested the
# tarballs they built, in memory of their own filesystem. This takes the
# bytes that are actually ATTACHED TO THE RELEASE, proves they are the same
# bytes SHA256SUMS names, and installs them. A truncated upload, a
# mis-attached file, or a checksums file describing something other than
# what shipped all survive every earlier check and die here.
#
# One cell, not ten: the other nine were each installed and exercised in the
# job that built them, on two distros. What is unproven at this point is the
# upload, and one asset demonstrates that as well as ten would.
set -euo pipefail

VERSION="${VERSION:?VERSION must be set}"
GITHUB_REPOSITORY="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY must be set}"

# The default feature, and the architecture this runner can smoke-test
# natively.
PG=18
ARCH=amd64
ASSET="edtf_postgres-${VERSION}-pg${PG}-linux-${ARCH}.tar.gz"
TAG="edtf-postgres-v${VERSION}"

scratch=$(mktemp -d)
trap 'rm -rf "${scratch}"' EXIT

# The release is still a draft — that is why immutability works at all — so
# this is an authenticated fetch rather than an anonymous one. What is being
# proved is the integrity of the attached bytes, not the anonymity of the
# fetch.
gh release download "${TAG}" --repo "${GITHUB_REPOSITORY}" \
  --pattern "${ASSET}" --dir "${scratch}"

if [[ ! -f "${scratch}/${ASSET}" ]]; then
  echo "::error::${ASSET} could not be downloaded from ${TAG}"
  exit 1
fi

# Compare against the checksums file that ships beside it, not against a
# digest recomputed from dist/ — the point is that the published file and
# the published manifest agree.
expected=""
expected=$(awk -v n="${ASSET}" '$2 == n { print $1 }' dist/SHA256SUMS)
actual=""
actual=$(sha256sum "${scratch}/${ASSET}" | cut -d' ' -f1)

if [[ -z ${expected} ]]; then
  echo "::error::${ASSET} is not listed in SHA256SUMS"
  exit 1
fi

if [[ ${expected} != "${actual}" ]]; then
  echo "::error::${ASSET}: SHA256SUMS says ${expected}, the release serves ${actual}"
  exit 1
fi

echo "ok  ${ASSET} matches SHA256SUMS"

PG="${PG}" VERSION="${VERSION}" DISTRO=trixie \
  TARBALL="${scratch}/${ASSET}" \
  .github/scripts/smoke-extension.sh

echo "::notice::canary passed — the released tarball installs and conforms"
