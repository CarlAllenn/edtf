#!/usr/bin/env bash
# Refuse to publish from anything but a release tag.
#
# Provenance is the product of this workflow. A run on a branch records
# `ref: refs/heads/...`, which is how every v1.0.0 attestation ended up
# naming a commit that built none of the published bytes (issue #54).
#
# A dry run is exempt, deliberately. It signs nothing and publishes nothing,
# so it has no provenance to get wrong — and without the exemption the
# rehearsal is unreachable: dispatching a workflow needs a ref where that
# workflow's file already exists, and minting a `v*` tag to create one fires
# the push trigger, where `inputs.dry_run` is undefined and the run attempts
# a real publish. The tag-immutability ruleset then makes the throwaway tag
# permanent. A rehearsal that can only be performed by risking the thing it
# rehearses is not a rehearsal.
set -euo pipefail

GITHUB_REF="${GITHUB_REF:?GITHUB_REF must be set}"
DRY_RUN="${DRY_RUN:-false}"

case "${GITHUB_REF}" in
  refs/tags/v*)
    echo "::notice::release ref ${GITHUB_REF}"
    ;;
  *)
    if [[ ${DRY_RUN} == "true" ]]; then
      echo "::notice::dry run on ${GITHUB_REF} — nothing is published or signed"
      echo "::warning::a real release runs at refs/tags/v<version>; this rehearsal cannot exercise the tag trigger itself"
    else
      echo "::error::refusing to publish from ${GITHUB_REF}"
      echo "::error::a real release must run at refs/tags/v<version> — dispatch with --ref <tag>"
      exit 1
    fi
    ;;
esac
