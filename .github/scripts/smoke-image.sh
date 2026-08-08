#!/usr/bin/env bash
# Boot the edtf-postgres image and prove the extension works (issue #82).
#
# The image-leg sibling of smoke-extension.sh: that script proves a tarball
# installs into a stock postgres; this one proves the image the tarball was
# baked into actually serves the extension. Runs twice per architecture —
# against the locally built candidate before any push, and against the
# published bytes pulled back by digest — so the attestation asserts
# something demonstrated of what a stranger pulls.
#
# The assertions are the load-bearing subset of smoke-extension.sh's:
# non-superuser CREATE EXTENSION (trusted = true), extversion == release,
# and the shared corpus. The upgrade-path, loaded-module and relocatable
# checks stay in smoke-extension.sh — they are properties of the tarball,
# proved before it was released, and the image install path is the same
# `tar -C /` those checks already covered.
set -euo pipefail

IMAGE_REF="${1:?usage: smoke-image.sh <image-ref>}"
VERSION="${VERSION:?VERSION must be set}"

CORPUS="crates/edtf-postgres/tests/corpus.sql"

if [[ ! -f ${CORPUS} ]]; then
  echo "::error::corpus not found: ${CORPUS}"
  exit 1
fi

container="edtf-image-smoke-$$"

cleanup() {
  docker rm --force "${container}" > /dev/null 2>&1 || true
}
trap cleanup EXIT

echo "::notice::image smoke test: ${IMAGE_REF}"

# No --rm: the EXIT trap already removes the container, and --rm would race
# it away before the `docker logs` diagnostic below could print anything.
docker run --detach \
  --name "${container}" \
  --env POSTGRES_PASSWORD=smoke \
  "${IMAGE_REF}" > /dev/null

READY="false"
for _ in $(seq 1 60); do
  if docker exec "${container}" pg_isready -U postgres > /dev/null 2>&1; then
    READY="true"
    break
  fi
  sleep 1
done

if [[ ${READY} != "true" ]]; then
  echo "::error::${IMAGE_REF} did not become ready"
  docker logs "${container}" 2>&1 | tail -20 || true
  exit 1
fi

# A plain role, owning its own database: CREATE on the database and nothing
# more. Deliberately NOT superuser — trusted = true is the claim under test.
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
  exit 1
fi
echo "ok  extversion ${INSTALLED} matches the release"

docker cp "${CORPUS}" "${container}:/tmp/corpus.sql"
docker exec "${container}" psql -U smoke -d smokedb -v ON_ERROR_STOP=1 -q \
  -f /tmp/corpus.sql
echo "ok  shared corpus passes against the image's extension"
