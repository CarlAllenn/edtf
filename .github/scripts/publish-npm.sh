#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) the edtf contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
# Publish the edtf-wasm npm package, resumably.
#
# npm trusted publisher: OIDC, provenance generated automatically for public
# packages. Skips if the version is already on the registry so the tag can
# be re-dispatched after a partial release.
set -euo pipefail

VERSION="${VERSION:?VERSION must be set}"

if [[ "$(npm view "edtf-wasm@${VERSION}" version 2> /dev/null || true)" == "${VERSION}" ]]; then
  echo "::notice::edtf-wasm ${VERSION} already on npm; skipping"
else
  echo "publishing edtf-wasm ${VERSION} to npm"
  npm publish crates/edtf-wasm/pkg/
fi

# Confirm it is actually there, whether this run put it there or a prior one
# did — the release must not proceed to attestation on a half-published set.
# Polled, not checked once: at v1.0.1 the publish succeeded and the very
# next `npm view` still missed it — the registry read path lags the write
# path by a few seconds, and that lag failed the release (issue #66).
for _ in $(seq 1 12); do
  if [[ "$(npm view "edtf-wasm@${VERSION}" version 2> /dev/null || true)" == "${VERSION}" ]]; then
    echo "::notice::edtf-wasm ${VERSION} present on npm"
    exit 0
  fi
  sleep 10
done

echo "::error::edtf-wasm ${VERSION} is not on npm after publish"
exit 1
