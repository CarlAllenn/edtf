#!/usr/bin/env bash
# Attach the extension tarballs and SHA256SUMS to the edtf-postgres release.
#
# Per-crate, like the SBOMs: these artifacts are edtf-postgres and nothing
# else, so they belong on that crate's release. There is no release at the
# umbrella tag to put them on anyway (issue #66, defect 1).
#
# The release is still a DRAFT at this point — that is the whole reason
# phase 1 stopped publishing. Immutability applies at publish, so every
# asset has to be in place first; publish-releases.sh comes last.
#
# Idempotent in the shape publish-crates.sh established: state is read back
# from the release rather than assumed from a workflow output, each asset is
# skipped if already present, and a final assertion proves the whole set
# landed. Without that assertion a partial upload is invisible — the release
# would simply have fewer assets than the support matrix promises.
set -euo pipefail

VERSION="${VERSION:?VERSION must be set}"
GITHUB_REPOSITORY="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY must be set}"

TAG="edtf-postgres-v${VERSION}"

assets=()
for path in dist/edtf_postgres-*.tar.gz; do
  assets+=("$(basename "${path}")")
done
assets+=(SHA256SUMS)

# Plain statement, then read — never a function as a condition (SC2310,
# .shellcheckrc enable=all): a failed API call must not read as "absent".
EXISTING=""
EXISTING=$(gh release view "${TAG}" --repo "${GITHUB_REPOSITORY}" \
  --json assets --jq '.assets[].name' 2> /dev/null) || EXISTING=""

for name in "${assets[@]}"; do
  if grep -qxF "${name}" <<< "${EXISTING}"; then
    echo "::notice::${name} already attached to ${TAG}; skipping"
    continue
  fi
  gh release upload "${TAG}" "dist/${name}" --repo "${GITHUB_REPOSITORY}"
  echo "::notice::attached ${name} to ${TAG}"
done

# Read the release back and prove every asset is on it.
FINAL=""
FINAL=$(gh release view "${TAG}" --repo "${GITHUB_REPOSITORY}" \
  --json assets --jq '.assets[].name' 2> /dev/null) || FINAL=""

missing=()
for name in "${assets[@]}"; do
  if ! grep -qxF "${name}" <<< "${FINAL}"; then
    missing+=("${name}")
  fi
done

if [[ ${#missing[@]} -gt 0 ]]; then
  echo "::error::not attached to ${TAG} after upload: ${missing[*]}"
  exit 1
fi

echo "::notice::all ${#assets[@]} extension assets attached to ${TAG}"
