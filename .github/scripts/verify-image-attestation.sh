#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) the edtf contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
# Verify a published image digest the way a consumer would (issue #159).
#
# This is the image leg's counterpart to self-verify-attestations.sh, and it
# exists as a script for the same reason the published `COPY --from` snippet
# is a Dockerfile CI builds: the command in SECURITY.md and the command the
# release runs must be one command. A documented invocation nobody executes
# is a guess, and an executed invocation nobody documents is a guarantee the
# consumer cannot use.
#
# An attest step that succeeded is not the same as an attestation that
# verifies against the identity consumers check, so this runs against the
# pushed digest, from the registry, after the push.
#
# The flags are the check (SECURITY.md says so for tarballs; it is no less
# true here). A bare verify passes for anything this workflow ever signed:
#
#   --source-ref      the tag this release is for — what ties the image to
#                     the release rather than to the repository in general
#   --source-digest   the commit that tag points at; the release run knows
#                     it, which is why this pins one flag more than the
#                     documented consumer command
#   --signer-workflow which workflow was permitted to sign at all
#
# Both manifest lists are passed by the caller: the runnable and `-artifact`
# variants are separate published digests with separate attestations, and
# verifying one proves nothing about the other.
set -euo pipefail

GITHUB_REPOSITORY="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY must be set}"
GITHUB_REF="${GITHUB_REF:?GITHUB_REF must be set}"
GITHUB_SHA="${GITHUB_SHA:?GITHUB_SHA must be set}"

if [[ $# -eq 0 ]]; then
  echo "::error::no image references given to verify"
  echo "usage: $0 <image>@<digest> [<image>@<digest> …]"
  exit 1
fi

for ref in "$@"; do
  gh attestation verify "oci://${ref}" \
    --repo "${GITHUB_REPOSITORY}" \
    --source-ref "${GITHUB_REF}" \
    --source-digest "${GITHUB_SHA}" \
    --signer-workflow "${GITHUB_REPOSITORY}/.github/workflows/publish.yml"
  echo "::notice::${ref} verified against ${GITHUB_REF} @ ${GITHUB_SHA}"
done
