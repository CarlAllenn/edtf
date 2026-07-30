#!/usr/bin/env bash
# Prove the attestations this run just produced say what they should.
#
# This is the check that would have caught v1.0.0 automatically. A bare
# `gh attestation verify` is NOT sufficient: it passes on the v1.0.0
# artifacts today, because their digests are genuine — what is wrong is the
# commit and ref the provenance names. So the assertion has to be made with
# the flags that pin those:
#
#   --source-ref     the tag this release is for
#   --source-digest  the commit that tag points at
#   --signer-workflow which workflow was permitted to sign at all
#
# If any of the three disagree, the release fails here rather than shipping
# a signature that verifies green while pointing somewhere else.
set -euo pipefail

GITHUB_REPOSITORY="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY must be set}"
GITHUB_REF="${GITHUB_REF:?GITHUB_REF must be set}"
GITHUB_SHA="${GITHUB_SHA:?GITHUB_SHA must be set}"

fail=0
checked=0

for artifact in dist/*; do
  [[ -f ${artifact} ]] || continue
  checked=$((checked + 1))
  if gh attestation verify "${artifact}" \
    --repo "${GITHUB_REPOSITORY}" \
    --source-ref "${GITHUB_REF}" \
    --source-digest "${GITHUB_SHA}" \
    --signer-workflow "${GITHUB_REPOSITORY}/.github/workflows/publish.yml" \
    > /dev/null 2>&1; then
    echo "ok  $(basename "${artifact}")  ref=${GITHUB_REF}  commit=${GITHUB_SHA}"
  else
    echo "::error::$(basename "${artifact}"): provenance does not match this tag, commit or workflow"
    fail=1
  fi
done

# A zero-artifact loop must not pass silently — that would be the check
# reporting success for having verified nothing.
if [[ ${checked} -eq 0 ]]; then
  echo "::error::no artifacts found in dist/ to verify"
  exit 1
fi

if [[ ${fail} -ne 0 ]]; then
  exit 1
fi

echo "::notice::${checked} artifacts verified against ${GITHUB_REF} @ ${GITHUB_SHA}"
