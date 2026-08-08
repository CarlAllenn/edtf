#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) the edtf contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
# Prove that what the registries now hold is byte-identical to what this run
# built, BEFORE anything is attested.
#
# This is the gate that makes the attestation honest. Attesting local files
# is only truthful if those files are what consumers will download; at
# v1.0.0 the two were never compared, and the attestation ended up
# describing a re-download rather than a build (issue #54).
#
# Adds the npm tarball to dist/ on success so the attest step covers it too.
set -euo pipefail

VERSION="${VERSION:?VERSION must be set}"
RUNNER_TEMP="${RUNNER_TEMP:?RUNNER_TEMP must be set}"

CRATES=(edtf-core edtf-calendars edtf-normalize edtf-wasm edtf-cli edtf-postgres)

fail=0

for name in "${CRATES[@]}"; do
  # Fetched to a file rather than piped into the parser: the pipe shape is
  # indistinguishable from download-then-run to scanners, and this is data,
  # not code — keep it looking like data.
  curl -sfA 'edtf-release-workflow' --retry 5 --retry-delay 10 --retry-all-errors \
    --max-time 120 \
    -o "${RUNNER_TEMP}/${name}.json" \
    "https://crates.io/api/v1/crates/${name}/${VERSION}"
  remote=$(python3 -c 'import sys,json;print(json.load(open(sys.argv[1]))["version"]["checksum"])' \
    "${RUNNER_TEMP}/${name}.json")
  built=$(sha256sum "dist/${name}-${VERSION}.crate" | cut -d' ' -f1)
  if [[ ${remote} != "${built}" ]]; then
    echo "::error::${name}: crates.io has ${remote}, this run built ${built}"
    fail=1
  else
    echo "ok  ${name}  ${built}"
  fi
done

# npm: pack the same directory that was published and compare against the
# tarball the registry actually serves. npm names it <name>-<version>.tgz,
# so the filename is derived rather than globbed.
npm pack crates/edtf-wasm/pkg/ --pack-destination "${RUNNER_TEMP}" > /dev/null
local_tgz="${RUNNER_TEMP}/edtf-wasm-${VERSION}.tgz"
published_tgz="${RUNNER_TEMP}/published.tgz"

curl -sfL --retry 5 --retry-delay 10 --retry-all-errors \
  --max-time 300 \
  -o "${published_tgz}" \
  "https://registry.npmjs.org/edtf-wasm/-/edtf-wasm-${VERSION}.tgz"

local_sum=$(sha256sum "${local_tgz}" | cut -d' ' -f1)
published_sum=$(sha256sum "${published_tgz}" | cut -d' ' -f1)

if [[ ${local_sum} != "${published_sum}" ]]; then
  echo "::error::edtf-wasm: npm has ${published_sum}, this run built ${local_sum}"
  fail=1
else
  echo "ok  edtf-wasm (npm)  ${local_sum}"
  cp "${local_tgz}" "dist/edtf-wasm-${VERSION}.tgz"
fi

if [[ ${fail} -ne 0 ]]; then
  echo "::error::registry bytes do not match this run's build — refusing to attest"
  exit 1
fi
