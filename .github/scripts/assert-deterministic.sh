#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) the edtf contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
# Prove `cargo package` produces identical bytes twice in a row.
#
# The attestation names the files in dist/, but `cargo publish` re-packages
# from source rather than uploading those files. That is only safe if
# packaging is reproducible: if it is not, the signature would describe
# bytes that differ from what shipped — a subtler version of the v1.0.0
# defect. The registry comparison later catches a mismatch too, but by then
# the crate is published and unrecoverable; this fails first, before any
# upload.
set -euo pipefail

RUNNER_TEMP="${RUNNER_TEMP:?RUNNER_TEMP must be set}"

first="${RUNNER_TEMP}/digests-1.txt"
second="${RUNNER_TEMP}/digests-2.txt"

(cd dist && sha256sum ./*.crate | sort) > "${first}"

# Repackage from scratch — package-all.sh clears its own output first.
.github/scripts/package-all.sh

(cd dist && sha256sum ./*.crate | sort) > "${second}"

if ! diff -u "${first}" "${second}"; then
  echo "::error::cargo package is not reproducible across runs — refusing to attest"
  exit 1
fi

echo "::notice::packaging is deterministic"
cat "${second}"
