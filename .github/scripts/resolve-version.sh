#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) the edtf contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
# Resolve the release version and refuse to continue unless everything agrees.
#
# Three things must line up before anything is built: the tag being published,
# the workspace version, and edtf-postgres's hand-maintained version. ci.yml
# gates the last one on every PR, but it is re-checked here because a wrong
# version reaching crates.io is unrecoverable in a way a red PR is not.
set -euo pipefail

GITHUB_REF_NAME="${GITHUB_REF_NAME:?GITHUB_REF_NAME must be set}"
GITHUB_ENV="${GITHUB_ENV:?GITHUB_ENV must be set}"
GITHUB_REF="${GITHUB_REF:?GITHUB_REF must be set}"
DRY_RUN="${DRY_RUN:-false}"

version=$(cargo pkgid --manifest-path crates/edtf-core/Cargo.toml | sed 's/.*[@#]//')
pg_version=$(cargo pkgid --manifest-path crates/edtf-postgres/Cargo.toml | sed 's/.*[@#]//')

# The tag must name the version being released. Skipped only when rehearsing
# from a branch, which has no version in its name — assert-publish-ref.sh has
# already established that a branch ref is permitted solely for a dry run.
if [[ ${GITHUB_REF} == refs/tags/* ]]; then
  if [[ "v${version}" != "${GITHUB_REF_NAME}" ]]; then
    echo "::error::tag ${GITHUB_REF_NAME} does not match workspace version ${version}"
    exit 1
  fi
elif [[ ${DRY_RUN} != "true" ]]; then
  echo "::error::refusing to release from non-tag ref ${GITHUB_REF}"
  exit 1
else
  echo "::notice::rehearsing from ${GITHUB_REF}; a real release runs at refs/tags/v${version}"
fi

if [[ ${pg_version} != "${version}" ]]; then
  echo "::error::edtf-postgres is ${pg_version}, workspace is ${version}"
  exit 1
fi

echo "VERSION=${version}" >> "${GITHUB_ENV}"
echo "::notice::releasing ${version} — tag, workspace and edtf-postgres agree"
