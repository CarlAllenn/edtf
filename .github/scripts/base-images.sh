#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) the edtf contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
# The digest-pinned container images every release-path script builds and
# tests in. Sourced, never executed on its own: callers get `base_image`.
#
# Why digests and not tags (issue #83, gap 3): these images are BUILD
# INPUTS to signed artifacts. Every other input is pinned — Rust and
# cargo-pgrx via mise.lock, rustup-init by per-arch SHA256 — and a floating
# `postgres:<major>-bookworm` was the one exception: the base could change
# between the run that tested and the run that shipped, underneath a
# pipeline whose whole design is "prove, then sign".
#
# A pinned-but-never-updated digest would be worse than a floating tag,
# which is why pinning waited for automation: the custom manager in
# renovate.json (customManagers, "pinned base images") matches the
# `<name>:<tag>@sha256:<digest>` strings below and keeps each digest
# current as Docker Hub republishes the tag. The digest is the MANIFEST
# LIST digest, so one pin serves both amd64 and arm64 runners natively.
#
# One table, one lookup: build-extension.sh, smoke-extension.sh and
# upgrade-smoke-extension.sh all resolve through here, so the image that
# builds the extension and the images that prove it can never drift apart
# in what they pin.
#
# The OCI image's own base (docker/edtf-postgres.Dockerfile) deliberately
# does NOT resolve through this table — its header explains why its base
# floats at build time (build-stage consumers inherit none of its layers).

# Usage: base_image <pg-major> <distro>  →  pinned reference on stdout.
# Unknown pairs are a hard error: a caller asking for an unpinned image is
# a matrix/table mismatch, and echoing a floating tag would silently
# reintroduce the defect this file exists to close.
base_image() {
  local pg="${1:?pg major required}"
  local distro="${2:?distro required}"
  case "${pg}-${distro}" in
    14-bookworm) echo "postgres:14-bookworm@sha256:f819dd96de74db3de87d13f0a4fbcdfbdc4f4ca37ed6e0807bf73848170eae3d" ;;
    15-bookworm) echo "postgres:15-bookworm@sha256:e8db9bd3e9e1751eb639fb17be53cc6d1b62a322adf75b99e791767a7a16ce69" ;;
    16-bookworm) echo "postgres:16-bookworm@sha256:64154d0babcb1741988719e703419af0382b19953706149f9872fbd0f438efa8" ;;
    17-bookworm) echo "postgres:17-bookworm@sha256:9b18b78397054fce88a9552e9d5a3ad5bb7fd258c5b3cc1c5028e46373d6ea8f" ;;
    18-bookworm) echo "postgres:18-bookworm@sha256:882236b897e39051d2368c5ccc6cda944904723506b2dfc97f2a8f5bc9afa382" ;;
    14-trixie) echo "postgres:14-trixie@sha256:2f439458ab6a57a925825ae14f9d06910e4fe4a41c8d4a0ae06397e65b707e1b" ;;
    15-trixie) echo "postgres:15-trixie@sha256:6eb0add3b77c081df18aa518ce43df58fdcc40f2e6d868a6fd08038dc7acd425" ;;
    16-trixie) echo "postgres:16-trixie@sha256:95206741a5b214807675e14165369d05b93a9cf692223b616d07cca227e74b0b" ;;
    17-trixie) echo "postgres:17-trixie@sha256:7958605b474b3d264a969cb3a123d6aa00ad1e1fe9da8a69984dabb704d93317" ;;
    18-trixie) echo "postgres:18-trixie@sha256:a02db8cac496f15b094798a38254f14d6e00741f709360e5e00bb6668ea31636" ;;
    *)
      echo "::error::no pinned base image for pg${pg} on ${distro} — extend base-images.sh" >&2
      return 1
      ;;
  esac
}

# Wait until the container's Postgres is REALLY ready — used by every
# release-path script that boots one of the images above.
#
# `pg_isready` alone is a race: the official postgres entrypoint starts a
# temporary server for init, stops it, and starts the real one. A probe
# that lands on the temporary server reports success, and the next psql
# then dies with "FATAL: the database system is starting up" — which is
# exactly how the v1.2.1 release lost its first pg17 smoke cell. Readiness
# is therefore three consecutive pg_isready successes topped by an actual
# query, which only the final server can answer.
wait_for_postgres() {
  local container="$1"
  local streak=0
  for _ in $(seq 1 90); do
    if docker exec "${container}" pg_isready -U postgres > /dev/null 2>&1; then
      streak=$((streak + 1))
    else
      streak=0
    fi
    if [[ ${streak} -ge 3 ]] \
      && docker exec "${container}" psql -U postgres -qAtc 'SELECT 1' > /dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done
  echo "::error::postgres in ${container} did not become ready" >&2
  docker logs "${container}" 2>&1 | tail -20 || true
  return 1
}
