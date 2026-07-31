#!/usr/bin/env bash
# Fail if edtf-postgres has drifted from the workspace version group.
#
# edtf-postgres cannot be a workspace member — Cargo only honours [profile]
# at the workspace root, so joining would silently impose the root's
# `opt-level = "z"` and `strip = "symbols"` on a Postgres extension. It
# therefore rides the unified version number by hand, bumped in the Release
# PR alongside the five crates release-plz manages.
#
# A forgotten hand-bump used to be silent: the old release-time guard saw
# the previous version already on crates.io, printed "already published;
# skipping", and exited GREEN — which is how edtf-postgres went untagged and
# unreleased at v1.0.0 while the release reported success. Running this on
# every PR turns that into a red check on the bump itself, hours before a
# release could act on it (issue #54).
set -euo pipefail

workspace_version=$(cargo pkgid --manifest-path crates/edtf-core/Cargo.toml | sed 's/.*[@#]//')
pg_version=$(cargo pkgid --manifest-path crates/edtf-postgres/Cargo.toml | sed 's/.*[@#]//')

if [[ ${pg_version} != "${workspace_version}" ]]; then
  echo "error: edtf-postgres is ${pg_version}, workspace is ${workspace_version}" >&2
  echo "       edtf-postgres rides the unified version and is bumped by hand:" >&2
  echo "       set version = \"${workspace_version}\" in crates/edtf-postgres/Cargo.toml" >&2
  exit 1
fi

# The five workspace crates share a version_group in release-plz.toml, so
# they should never disagree either — but the group is only applied when
# release-plz runs, and a hand-authored bump PR can miss one.
for name in edtf-calendars edtf-normalize edtf-wasm edtf-cli; do
  version=$(cargo pkgid --manifest-path "crates/${name}/Cargo.toml" | sed 's/.*[@#]//')
  if [[ ${version} != "${workspace_version}" ]]; then
    echo "error: ${name} is ${version}, workspace is ${workspace_version}" >&2
    exit 1
  fi
done

echo "all six crates at ${workspace_version}"
