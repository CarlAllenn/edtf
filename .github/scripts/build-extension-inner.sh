#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) the edtf contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
# Runs INSIDE postgres:<major>-bookworm. Driven by build-extension.sh, which
# explains why the build happens in a container at all.
#
# Everything here is deliberately derived rather than restated: the Rust and
# cargo-pgrx versions arrive from mise.toml via the caller, so the container
# builds with the same pins as every other job.
set -euo pipefail

PG="${PG:?PG must be set}"
VERSION="${VERSION:?VERSION must be set}"
RUST_VERSION="${RUST_VERSION:?RUST_VERSION must be set}"
PGRX_VERSION="${PGRX_VERSION:?PGRX_VERSION must be set}"

export DEBIAN_FRONTEND=noninteractive

# postgresql-server-dev-N brings the server headers, which the runtime image
# does not carry (`pg_config` is present, `postgres.h` is not). It comes from
# the pgdg repository the image already has configured — which is the whole
# point: these are the headers of the Postgres people actually run.
apt-get update -qq
apt-get install -y -qq --no-install-recommends \
  "postgresql-server-dev-${PG}" \
  build-essential ca-certificates curl pkg-config libclang-dev git

# rustup-init pinned by version and per-architecture SHA256, not the
# `curl | sh` installer: sh.rustup.rs serves whatever is current, so a pipe
# from it executes unpinned code inside the build. The checksums are the
# published rustup-init.sha256 values for this version; a new rustup means
# updating all three lines together.
RUSTUP_VERSION=1.29.0
RUSTUP_SHA256_AMD64=4acc9acc76d5079515b46346a485974457b5a79893cfb01112423c89aeb5aa10
RUSTUP_SHA256_ARM64=9732d6c5e2a098d3521fca8145d826ae0aaa067ef2385ead08e6feac88fa5792

DEB_ARCH=$(dpkg --print-architecture)
case "${DEB_ARCH}" in
  amd64) RUSTUP_TARGET=x86_64-unknown-linux-gnu RUSTUP_SHA256=${RUSTUP_SHA256_AMD64} ;;
  arm64) RUSTUP_TARGET=aarch64-unknown-linux-gnu RUSTUP_SHA256=${RUSTUP_SHA256_ARM64} ;;
  *)
    echo "::error::unexpected architecture ${DEB_ARCH}"
    exit 1
    ;;
esac

curl --proto '=https' --tlsv1.2 -sSf \
  -o /tmp/rustup-init \
  "https://static.rust-lang.org/rustup/archive/${RUSTUP_VERSION}/${RUSTUP_TARGET}/rustup-init"
echo "${RUSTUP_SHA256}  /tmp/rustup-init" | sha256sum --check --strict -
chmod +x /tmp/rustup-init
/tmp/rustup-init -y --profile minimal --default-toolchain "${RUST_VERSION}" --no-modify-path
rm /tmp/rustup-init

# rustup's env script is generated at install time and cannot be followed
# statically.
# shellcheck source=/dev/null
. "${HOME}/.cargo/env"

cargo install cargo-pgrx --version "${PGRX_VERSION}" --locked

# Register the SYSTEM Postgres rather than downloading and compiling one.
# This is the defect this whole job exists to prevent: ci.yml builds against
# pgrx's own `--pg18 download` Postgres, which nobody runs, while consumers
# run a pgdg build. Packaging from the wrong pg_config produces a tarball
# that builds green, tars cleanly, and fails at CREATE EXTENSION.
cargo pgrx init "--pg${PG}" /usr/bin/pg_config

cd /build/crates/edtf-postgres
cargo pgrx package \
  --no-default-features --features "pg${PG}" \
  --pg-config /usr/bin/pg_config \
  --out-dir /build/pkgroot

# dpkg's spelling (amd64/arm64), not uname's (x86_64/aarch64). The asset
# names are what people paste into Dockerfiles, and the mismatch between the
# two spellings is the classic install-snippet bug.
ARCH=$(dpkg --print-architecture)

# Split the debug info out of the shipped library, Debian-style (issue #83,
# gap 2). Postgres resolves the extension's functions with dlsym against the
# DYNAMIC symbol table (.dynsym), which stripping never touches — so the
# shipped .so loses nothing it needs. What stripping would cost is the
# backtrace when the extension crashes inside a backend, and that is what
# the -dbgsym tarball preserves: the full debug info, split into a sibling
# artifact, findable by gdb through both the .gnu_debuglink stamped into
# the stripped library and the /usr/lib/debug path mirror it unpacks into.
# The smoke tests downstream run against the STRIPPED tarball, so "stripped
# still loads, resolves and conforms" is proven every release, not assumed.
so_path=""
so_path=$(find /build/pkgroot -name 'edtf_postgres.so' -type f)
if [[ -z ${so_path} ]]; then
  echo "::error::edtf_postgres.so not found in the package root"
  exit 1
fi
so_rel="${so_path#/build/pkgroot/}"

dbg_dir="/build/dbgroot/usr/lib/debug/$(dirname "${so_rel}")"
mkdir -p "${dbg_dir}"
objcopy --only-keep-debug "${so_path}" "${dbg_dir}/edtf_postgres.so.debug"
chmod 0644 "${dbg_dir}/edtf_postgres.so.debug"
objcopy --strip-unneeded --remove-section=.comment \
  --add-gnu-debuglink="${dbg_dir}/edtf_postgres.so.debug" "${so_path}"

# Normalised metadata: fixed ownership, sorted entries, epoch mtimes. The
# compiled .so is not claimed to be bit-reproducible, but nothing about the
# ARCHIVE should vary run to run for reasons unrelated to its contents.
tar --create --gzip \
  --owner=0 --group=0 --numeric-owner \
  --sort=name --mtime=@0 \
  --file "/out/edtf_postgres-${VERSION}-pg${PG}-linux-${ARCH}.tar.gz" \
  --directory /build/pkgroot .

tar --create --gzip \
  --owner=0 --group=0 --numeric-owner \
  --sort=name --mtime=@0 \
  --file "/out/edtf_postgres-dbgsym-${VERSION}-pg${PG}-linux-${ARCH}.tar.gz" \
  --directory /build/dbgroot .

echo "::notice::built edtf_postgres-${VERSION}-pg${PG}-linux-${ARCH}.tar.gz (+ dbgsym)"
