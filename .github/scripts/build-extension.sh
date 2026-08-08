#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) the edtf contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
# Build the prebuilt extension tarball for one Postgres major, inside the
# Postgres image it targets.
#
# Two defects, one fix.
#
# `pg_config`: ci.yml builds against pgrx's own downloaded Postgres
# (`cargo pgrx init --pg18 download`), which nobody runs. The first consumer
# runs a pgdg build. Package from the wrong pg_config and the result builds
# green, tars cleanly, and fails at CREATE EXTENSION for exactly the person
# these artifacts exist to serve. Building inside `postgres:<major>-bookworm`
# makes the right pg_config structural rather than remembered.
#
# glibc: an extension is a shared library dlopen'd into the server process,
# so the glibc it was linked against is a FLOOR on where it can run. Building
# on the runner directly (ubuntu-24.04, glibc 2.39) sets that floor as high
# as it goes. bookworm is 2.36, which covers Debian 12 and 13 both — and
# trixie, which most people actually run, is 2.41. Hence bookworm and not
# trixie: build against the oldest supported libc, test on the newest.
#
# `docker run` as a STEP, never a job-level `container:`. harden-runner needs
# kernel capabilities that are unavailable inside an unprivileged container
# and does not support containerised jobs; `egress-policy: block` as the
# first step of every job is canon here.
#
# The image tag is floating rather than digest-pinned. Digest-pinning five
# majors needs a Renovate regex manager in the shared config to keep them
# current, and a pinned-but-never-updated base image is worse than a floating
# one. Tracked as follow-up (issue #55).
set -euo pipefail

PG="${PG:?PG must be set (e.g. 18)}"
VERSION="${VERSION:?VERSION must be set}"
OUT_DIR="${OUT_DIR:?OUT_DIR must be set}"

# Derived, never restated: the same pins the rest of the repository builds
# with. A second copy of a version number is a second thing to forget.
RUST_VERSION=""
RUST_VERSION=$(taplo get -f mise.toml 'tools.rust.version')
PGRX_VERSION=""
PGRX_VERSION=$(taplo get -f mise.toml 'tools."cargo:cargo-pgrx"')

if [[ -z ${RUST_VERSION} || -z ${PGRX_VERSION} ]]; then
  echo "::error::could not resolve the rust / cargo-pgrx pins from mise.toml"
  exit 1
fi

mkdir -p "${OUT_DIR}"
abs_out=$(cd "${OUT_DIR}" && pwd)

echo "::notice::building pg${PG} in postgres:${PG}-bookworm (rust ${RUST_VERSION}, cargo-pgrx ${PGRX_VERSION})"

# The source is mounted read-only and copied to /build: the container must
# not be able to write into the checkout, and copying only the workspace
# files keeps target/ out of the container entirely.
docker run --rm \
  --env PG="${PG}" \
  --env VERSION="${VERSION}" \
  --env RUST_VERSION="${RUST_VERSION}" \
  --env PGRX_VERSION="${PGRX_VERSION}" \
  --volume "${PWD}:/src:ro" \
  --volume "${abs_out}:/out" \
  "postgres:${PG}-bookworm" \
  bash -c '
    set -euo pipefail
    mkdir -p /build
    cp -a /src/Cargo.toml /src/Cargo.lock /src/crates /src/.cargo /build/
    exec /src/.github/scripts/build-extension-inner.sh
  '
