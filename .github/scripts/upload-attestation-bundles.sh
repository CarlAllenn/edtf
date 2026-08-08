#!/usr/bin/env bash
# Attach the Sigstore attestation bundles to the per-crate releases.
#
# The attestations already exist in GitHub's attestation store — that is
# where `gh attestation verify` reads them — but nothing on the RELEASE
# carries them, so a consumer (or a scanner: OpenSSF Scorecard's
# Signed-Releases check) looking at the release itself sees unsigned
# assets. The bundles are the same Sigstore material, published where the
# artifacts are. `*.intoto.jsonl` is the conventional name for in-toto
# provenance bundles and the shape scanners recognise.
#
# Each release gets two bundles:
#   <name>-<version>.provenance.intoto.jsonl — the build-provenance bundle.
#     One attest call covered every dist/* subject, so this file is the
#     same for all six releases; the subjects named inside it are what
#     scope it, not the filename.
#   <name>-<version>.sbom.intoto.jsonl — that crate's SBOM attestation,
#     which is per-crate by construction.
#
# Read-back-and-skip, never `--clobber`, same as upload-sboms.sh: a resumed
# run reaches this step with some releases already published and immutable,
# and `--clobber` would turn that recovery path into a fatal error.
set -euo pipefail

VERSION="${VERSION:?VERSION must be set}"
GITHUB_REPOSITORY="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY must be set}"
BUNDLE_DIR="${BUNDLE_DIR:?BUNDLE_DIR must be set}"

CRATES=(edtf-core edtf-calendars edtf-normalize edtf-wasm edtf-cli edtf-postgres)

# Plain statements, never functions-as-conditions: under `set -e` the flag is
# suspended inside a function, which SC2310 rejects (.shellcheckrc
# enable=all) — a failed API call must not read as "asset absent".
for name in "${CRATES[@]}"; do
  tag="${name}-v${VERSION}"

  ATTACHED=""
  ATTACHED=$(gh release view "${tag}" --repo "${GITHUB_REPOSITORY}" \
    --json assets --jq '.assets[].name' 2> /dev/null) || ATTACHED=""

  for kind in provenance sbom; do
    asset="${name}-${VERSION}.${kind}.intoto.jsonl"

    if grep -qxF "${asset}" <<< "${ATTACHED}"; then
      echo "::notice::${asset} already attached to ${tag}; skipping"
      continue
    fi

    gh release upload "${tag}" "${BUNDLE_DIR}/${asset}" --repo "${GITHUB_REPOSITORY}"
    echo "::notice::attached ${asset} to ${tag}"
  done
done

# Prove the whole set landed, whether this run attached them or a prior one
# did. Without this a partial attach is invisible: the release simply has
# fewer assets than it should.
missing=()
for name in "${CRATES[@]}"; do
  tag="${name}-v${VERSION}"

  FINAL=""
  FINAL=$(gh release view "${tag}" --repo "${GITHUB_REPOSITORY}" \
    --json assets --jq '.assets[].name' 2> /dev/null) || FINAL=""

  for kind in provenance sbom; do
    asset="${name}-${VERSION}.${kind}.intoto.jsonl"
    if ! grep -qxF "${asset}" <<< "${FINAL}"; then
      missing+=("${tag}/${asset}")
    fi
  done
done

if [[ ${#missing[@]} -gt 0 ]]; then
  echo "::error::attestation bundles not attached after upload: ${missing[*]}"
  exit 1
fi

echo "::notice::all attestation bundles attached at ${VERSION}"
