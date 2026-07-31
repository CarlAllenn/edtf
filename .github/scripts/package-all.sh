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

PURE_RUST_CRATES=(edtf-core edtf-calendars edtf-normalize edtf-wasm edtf-cli)

# Clear prior output so a re-run cannot inherit anything.
rm -rf dist target/package
mkdir -p dist

# The five pure-Rust crates, verify build included.
cargo package --workspace --exclude edtf-postgres

# edtf-postgres with --no-verify: the verify build runs pgrx's build script,
# which needs an initialized $PGRX_HOME that this runner does not have.
# ci.yml's postgres job runs `cargo package` WITH the verify build on runners
# that do — against every Postgres major — so this is backed by a real check
# rather than assumed (issue #54).
#
# edtf-core is named alongside it even though it was packaged above.
# --no-verify skips the BUILD, not the dependency resolution: preparing the
# package still resolves `edtf-core = ^<version>` from the registry, and on a
# release that version does not exist there yet — the release being what
# creates it. Naming both makes cargo resolve against a temporary registry
# built from the sibling. Re-packaging edtf-core is harmless: packaging is
# deterministic (the step after this proves it), so the .crate is identical.
cargo package -p edtf-core -p edtf-postgres --no-verify

# One workspace now, so every .crate lands in the same target/package.
for name in "${PURE_RUST_CRATES[@]}" edtf-postgres; do
  src="target/package/${name}-${VERSION}.crate"
  if [[ ! -f ${src} ]]; then
    echo "::error::expected ${src} to exist"
    exit 1
  fi
  cp "${src}" "dist/${name}-${VERSION}.crate"
done

# Exactly six, nothing else.
count=$(find dist -name '*.crate' -type f | wc -l | tr -d ' ')
if [[ ${count} -ne 6 ]]; then
  echo "::error::expected 6 crates in dist, found ${count}"
  exit 1
fi
