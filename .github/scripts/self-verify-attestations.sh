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

# Retried, and stderr kept: at v1.0.1 every artifact "failed" here with the
# real error invisible — it was a blocked egress host, not bad provenance,
# and the swallowed stderr said so plainly (issue #66). The retries cover
# the attestation API's read-after-write lag; the surfaced stderr covers
# everything else.
# Sets VERIFIED rather than returning a status — a plain statement, not a
# condition, for the same SC2310 reason as push-umbrella-tag.sh.
VERIFIED="false"
verify_once() {
  VERIFIED="false"
  if gh attestation verify "$1" \
    --repo "${GITHUB_REPOSITORY}" \
    --source-ref "${GITHUB_REF}" \
    --source-digest "${GITHUB_SHA}" \
    --signer-workflow "${GITHUB_REPOSITORY}/.github/workflows/publish.yml" \
    > /dev/null 2> "${RUNNER_TEMP:-/tmp}/verify-stderr.txt"; then
    VERIFIED="true"
  fi
}

for artifact in dist/*; do
  [[ -f ${artifact} ]] || continue
  checked=$((checked + 1))

  for attempt in 1 2 3; do
    verify_once "${artifact}"
    if [[ ${VERIFIED} == "true" ]]; then
      break
    fi
    if [[ ${attempt} -lt 3 ]]; then
      sleep 20
    fi
  done

  if [[ ${VERIFIED} == "true" ]]; then
    echo "ok  $(basename "${artifact}")  ref=${GITHUB_REF}  commit=${GITHUB_SHA}"
  else
    echo "::error::$(basename "${artifact}"): attestation verification failed"
    cat "${RUNNER_TEMP:-/tmp}/verify-stderr.txt"
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
