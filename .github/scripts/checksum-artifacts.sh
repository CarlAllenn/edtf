#!/usr/bin/env bash
# Write SHA256SUMS over the binary artifacts — the extension tarballs and
# the CLI tarballs — and prove both matrices are whole.
#
# One manifest, not one per artifact family: dist/ can only hold one file
# named SHA256SUMS, and that name is what install snippets and downstream
# verifiers (monumental-archive's check-edtf-attestation.sh) expect on the
# release. The file is attached to BOTH the edtf-postgres and edtf-cli
# releases; consumers verify the lines that name their artifact.
#
# The checksums file is what a Dockerfile or install script actually verifies
# against, so it is generated from the same dist/ the attestation covers —
# `actions/attest` takes `subject-path: dist/*`, and SHA256SUMS lands there
# too, so the file is itself attested. Generating it anywhere else, or from
# anything other than these bytes, would let the two drift.
#
# The count assertions are the point of the script as much as the hashing
# is. A missing matrix cell is otherwise invisible: `download-artifact` with
# a pattern happily produces fewer files than expected, and the release
# would ship a support matrix claiming N tarballs with N-1 attached.
set -euo pipefail

VERSION="${VERSION:?VERSION must be set}"

PG_MAJORS=(14 15 16 17 18)
ARCHES=(amd64 arm64)
CLI_TARGETS=(
  x86_64-unknown-linux-gnu
  aarch64-unknown-linux-gnu
  aarch64-apple-darwin
  x86_64-apple-darwin
)

expected=$((${#PG_MAJORS[@]} * ${#ARCHES[@]} + ${#CLI_TARGETS[@]}))

missing=()
for pg in "${PG_MAJORS[@]}"; do
  for arch in "${ARCHES[@]}"; do
    name="edtf_postgres-${VERSION}-pg${pg}-linux-${arch}.tar.gz"
    if [[ ! -f "dist/${name}" ]]; then
      missing+=("${name}")
    fi
  done
done
for target in "${CLI_TARGETS[@]}"; do
  name="edtf-cli-${VERSION}-${target}.tar.gz"
  if [[ ! -f "dist/${name}" ]]; then
    missing+=("${name}")
  fi
done

if [[ ${#missing[@]} -gt 0 ]]; then
  echo "::error::${#missing[@]} of ${expected} binary tarballs are missing:"
  printf '::error::  %s\n' "${missing[@]}"
  exit 1
fi

# Sorted, so the file is stable across runs regardless of download order.
(cd dist && sha256sum edtf_postgres-*.tar.gz edtf-cli-*.tar.gz | sort -k2 > SHA256SUMS)

echo "::notice::${expected} binary tarballs present and checksummed"
cat dist/SHA256SUMS
