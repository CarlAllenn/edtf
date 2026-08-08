#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) the edtf contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
# Install a built tarball into a clean Postgres and prove it works.
#
# This is the "prove before signing" step for this artifact class. The crates
# are proved by comparing their bytes against what the registry serves; a
# tarball has no registry to compare against, so the proof is installation.
# A tarball that was never installed is not a tested tarball.
#
# The container is a stock `postgres:<major>-<distro>` with NO build tooling
# — no Rust, no cargo-pgrx, no server headers. If anything the build image
# happened to provide were still needed at runtime, it fails here rather
# than for a stranger.
#
# Each assertion exists because the thing it checks can fail silently, and
# every claim the control file or the docs make is checked by one of them:
#
#   1. CREATE EXTENSION as a NON-SUPERUSER. `trusted = true` in the control
#      file claims a user with CREATE on the database can install this, and
#      nothing else tests that claim. Postgres runs a trusted extension's
#      script as the bootstrap superuser for such a user, which is both the
#      usability win and the reason the setting carries weight. If it fails
#      here, the control file asserts something false to precisely the
#      managed-Postgres audience most likely to rely on it.
#
#   2. extversion == the release version — proving cargo-pgrx substituted
#      @CARGO_VERSION@ and the control file agrees with the shipped schema.
#
#   3. An upgrade path Postgres can actually see. assert-upgrade-path.sh
#      checks the graph in the repository; this checks the scripts SHIPPED
#      inside the tarball and that Postgres can chain them.
#
#   4. The shared corpus (tests/corpus.sql), the same assertions the pgrx
#      suite runs, so the binary is held to the standard of the source.
#
#   5. The LIBRARY's own version (pg18+), which is the only check that
#      reaches the .so rather than the control file.
#
#   6. `relocatable = true` — that SET SCHEMA really works.
#
#   7. Distro reach. Built on bookworm (glibc 2.36) and run here on both
#      bookworm and trixie (2.41). Testing only where it was built would
#      assume glibc forward compatibility rather than demonstrate it.
set -euo pipefail

# shellcheck source=.github/scripts/base-images.sh
. "$(dirname "${BASH_SOURCE[0]}")/base-images.sh"

PG="${PG:?PG must be set (e.g. 18)}"
VERSION="${VERSION:?VERSION must be set}"
TARBALL="${TARBALL:?TARBALL must be set}"
DISTRO="${DISTRO:?DISTRO must be set (bookworm or trixie)}"

CORPUS="crates/edtf-postgres/tests/corpus.sql"

if [[ ! -f ${TARBALL} ]]; then
  echo "::error::tarball not found: ${TARBALL}"
  exit 1
fi
if [[ ! -f ${CORPUS} ]]; then
  echo "::error::corpus not found: ${CORPUS}"
  exit 1
fi

SMOKE_IMAGE=""
SMOKE_IMAGE=$(base_image "${PG}" "${DISTRO}")

container="edtf-smoke-pg${PG}-${DISTRO}-$$"

cleanup() {
  docker rm --force "${container}" > /dev/null 2>&1 || true
}
trap cleanup EXIT

echo "::notice::smoke test: pg${PG} on ${DISTRO}"

# No --rm: the EXIT trap already removes the container, and --rm would race
# it away before the `docker logs` diagnostic below could print anything —
# so a startup failure would report nothing at all.
docker run --detach \
  --name "${container}" \
  --env POSTGRES_PASSWORD=smoke \
  "${SMOKE_IMAGE}" > /dev/null

READY="false"
for _ in $(seq 1 60); do
  if docker exec "${container}" pg_isready -U postgres > /dev/null 2>&1; then
    READY="true"
    break
  fi
  sleep 1
done

if [[ ${READY} != "true" ]]; then
  echo "::error::${SMOKE_IMAGE} did not become ready"
  docker logs "${container}" 2>&1 | tail -20 || true
  exit 1
fi

# Install exactly as the documented instructions say to: untar into /.
docker cp "${TARBALL}" "${container}:/tmp/ext.tar.gz"
docker exec --user 0 "${container}" tar -xzf /tmp/ext.tar.gz -C /

# A plain role, owning its own database: CREATE on the database and nothing
# more. Deliberately NOT superuser — that is the point of the check.
docker exec "${container}" psql -U postgres -v ON_ERROR_STOP=1 -q -c \
  "CREATE ROLE smoke LOGIN PASSWORD 'smoke';"
docker exec "${container}" psql -U postgres -v ON_ERROR_STOP=1 -q -c \
  "CREATE DATABASE smokedb OWNER smoke;"

docker exec "${container}" psql -U smoke -d smokedb -v ON_ERROR_STOP=1 -q -c \
  "CREATE EXTENSION edtf_postgres;"
echo "ok  CREATE EXTENSION as a non-superuser (trusted = true holds)"

INSTALLED=""
INSTALLED=$(docker exec "${container}" psql -U smoke -d smokedb -tAc \
  "SELECT extversion FROM pg_extension WHERE extname = 'edtf_postgres';")

if [[ ${INSTALLED} != "${VERSION}" ]]; then
  echo "::error::installed extversion is '${INSTALLED}', expected '${VERSION}'"
  echo "::error::the control file and the shipped schema disagree about the version"
  exit 1
fi
# What this actually proves: cargo-pgrx substituted @CARGO_VERSION@ with the
# crate version, and the tarball's control file and edtf_postgres--<v>.sql
# agree. It does NOT prove the .so is this release's build — extversion is
# read from the control file, not from the library. pg_get_loaded_modules
# below is the check that reaches the binary.
echo "ok  extversion ${INSTALLED} matches the release"

# The upgrade path, exercised by Postgres rather than asserted by filename.
# assert-upgrade-path.sh checks the graph in the repository; this checks that
# the scripts actually SHIPPED inside the tarball and that Postgres can see
# and chain them. pg_extension_update_paths reads the installed .sql files,
# so a missing or misnamed script shows up as a NULL path.
#
# Applying a real ALTER EXTENSION UPDATE needs the PREVIOUS release's
# tarball installed first — that leg is upgrade-smoke-extension.sh, which
# runs after this script in the release matrix.
PATHS=""
PATHS=$(docker exec "${container}" psql -U smoke -d smokedb -tAc \
  "SELECT count(*) FROM pg_extension_update_paths('edtf_postgres') WHERE path IS NOT NULL;")

if [[ -z ${PATHS} || ${PATHS} -lt 1 ]]; then
  echo "::error::no usable ALTER EXTENSION UPDATE path is visible to Postgres"
  echo "::error::the upgrade scripts did not ship in the tarball, or are misnamed"
  docker exec "${container}" psql -U smoke -d smokedb \
    -c "SELECT * FROM pg_extension_update_paths('edtf_postgres');" || true
  exit 1
fi
echo "ok  ${PATHS} upgrade path(s) shipped and resolvable by Postgres"

docker cp "${CORPUS}" "${container}:/tmp/corpus.sql"
docker exec "${container}" psql -U smoke -d smokedb -v ON_ERROR_STOP=1 -q \
  -f /tmp/corpus.sql
echo "ok  shared corpus passes against the installed extension"

# AFTER the corpus, deliberately. pg_get_loaded_modules() reports modules
# actually dlopen'd into the backend, and CREATE EXTENSION alone does not
# load the library — it is loaded lazily on the first function call, so this
# returns nothing until the corpus has exercised one.
#
# Postgres 18+ only (verified: the function is absent from pg_proc on 14).
# It is the only check that reads a version out of the LIBRARY rather than
# out of the control file, which is what makes it worth the version gate:
# extversion would still match if the .so were from another build entirely.
if [[ ${PG} -ge 18 ]]; then
  # Both statements in ONE session: every `docker exec psql` is a fresh
  # backend, and a module loaded by the corpus run is gone by the next
  # connection. Call a function to force the dlopen, then read the registry.
  #
  # `edtf-postgres`, with a hyphen: pg_module_magic!(name, version) registers
  # the CRATE name, while the extension — and the .so — are edtf_postgres
  # with an underscore. The two differ by one character and nothing else in
  # the pipeline would notice.
  LOADED=""
  LOADED=$(
    docker exec -i "${container}" psql -U smoke -d smokedb -tA << 'SQL' | tail -1
SELECT edtf_valid('1985');
SELECT version FROM pg_get_loaded_modules() WHERE module_name = 'edtf-postgres';
SQL
  )

  if [[ ${LOADED} != "${VERSION}" ]]; then
    echo "::error::loaded module reports version '${LOADED:-none}', expected '${VERSION}'"
    echo "::error::the shared library is not this release's build"
    exit 1
  fi
  echo "ok  loaded module reports ${LOADED} — the .so is this release's build"
else
  echo "::notice::pg${PG} has no pg_get_loaded_modules(); library-version check skipped"
fi

# `relocatable = true` in the control file is a promise that the extension
# can be moved between schemas. Nothing else tests it, and a promise the
# extension cannot keep is worse than not making it — a user who relies on
# it discovers the truth at ALTER EXTENSION time, in their own database.
#
# Run as the SUPERUSER, not as `smoke`. That is a direct consequence of
# `trusted = true`: Postgres runs a trusted extension's install script as the
# bootstrap superuser, so the functions are owned by `postgres` even though
# `smoke` issued the CREATE EXTENSION. `smoke` therefore cannot relocate
# them — "must be owner of function edtf_canonical". Installing as a
# non-superuser and relocating as the owner is the real-world shape.
docker exec "${container}" psql -U postgres -d smokedb -v ON_ERROR_STOP=1 -q -c \
  "CREATE SCHEMA edtf; ALTER EXTENSION edtf_postgres SET SCHEMA edtf;"
RELOCATED=""
RELOCATED=$(docker exec "${container}" psql -U postgres -d smokedb -tAc \
  "SELECT edtf.edtf_valid('1985-04-12');")

if [[ ${RELOCATED} != "t" ]]; then
  echo "::error::after SET SCHEMA the functions do not resolve in the new schema"
  exit 1
fi
echo "ok  relocatable — SET SCHEMA works and the functions follow"

echo "::notice::pg${PG} on ${DISTRO}: installed, trusted, versioned, conformant"
