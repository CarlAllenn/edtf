#!/usr/bin/env bash
# Write SHA256SUMS over the extension tarballs, and prove the matrix is whole.
#
# The checksums file is what a Dockerfile or install script actually verifies
# against, so it is generated from the same dist/ the attestation covers —
# `actions/attest` takes `subject-path: dist/*`, and SHA256SUMS lands there
# too, so the file is itself attested. Generating it anywhere else, or from
# anything other than these bytes, would let the two drift.
#
# The count assertion is the point of the script as much as the hashing is.
# A missing matrix cell is otherwise invisible: `download-artifact` with a
# pattern happily produces fewer files than expected, and the release would
# ship a support matrix claiming ten tarballs with nine attached.
set -euo pipefail

VERSION="${VERSION:?VERSION must be set}"

PG_MAJORS=(14 15 16 17 18)
ARCHES=(amd64 arm64)

expected=$((${#PG_MAJORS[@]} * ${#ARCHES[@]}))

missing=()
for pg in "${PG_MAJORS[@]}"; do
  for arch in "${ARCHES[@]}"; do
    name="edtf_postgres-${VERSION}-pg${pg}-linux-${arch}.tar.gz"
    if [[ ! -f "dist/${name}" ]]; then
      missing+=("${name}")
    fi
  done
done

if [[ ${#missing[@]} -gt 0 ]]; then
  echo "::error::${#missing[@]} of ${expected} extension tarballs are missing:"
  printf '::error::  %s\n' "${missing[@]}"
  exit 1
fi

# Sorted, so the file is stable across runs regardless of download order.
(cd dist && sha256sum edtf_postgres-*.tar.gz | sort -k2 > SHA256SUMS)

echo "::notice::${expected} extension tarballs present and checksummed"
cat dist/SHA256SUMS
