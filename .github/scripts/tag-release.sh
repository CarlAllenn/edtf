#!/usr/bin/env bash
# Recovery path (issue #66): create the six per-crate tags and GitHub
# releases for the manifest version, from a workflow_dispatch on main.
#
# Why this exists: release-plz refuses to tag unless the current commit is a
# release-PR merge commit. Any defect discovered AFTER the release PR merges
# therefore leaves the release unrecoverable by the automated path — the
# fix lands as an ordinary PR, release-plz answers "current commit is not
# from a release PR", and re-running phase 1 is a no-op. This script is the
# way back: it does exactly what release-plz's release step would have done
# (per-crate tags + GitHub releases at the current commit), guarded so it
# can only ever act on a version that is bumped in the manifests but absent
# from both the registry and the tag namespace.
#
# It deliberately does NOT push the umbrella tag: that stays with
# push-umbrella-tag.sh, which runs right after this in the dispatch job and
# carries the PAT that publish.yml's trigger requires. This script runs with
# the default GITHUB_TOKEN — per-crate tags are meant to trigger nothing.
set -euo pipefail

GITHUB_REPOSITORY="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY must be set}"
GITHUB_SHA="${GITHUB_SHA:?GITHUB_SHA must be set}"
GITHUB_REF="${GITHUB_REF:?GITHUB_REF must be set}"

# Recovery is only meaningful from main: the manifests being trusted below
# are the ones the release PR merged there.
if [[ ${GITHUB_REF} != "refs/heads/main" ]]; then
  echo "::error::dispatch this workflow from main, not ${GITHUB_REF}"
  exit 1
fi

version=$(cargo pkgid --manifest-path crates/edtf-core/Cargo.toml | sed 's/.*[@#]//')

# Same discriminator as push-umbrella-tag.sh: a version already on crates.io
# is an earlier, completed release — tagging it again at a newer commit
# would be exactly the tags-say-one-thing-registry-says-another mismatch
# this pipeline exists to remove.
published=""
published=$(curl -sfA 'edtf-release-workflow' \
  "https://crates.io/api/v1/crates/edtf-core/${version}" 2> /dev/null) || published=""
if [[ ${published} == *'"num"'* ]]; then
  echo "::error::edtf-core ${version} is already on crates.io — nothing to recover"
  exit 1
fi

# An existing umbrella tag means phase 1 already completed; the way to
# resume a half-published release is to re-dispatch publish.yml at the tag.
if gh api "repos/${GITHUB_REPOSITORY}/git/ref/tags/v${version}" > /dev/null 2>&1; then
  echo "::error::v${version} already exists — re-dispatch publish.yml at the tag instead"
  exit 1
fi

# The section of a crate's changelog for this version, release-plz heading
# style (`## [1.0.1](compare-url) - date` or `## 1.0.1 - date`). Empty when
# the crate had no changelog-worthy commits, which is normal for a
# version-group bump.
changelog_section() {
  awk -v ver="$1" '
    found && /^## / { exit }
    found { print }
    $0 ~ "^## \\[?" ver "[]( ]" { found = 1 }
  ' "$2"
}

CRATES=(edtf-core edtf-calendars edtf-normalize edtf-wasm edtf-cli edtf-postgres)

for name in "${CRATES[@]}"; do
  tag="${name}-v${version}"

  # Sets TAG_SHA to the tagged commit, or "" if the tag does not exist.
  # Plain statement rather than a condition — see push-umbrella-tag.sh for
  # why (SC2310, enable=all).
  TAG_SHA=""
  TAG_SHA=$(gh api "repos/${GITHUB_REPOSITORY}/git/ref/tags/${tag}" \
    -q .object.sha 2> /dev/null) || TAG_SHA=""

  if [[ -n ${TAG_SHA} && ${TAG_SHA} != "${GITHUB_SHA}" ]]; then
    echo "::error::${tag} exists at ${TAG_SHA}, not ${GITHUB_SHA} — refusing to touch it"
    exit 1
  fi

  notes=$(changelog_section "${version}" "crates/${name}/CHANGELOG.md")
  stripped=$(tr -d '[:space:]' <<< "${notes}")
  if [[ -z ${stripped} ]]; then
    notes="Part of the unified v${version} release of the edtf crate family."
  fi

  # Sets RELEASE_EXISTS; plain statement for the same SC2310 reason.
  RELEASE_EXISTS="false"
  if gh release view "${tag}" --repo "${GITHUB_REPOSITORY}" > /dev/null 2>&1; then
    RELEASE_EXISTS="true"
  fi

  # --draft, matching release-plz's git_release_draft on the normal path
  # (issue #55). This script is the recovery path and is NOT covered by that
  # setting — it calls `gh release create` directly — so the flag has to be
  # repeated here or a recovered release would publish immediately, with no
  # assets on it and no way to attach them once immutability applies.
  # publish-releases.sh publishes these at the end of phase 2.
  if [[ ${RELEASE_EXISTS} == "true" ]]; then
    echo "::notice::release ${tag} already exists; leaving it alone"
  elif [[ -n ${TAG_SHA} ]]; then
    # Tag already at the right commit (an earlier partial run): release only.
    gh release create "${tag}" --repo "${GITHUB_REPOSITORY}" \
      --draft --title "${tag}" --notes "${notes}"
    echo "::notice::created draft release ${tag} for the existing tag"
  else
    # `gh release create --target` creates the tag and the release together.
    gh release create "${tag}" --repo "${GITHUB_REPOSITORY}" \
      --draft --target "${GITHUB_SHA}" --title "${tag}" --notes "${notes}"
    echo "::notice::created ${tag} -> ${GITHUB_SHA} (tag + draft release)"
  fi
done

echo "::notice::all six per-crate tags and releases exist at ${GITHUB_SHA}"
