#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) the edtf contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
# Decide whether this run is a rehearsal, explicitly.
#
# Not derived from `${{ inputs.dry_run || 'false' }}`. GitHub's documentation
# does not state what the `inputs` context contains for events other than
# workflow_dispatch and workflow_call, nor whether a `type: boolean` input
# arrives as a boolean or a string. Both matter here: getting it wrong in one
# direction makes a tag push silently skip publishing, and in the other makes
# a rehearsal publish for real.
#
# So the decision is made from GITHUB_EVENT_NAME, which is documented and
# unambiguous:
#   push             -> a real release, never a rehearsal
#   workflow_dispatch -> whatever the operator asked for, defaulting to
#                        rehearsal, because the safe default for an
#                        ambiguous input is the one that publishes nothing
set -euo pipefail

GITHUB_EVENT_NAME="${GITHUB_EVENT_NAME:?GITHUB_EVENT_NAME must be set}"
GITHUB_ENV="${GITHUB_ENV:?GITHUB_ENV must be set}"

# Raw dispatch input, passed in as a string by the workflow. Empty for any
# event that has no inputs.
raw="${DRY_RUN_INPUT:-}"

case "${GITHUB_EVENT_NAME}" in
  push)
    dry_run="false"
    reason="push to a release tag"
    ;;
  workflow_dispatch)
    if [[ ${raw} == "false" ]]; then
      dry_run="false"
      reason="dispatch with dry_run explicitly false"
    else
      # Anything else — "true", empty, or an unexpected value — rehearses.
      dry_run="true"
      reason="dispatch (dry_run=${raw:-unset})"
    fi
    ;;
  *)
    dry_run="true"
    reason="unexpected event ${GITHUB_EVENT_NAME}; refusing to publish"
    ;;
esac

echo "DRY_RUN=${dry_run}" >> "${GITHUB_ENV}"
echo "::notice::DRY_RUN=${dry_run} (${reason})"
