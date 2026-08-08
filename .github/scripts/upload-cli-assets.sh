#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) the edtf contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
# Attach the CLI tarballs and SHA256SUMS to the edtf-cli release.
#
# Per-crate, like the SBOMs and the extension tarballs: these artifacts are
# edtf-cli and nothing else, so they belong on that crate's release.
# SHA256SUMS is the shared manifest from checksum-artifacts.sh — it also
# names the extension tarballs, and it is attached to both releases so each
# artifact's release carries the checksums that describe it.
#
# REPLACES rather than skips, while the release is a draft — the same
# reasoning as upload-extension-assets.sh: a re-dispatched run rebuilds the
# binaries, the bytes are not claimed reproducible, and SHA256SUMS is
# regenerated over the new bytes. Skipping by name would leave a release
# whose checksums file contradicts its own assets.
#
# Once a release is published it is immutable and nothing can be attached.
# That is not recoverable in place, so it fails loudly rather than pretending.
set -euo pipefail

VERSION="${VERSION:?VERSION must be set}"
GITHUB_REPOSITORY="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY must be set}"

TAG="edtf-cli-v${VERSION}"

assets=()
for path in dist/edtf-cli-*.tar.gz; do
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

echo "::notice::all ${#assets[@]} CLI assets attached to ${TAG}"
