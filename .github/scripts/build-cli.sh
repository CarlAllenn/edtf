#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) the edtf contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
# Build, test and package the edtf-cli binary for the runner's native
# target (issue #84). Native hardware per platform, no cross-compilation:
# a binary released for a target must have had its tests RUN on that
# target, or the release asserts something no CI leg ever demonstrated.
#
# The test step is the whole cli.rs suite, in release mode, against the
# same binary that ships — assert_cmd drives target/release/edtf, so the
# packaged bytes are the tested bytes.
set -euo pipefail

VERSION="${VERSION:?VERSION must be set}"
TARGET="${TARGET:?TARGET must be set}"
OUT_DIR="${OUT_DIR:?OUT_DIR must be set}"

cargo build --release --locked -p edtf-cli
cargo test --release --locked -p edtf-cli

# The host must BE the target — this script never cross-compiles, and a
# silent host/target mismatch would package an untested binary under a
# tested name.
HOST=$(rustc --version --verbose | sed -n 's/^host: //p')
if [[ ${HOST} != "${TARGET}" ]]; then
  echo "::error::runner host is ${HOST} but the matrix says ${TARGET}"
  exit 1
fi

mkdir -p "${OUT_DIR}"

# GNU tar everywhere: the normalised-metadata flags (--sort, --mtime,
# --owner) are GNU extensions, and the macOS images ship BSD tar as `tar`
# with GNU tar beside it as `gtar`. Asserted rather than assumed — BSD tar
# rejects the flags, so a runner without GNU tar must fail here, by name,
# not in the middle of the tar invocation.
TAR=tar
if command -v gtar > /dev/null; then
  TAR=gtar
fi
if ! "${TAR}" --version | grep -q 'GNU tar'; then
  echo "::error::GNU tar is required (found neither gtar nor a GNU tar)"
  exit 1
fi

# Same normalisation as the extension tarballs: fixed ownership, sorted
# entries, epoch mtimes. The binary is not claimed bit-reproducible, but
# nothing about the ARCHIVE should vary run to run for reasons unrelated
# to its contents.
"${TAR}" --create --gzip \
  --owner=0 --group=0 --numeric-owner \
  --sort=name --mtime=@0 \
  --file "${OUT_DIR}/edtf-cli-${VERSION}-${TARGET}.tar.gz" \
  --directory target/release edtf

echo "::notice::built edtf-cli-${VERSION}-${TARGET}.tar.gz"
