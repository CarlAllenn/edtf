#!/usr/bin/env bash
# Attach each crate's SBOM to that crate's GitHub release.
#
# Per-crate, not umbrella (issue #66, defect 1): phase 1 creates only the
# per-crate releases (edtf-core-v1.0.1, …) — there is no release at the
# umbrella tag, so the old `gh release upload "${GITHUB_REF_NAME}"` targeted
# something that never exists. Per-crate is also the honest shape: each SBOM
# describes exactly one crate's dependency closure.
#
# Read-back-and-skip, never `--clobber`. This script used to clobber
# unconditionally, which made every resume fatal once immutability was on:
# publish-releases.sh publishes the six releases ONE AT A TIME and is
# designed to stop mid-loop, so a re-dispatch reaches this step with some
# releases already published and immutable. `--clobber` deletes before
# uploading and immutable releases refuse both, so the run died here — several
# steps before it could reach publish-releases.sh and finish the remaining
# drafts. The documented recovery path was therefore permanently blocked by
# the one surviving `--clobber` in the pipeline.
set -euo pipefail

VERSION="${VERSION:?VERSION must be set}"
GITHUB_REPOSITORY="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY must be set}"

CRATES=(edtf-core edtf-calendars edtf-normalize edtf-wasm edtf-cli edtf-postgres)

# Plain statements, never functions-as-conditions: under `set -e` the flag is
# suspended inside a function, which SC2310 rejects (.shellcheckrc
# enable=all) — a failed API call must not read as "asset absent".
for name in "${CRATES[@]}"; do
  tag="${name}-v${VERSION}"
  asset="${name}.cdx.json"

  ATTACHED=""
  ATTACHED=$(gh release view "${tag}" --repo "${GITHUB_REPOSITORY}" \
    --json assets --jq '.assets[].name' 2> /dev/null) || ATTACHED=""

  if grep -qxF "${asset}" <<< "${ATTACHED}"; then
    echo "::notice::${asset} already attached to ${tag}; skipping"
    continue
  fi

  gh release upload "${tag}" "sbom/${asset}" --repo "${GITHUB_REPOSITORY}"
  echo "::notice::attached ${asset} to ${tag}"
done

# Prove the whole set landed, whether this run attached them or a prior one
# did. Without this a partial attach is invisible: the release simply has
# fewer assets than it should.
missing=()
for name in "${CRATES[@]}"; do
  tag="${name}-v${VERSION}"
  asset="${name}.cdx.json"

  FINAL=""
  FINAL=$(gh release view "${tag}" --repo "${GITHUB_REPOSITORY}" \
    --json assets --jq '.assets[].name' 2> /dev/null) || FINAL=""

  if ! grep -qxF "${asset}" <<< "${FINAL}"; then
    missing+=("${tag}/${asset}")
  fi
done

if [[ ${#missing[@]} -gt 0 ]]; then
  echo "::error::SBOMs not attached after upload: ${missing[*]}"
  exit 1
fi

echo "::notice::all ${#CRATES[@]} SBOMs attached at ${VERSION}"
