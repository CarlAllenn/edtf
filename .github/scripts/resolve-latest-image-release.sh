#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) the edtf contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
# Resolve the newest published edtf-postgres release for the PR-side image
# gate (issue #160).
#
# The gate builds the LAST RELEASED version's image, not this branch's: the
# image is built from release assets by URL, so a pull request for an
# unreleased version has nothing to build from. It loses nothing — the
# Dockerfile graph and its targets, the stage wiring, ADD --checksum, the
# layout, the smoke scripts and the base-image lookup are what a pull
# request changes, and they are exercised identically whichever released
# version supplies the bytes.
#
# Which makes version resolution the ONE thing this gate does differently
# from the release path: there, VERSION is a `taplo get` from
# crates/edtf-postgres/Cargo.toml; here it is the newest release tag.
#
# Drafts and prereleases are excluded deliberately: a draft's assets are not
# fetchable by URL, and that URL is the exact path under test.
set -euo pipefail

GITHUB_ENV="${GITHUB_ENV:?GITHUB_ENV must be set}"
REPO="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY must be set}"

# Bounded: gh has no transfer deadline of its own (v1.2.0 hang lesson).
# Sorted by publication rather than trusting list order, and captured before
# the emptiness check so a failed call cannot pass as "no releases".
TAG=""
TAG=$(timeout 120 gh api "repos/${REPO}/releases?per_page=100" \
  --jq '[.[]
        | select(.draft == false and .prerelease == false)
        | select(.tag_name | startswith("edtf-postgres-v"))]
        | sort_by(.published_at) | reverse | .[0].tag_name // ""')

if [[ -z ${TAG} ]]; then
  echo "::error::no published edtf-postgres-v* release for the image gate to build"
  exit 1
fi

echo "VERSION=${TAG#edtf-postgres-v}" >> "${GITHUB_ENV}"
echo "::notice::image gate builds the released ${TAG}"
