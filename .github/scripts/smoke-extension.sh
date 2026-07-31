#!/usr/bin/env bash
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
# Four things are asserted, and each exists because it can fail silently:
#
#   1. CREATE EXTENSION as a NON-SUPERUSER. `trusted = true` in the control
#      file claims a user with CREATE on the database can install this, and
#      nothing else tests that claim. Postgres runs a trusted extension's
#      script as the bootstrap superuser for such a user, which is both the
#      usability win and the reason the setting carries weight. If it fails
#      here, the control file asserts something false to precisely the
#      managed-Postgres audience most likely to rely on it.
#
#   2. extversion == the release version. `pg_module_magic!(name, version)`
#      embeds the crate version, so this is what proves the .so is THIS
#      release's build rather than a stale artifact from a cache or the
#      wrong matrix cell — a failure that passes every functional test.
#
#   3. The shared corpus (tests/corpus.sql), the same assertions the pgrx
#      suite runs, so the binary is held to the standard of the source.
#
#   4. Distro reach. Built on bookworm (glibc 2.36) and run here on both
#      bookworm and trixie (2.41). Testing only where it was built would
#      assume glibc forward compatibility rather than demonstrate it.
set -euo pipefail

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

container="edtf-smoke-pg${PG}-${DISTRO}-$$"

cleanup() {
  docker rm --force "${container}" > /dev/null 2>&1 || true
}
trap cleanup EXIT

echo "::notice::smoke test: pg${PG} on ${DISTRO}"

docker run --detach --rm \
  --name "${container}" \
  --env POSTGRES_PASSWORD=smoke \
  "postgres:${PG}-${DISTRO}" > /dev/null

READY="false"
for _ in $(seq 1 60); do
  if docker exec "${container}" pg_isready -U postgres > /dev/null 2>&1; then
    READY="true"
    break
  fi
  sleep 1
done

if [[ ${READY} != "true" ]]; then
  echo "::error::postgres:${PG}-${DISTRO} did not become ready"
  docker logs "${container}" 2>&1 | tail -20
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
  echo "::error::the shared library does not belong to this release"
  exit 1
fi
echo "ok  extversion ${INSTALLED} matches the release"

docker cp "${CORPUS}" "${container}:/tmp/corpus.sql"
docker exec "${container}" psql -U smoke -d smokedb -v ON_ERROR_STOP=1 -q \
  -f /tmp/corpus.sql
echo "ok  shared corpus passes against the installed extension"

echo "::notice::pg${PG} on ${DISTRO}: installed, trusted, versioned, conformant"
