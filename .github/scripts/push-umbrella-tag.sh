#!/usr/bin/env bash
# Push the umbrella tag `v<version>`, which is what triggers publish.yml.
#
# Must be run with RELEASE_PLZ_TOKEN, not GITHUB_TOKEN: tags pushed with the
# default token do not trigger workflows (GitHub's recursion guard), and a
# release that silently triggers nothing looks exactly like a successful one
# — the same failure shape as the old "already published; skipping" green
# exit that left edtf-postgres unreleased at v1.0.0 (issue #54).
#
# `v<version>` matches publish.yml's `v*` filter; the per-crate tags
# (edtf-core-v1.0.1 …) do not, so exactly one run fires per release. This is
# the "one number, one tag" line in release-plz.toml made literal.
#
# Deliberately not gated on release-plz's `releases_created` output: if this
# step fails (expired PAT, transient API error), a re-run would find nothing
# newly released, skip, and leave the release permanently untriggered —
# tags cut, nothing published, green. State is derived from the tags
# themselves so re-running always converges.
set -euo pipefail

GITHUB_REPOSITORY="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY must be set}"
GITHUB_SHA="${GITHUB_SHA:?GITHUB_SHA must be set}"

version=$(cargo pkgid --manifest-path crates/edtf-core/Cargo.toml | sed 's/.*[@#]//')
anchor="edtf-core-v${version}"
tag="v${version}"

# Sets ANCHOR_SHA to the tagged commit, or "" if the tag does not exist.
# Invoked as a plain statement rather than as a condition: under `set -e`,
# SC2310 (enable=all) rejects functions used as predicates, because the
# flag is suspended inside them and a real API failure would read as
# "tag absent".
ANCHOR_SHA=""
read_anchor_sha() {
  ANCHOR_SHA=$(gh api "repos/${GITHUB_REPOSITORY}/git/ref/tags/${anchor}" \
    -q .object.sha 2> /dev/null) || ANCHOR_SHA=""
}

read_anchor_sha

# No per-crate tag: release-plz has not released this version. Ordinary push.
if [[ -z ${ANCHOR_SHA} ]]; then
  echo "::notice::${anchor} does not exist — no release in progress"
  exit 0
fi

# The anchor tag existing is NOT sufficient — it also exists for every past
# release. The discriminator is whether this version has actually been
# published yet, which is the question that matters: tags cut but nothing on
# crates.io means a release is mid-flight and publish.yml still needs its
# trigger.
#
# It deliberately does NOT compare the anchor to GITHUB_SHA. release-plz
# releases from "the latest commit of the release PR", not from the squash
# commit that lands on main, so those two are never equal for a squash-merged
# release — a SHA comparison silently skips every real release and leaves it
# tags-cut-nothing-published, green. That is precisely what happened to
# v1.0.1 on its first attempt.
published=""
published=$(curl -sfA 'edtf-release-workflow' \
  "https://crates.io/api/v1/crates/edtf-core/${version}" 2> /dev/null) || published=""

if [[ ${published} == *'"num"'* ]]; then
  echo "::notice::edtf-core ${version} is already on crates.io — ${version} is an earlier release"
  exit 0
fi

if gh api "repos/${GITHUB_REPOSITORY}/git/ref/tags/${tag}" > /dev/null 2>&1; then
  echo "::notice::${tag} already exists; leaving it alone"
else
  gh api --method POST "repos/${GITHUB_REPOSITORY}/git/refs" \
    -f "ref=refs/tags/${tag}" -f "sha=${ANCHOR_SHA}" > /dev/null
  echo "::notice::pushed ${tag} -> ${ANCHOR_SHA}"
fi

# Assert it landed. A missing tag means publish.yml never fires, so failing
# loudly here is the whole point of the check.
if ! gh api "repos/${GITHUB_REPOSITORY}/git/ref/tags/${tag}" > /dev/null 2>&1; then
  echo "::error::${tag} was not created — publish.yml will not run"
  exit 1
fi

echo "::notice::${tag} confirmed; publish.yml takes over"
