#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) the edtf contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
# Consume the just-published artifacts the way a stranger would.
#
# Everything before this proves the bytes are what we built and signed. This
# proves they actually work: resolved from the registry, in a scratch
# CARGO_HOME so nothing is satisfied from this run's own caches or the local
# workspace. A crate can upload perfectly and still be unusable — a missing
# file in the package, a path dependency that did not translate to a version
# requirement — and that failure only ever shows up on a fresh consumer.
set -euo pipefail

VERSION="${VERSION:?VERSION must be set}"

scratch=$(mktemp -d)
trap 'rm -rf "${scratch}"' EXIT

# Scratch CARGO_HOME: forces a real registry fetch rather than reusing
# anything this job already has.
export CARGO_HOME="${scratch}/cargo"

# Library crates: resolve and compile against the published versions.
cargo new --lib "${scratch}/probe" > /dev/null
(
  cd "${scratch}/probe"
  cargo add "edtf-core@=${VERSION}" > /dev/null
  cargo add "edtf-calendars@=${VERSION}" > /dev/null
  cargo add "edtf-normalize@=${VERSION}" > /dev/null
  cargo build --quiet
)
echo "ok  edtf-core, edtf-calendars, edtf-normalize resolve and compile"

# Binary crate: the [[bin]] is named `edtf`, not `edtf-cli`.
cargo install "edtf-cli@${VERSION}" --root "${scratch}/cli" --quiet
"${scratch}/cli/bin/edtf" --version
echo "ok  edtf-cli installs and runs"

# npm package: fetch the published tarball as a consumer would.
npm pack "edtf-wasm@${VERSION}" --pack-destination "${scratch}" > /dev/null
echo "ok  edtf-wasm fetches from npm"

# edtf-postgres is not exercised here, but it is no longer unexercised.
# Consuming it needs a running Postgres, which this script has no business
# starting, so its canary is a separate step: canary-extension.sh downloads
# the tarball attached to the release, checks it against SHA256SUMS and
# installs it into a clean Postgres. That runs after the assets are
# attached, which is necessarily later than this script (issue #55).

echo "::notice::canary passed — published artifacts consume cleanly"
