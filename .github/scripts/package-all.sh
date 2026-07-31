#!/usr/bin/env bash
# Package all six crates into ./dist, explicitly and reproducibly.
#
# Shared by publish.yml's package step and its determinism assertion, which
# runs this twice and diffs the digests — so it must be idempotent and must
# leave nothing behind from a previous run.
#
# Explicit filenames, never `cp target/package/*.crate`: cargo keeps stale
# .crate files from earlier versions in target/package (observed locally:
# 0.1.0 and 0.2.0 artifacts sitting alongside 1.0.0). A glob would copy
# those into dist, and dist is what gets attested — signing artifacts from
# a version this release is not publishing.
set -euo pipefail

VERSION="${VERSION:?VERSION must be set}"

WORKSPACE_CRATES=(edtf-core edtf-calendars edtf-normalize edtf-wasm edtf-cli)
PG_MANIFEST="crates/edtf-postgres/Cargo.toml"

# Clear prior output so a re-run cannot inherit anything.
rm -rf dist target/package crates/edtf-postgres/target/package
mkdir -p dist

# Resolves publish order itself (core -> calendars/normalize -> wasm/cli).
cargo package --workspace

# --no-verify: the verify build runs pgrx's build script, which needs an
# initialized $PGRX_HOME that this runner does not have. ci.yml's postgres
# job runs `cargo package` WITH the verify build on a runner that does, so
# this is backed by a real check rather than assumed (issue #54).
cargo package --no-verify --manifest-path "${PG_MANIFEST}"

for name in "${WORKSPACE_CRATES[@]}"; do
  src="target/package/${name}-${VERSION}.crate"
  if [[ ! -f ${src} ]]; then
    echo "::error::expected ${src} to exist"
    exit 1
  fi
  cp "${src}" "dist/${name}-${VERSION}.crate"
done

pg_src="crates/edtf-postgres/target/package/edtf-postgres-${VERSION}.crate"
if [[ ! -f ${pg_src} ]]; then
  echo "::error::expected ${pg_src} to exist"
  exit 1
fi
cp "${pg_src}" "dist/edtf-postgres-${VERSION}.crate"

# Exactly six, nothing else.
count=$(find dist -name '*.crate' -type f | wc -l | tr -d ' ')
if [[ ${count} -ne 6 ]]; then
  echo "::error::expected 6 crates in dist, found ${count}"
  exit 1
fi
