#!/usr/bin/env bash
# Publish the six per-crate releases, last, once every asset is attached —
# one at a time, verifying immutability before moving to the next.
#
# Phase 1 creates them as drafts (`git_release_draft` in release-plz.toml,
# and `--draft` in tag-release.sh for the dispatch recovery path). This step
# is what makes them public.
#
# Why drafts at all: immutability is enforced at PUBLISH, not at creation.
# GitHub's guidance is create-as-draft, attach every asset, then publish —
# and the previous shape did the opposite, publishing in phase 1 and
# attaching SBOMs in phase 2 with `gh release upload --clobber`. With
# immutability on, that clobber is refused and the release cannot be
# completed at all. It is the better shape independently: a release that
# strands now leaves nothing public, and assets appear with the release.
#
# Why one at a time. A release published while the repository's
# immutable-releases setting is off stays mutable PERMANENTLY — `gh release
# edit` cannot retrofit it and re-dispatching cannot either. The obvious
# guard would be a precondition reading
# `GET /repos/{owner}/{repo}/immutable-releases`, but that is a repository
# settings read: `administration` is not a grantable workflow permission
# scope, so GITHUB_TOKEN cannot be given access to it. Checking the setting
# from CI is therefore not available.
#
# So the check is made against the only thing this job can observe — a
# release it has just published — and the blast radius is bounded by doing
# them sequentially. If the setting is off, exactly ONE release is mutable
# and the run stops; the other five stay drafts and publish correctly once
# the setting is enabled and the tag is re-dispatched.
#
# Operators can check the setting directly before releasing (admin rights
# required); the runbook records the call.
#
# Idempotent, like every other step: an already-published release is left
# alone, so re-dispatching at the tag converges rather than erroring.
set -euo pipefail

VERSION="${VERSION:?VERSION must be set}"
GITHUB_REPOSITORY="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY must be set}"

CRATES=(edtf-core edtf-calendars edtf-normalize edtf-wasm edtf-cli edtf-postgres)

# Plain statements rather than functions-as-conditions throughout: under
# `set -e` the flag is suspended inside a function, which SC2310 rejects
# (.shellcheckrc enable=all) — a real API failure would otherwise be read
# as a legitimate value.
for name in "${CRATES[@]}"; do
  tag="${name}-v${VERSION}"

  STATE=""
  STATE=$(gh api "repos/${GITHUB_REPOSITORY}/releases/tags/${tag}" \
    --jq '"\(.draft) \(.immutable)"' 2> /dev/null) || STATE=""

  if [[ -z ${STATE} ]]; then
    echo "::error::release ${tag} does not exist — phase 1 did not create it"
    exit 1
  fi

  if [[ ${STATE} == "false true" ]]; then
    echo "ok  ${tag}  already published, immutable"
    continue
  fi

  if [[ ${STATE} == "false "* ]]; then
    echo "::error::${tag} is already published but NOT immutable (${STATE})"
    echo "::error::that cannot be corrected in place; enable immutable releases and cut a new version"
    exit 1
  fi

  gh release edit "${tag}" --repo "${GITHUB_REPOSITORY}" --draft=false

  # Read back immediately. Anything other than published-and-immutable stops
  # the run here, leaving the remaining releases as drafts.
  AFTER=""
  AFTER=$(gh api "repos/${GITHUB_REPOSITORY}/releases/tags/${tag}" \
    --jq '"\(.draft) \(.immutable)"' 2> /dev/null) || AFTER=""

  if [[ ${AFTER} != "false true" ]]; then
    echo "::error::${tag}: expected draft=false immutable=true, got '${AFTER:-unreadable}'"
    echo "::error::immutable releases appear to be DISABLED on ${GITHUB_REPOSITORY}"
    echo "::error::enable it, then re-dispatch this tag — the remaining releases are still drafts:"
    echo "::error::  gh api --method PUT repos/${GITHUB_REPOSITORY}/immutable-releases"
    exit 1
  fi

  echo "ok  ${tag}  published, immutable"
done

echo "::notice::all six releases published and immutable at ${VERSION}"
