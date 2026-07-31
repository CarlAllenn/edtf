#!/usr/bin/env bash
# Attach the extension tarballs and SHA256SUMS to the edtf-postgres release.
#
# Per-crate, like the SBOMs: these artifacts are edtf-postgres and nothing
# else, so they belong on that crate's release. There is no release at the
# umbrella tag to put them on anyway (issue #66, defect 1).
#
# REPLACES rather than skips, while the release is a draft.
#
# The obvious shape — skip any asset whose name is already attached — is
# wrong here, and dangerously so. A re-dispatched run rebuilds all ten
# tarballs, and those bytes are explicitly not reproducible
# (build-extension-inner.sh says so: the .so is not claimed bit-identical,
# and the container apt-installs at run time). checksum-extension.sh then
# regenerates SHA256SUMS over the NEW bytes. Skipping by name would leave the
# release carrying run 1's tarballs beside run 2's manifest — a published,
# immutable release whose checksums file contradicts its own assets — and the
# canary would compare freshly built digests against stale published ones and
# fail forever, so the release could never be completed at all.
#
# Drafts are mutable, which is the whole reason phase 1 stops short of
# publishing. So the coherent move is to overwrite: after this runs, the
# attached bytes, dist/, SHA256SUMS and the attestation subjects all describe
# the same artifacts by construction.
#
# Once a release is published it is immutable and nothing can be attached.
# That is not recoverable in place, so it fails loudly rather than pretending.
set -euo pipefail

VERSION="${VERSION:?VERSION must be set}"
GITHUB_REPOSITORY="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY must be set}"

TAG="edtf-postgres-v${VERSION}"

assets=()
for path in dist/edtf_postgres-*.tar.gz; do
  assets+=("$(basename "${path}")")
done
assets+=(SHA256SUMS)

# Plain statements, never functions-as-conditions (SC2310, .shellcheckrc
# enable=all): a failed API call must not read as a legitimate value.
IS_DRAFT=""
IS_DRAFT=$(gh release view "${TAG}" --repo "${GITHUB_REPOSITORY}" \
  --json isDraft --jq .isDraft 2> /dev/null) || IS_DRAFT=""

if [[ -z ${IS_DRAFT} ]]; then
  echo "::error::release ${TAG} does not exist — phase 1 did not create it"
  exit 1
fi

if [[ ${IS_DRAFT} == "true" ]]; then
  for name in "${assets[@]}"; do
    gh release upload "${TAG}" "dist/${name}" \
      --repo "${GITHUB_REPOSITORY}" --clobber
    echo "::notice::attached ${name} to ${TAG}"
  done
else
  echo "::notice::${TAG} is already published; assets are immutable"
fi

# Read the release back and prove every asset is on it. On the published
# path this is the only check that runs, and it is the one that matters:
# an immutable release missing an asset cannot be repaired.
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
  echo "::error::not attached to ${TAG}: ${missing[*]}"
  if [[ ${IS_DRAFT} != "true" ]]; then
    echo "::error::${TAG} is published and immutable — this cannot be repaired in place"
  fi
  exit 1
fi

echo "::notice::all ${#assets[@]} extension assets attached to ${TAG}"
