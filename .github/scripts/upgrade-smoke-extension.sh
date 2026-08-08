#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) the edtf contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
# Execute a real ALTER EXTENSION UPDATE from the previous release to this
# build (issue #83, gap 1).
#
# smoke-extension.sh proves the upgrade scripts SHIPPED and that Postgres
# can resolve a chain — pg_extension_update_paths reads the .sql files —
# but until v1.1.0 no release had a tarball to upgrade FROM, so the UPDATE
# itself was asserted, never executed. That constraint has expired: this
# installs the newest prior release that published a tarball for this
# pg/arch cell, extracts THIS build over it, runs ALTER EXTENSION UPDATE,
# and holds the upgraded extension to the same corpus as a fresh install.
#
# Skips cleanly when no prior release ships a matching tarball — true for
# a new pg major's first release, and for the fork case the runbook
# describes — because "nothing to upgrade from" is a fact, not a failure.
set -euo pipefail

# shellcheck source=.github/scripts/base-images.sh
. "$(dirname "${BASH_SOURCE[0]}")/base-images.sh"

PG="${PG:?PG must be set}"
ARCH="${ARCH:?ARCH must be set}"
VERSION="${VERSION:?VERSION must be set}"
TARBALL="${TARBALL:?TARBALL must be set to this builds tarball}"
GITHUB_REPOSITORY="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY must be set}"

CORPUS="crates/edtf-postgres/tests/corpus.sql"

if [[ ! -f ${TARBALL} ]]; then
  echo "::error::tarball not found: ${TARBALL}"
  exit 1
fi

# Newest prior release that actually shipped this cell's tarball. Sorted by
# semver, not release date: a backported patch must not masquerade as the
# newest upgrade origin. Draft releases are excluded — their assets are not
# downloadable and they are not published history.
PREV=""
TAGS=""
TAGS=$(gh release list --repo "${GITHUB_REPOSITORY}" \
  --exclude-drafts --limit 100 \
  --json tagName --jq '.[].tagName' | grep '^edtf-postgres-v' || true)

CANDIDATES=""
CANDIDATES=$(sort -rV <<< "${TAGS//edtf-postgres-v/}")

while read -r tag; do
  if [[ ${tag} == "${VERSION}" ]]; then
    continue
  fi
  # Only versions BELOW this one: re-dispatching an old tag must not try to
  # "upgrade" from a newer release.
  lowest=$(printf '%s\n%s\n' "${tag}" "${VERSION}" | sort -V | head -1)
  if [[ ${lowest} != "${tag}" ]]; then
    continue
  fi
  asset="edtf_postgres-${tag}-pg${PG}-linux-${ARCH}.tar.gz"
  HAS=""
  HAS=$(gh release view "edtf-postgres-v${tag}" --repo "${GITHUB_REPOSITORY}" \
    --json assets --jq '.assets[].name' 2> /dev/null) || HAS=""
  if grep -qxF "${asset}" <<< "${HAS}"; then
    PREV="${tag}"
    break
  fi
done <<< "${CANDIDATES}"

if [[ -z ${PREV} ]]; then
  echo "::notice::no prior release ships a pg${PG}/${ARCH} tarball; upgrade leg skipped"
  exit 0
fi

echo "::notice::upgrade test: ${PREV} -> ${VERSION} (pg${PG}, ${ARCH})"

prev_dir=$(mktemp -d)
# Bounded: gh has no transfer deadline of its own (v1.2.0 hang lesson).
timeout 300 gh release download "edtf-postgres-v${PREV}" --repo "${GITHUB_REPOSITORY}" \
  --pattern "edtf_postgres-${PREV}-pg${PG}-linux-${ARCH}.tar.gz" \
  --dir "${prev_dir}"

container="edtf-upgrade-pg${PG}-$$"

cleanup() {
  docker rm --force "${container}" > /dev/null 2>&1 || true
  rm -rf "${prev_dir}"
}
trap cleanup EXIT

# trixie, the newer distro: the fresh-install legs already cover both; the
# upgrade mechanics are distro-independent SQL script resolution.
UPGRADE_IMAGE=""
UPGRADE_IMAGE=$(base_image "${PG}" trixie)

docker run --detach \
  --name "${container}" \
  --env POSTGRES_PASSWORD=smoke \
  "${UPGRADE_IMAGE}" > /dev/null

READY="false"
for _ in $(seq 1 60); do
  if docker exec "${container}" pg_isready -U postgres > /dev/null 2>&1; then
    READY="true"
    break
  fi
  sleep 1
done

if [[ ${READY} != "true" ]]; then
  echo "::error::${UPGRADE_IMAGE} did not become ready"
  docker logs "${container}" 2>&1 | tail -20 || true
  exit 1
fi

# Install the PREVIOUS release and create the extension at its version —
# the same non-superuser shape as smoke-extension.sh.
docker cp "${prev_dir}/edtf_postgres-${PREV}-pg${PG}-linux-${ARCH}.tar.gz" \
  "${container}:/tmp/prev.tar.gz"
docker exec --user 0 "${container}" tar -xzf /tmp/prev.tar.gz -C /

docker exec "${container}" psql -U postgres -v ON_ERROR_STOP=1 -q -c \
  "CREATE ROLE smoke LOGIN PASSWORD 'smoke';"
docker exec "${container}" psql -U postgres -v ON_ERROR_STOP=1 -q -c \
  "CREATE DATABASE smokedb OWNER smoke;"
docker exec "${container}" psql -U smoke -d smokedb -v ON_ERROR_STOP=1 -q -c \
  "CREATE EXTENSION edtf_postgres VERSION '${PREV}';"
echo "ok  ${PREV} installed and created"

# THIS build over it — exactly what a consumer's image bump does — then the
# UPDATE. No target version: the new control file's default_version is the
# claim under test.
docker cp "${TARBALL}" "${container}:/tmp/next.tar.gz"
docker exec --user 0 "${container}" tar -xzf /tmp/next.tar.gz -C /

docker exec "${container}" psql -U smoke -d smokedb -v ON_ERROR_STOP=1 -q -c \
  "ALTER EXTENSION edtf_postgres UPDATE;"

UPGRADED=""
UPGRADED=$(docker exec "${container}" psql -U smoke -d smokedb -tAc \
  "SELECT extversion FROM pg_extension WHERE extname = 'edtf_postgres';")

if [[ ${UPGRADED} != "${VERSION}" ]]; then
  echo "::error::after ALTER EXTENSION UPDATE extversion is '${UPGRADED}', expected '${VERSION}'"
  echo "::error::the upgrade chain from ${PREV} did not reach this release"
  exit 1
fi
echo "ok  ALTER EXTENSION UPDATE reached ${UPGRADED}"

# The upgraded extension is held to the same standard as a fresh install:
# an upgrade that "succeeds" but leaves a broken function surface is the
# failure mode upgrade scripts exist to prevent.
docker cp "${CORPUS}" "${container}:/tmp/corpus.sql"
docker exec "${container}" psql -U smoke -d smokedb -v ON_ERROR_STOP=1 -q \
  -f /tmp/corpus.sql
echo "ok  shared corpus passes against the upgraded extension"
