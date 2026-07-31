#!/usr/bin/env bash
# Diff the generated SQL surface against the checked-in snapshot.
#
# The contract of this crate is its SQL surface, not a Rust API — which is
# why release-plz turns cargo-semver-checks off for it. Nothing else makes a
# change to that surface visible, so a signature change could ship as an
# ordinary refactor and only surface when someone's CREATE EXTENSION or
# ALTER EXTENSION UPDATE failed.
#
# Two jobs, one file:
#   * review — a surface change shows up as a diff in the PR, deliberately
#   * upgrade scripts — a clean diff means an EMPTY upgrade script is correct
#     for this release; a non-empty diff is the list of what it has to do
#
# That turns "does this release need a real upgrade script?" from a judgment
# call into a derived fact (see sql/README.md).
#
# Requires an initialised $PGRX_HOME: `cargo pgrx schema` builds the
# extension. It therefore runs in ci.yml's postgres job, and locally via
# `task pg:schema-snapshot`.
#
# Bootstrapping: with no snapshot present this writes one and fails, so the
# first run records the truth from a real pgrx build rather than from
# anyone's expectation of what the output looks like.
set -euo pipefail

PG="${PG:?PG must be set (e.g. 18)}"

CRATE_DIR="crates/edtf-postgres"
SNAPSHOT="${CRATE_DIR}/schema.snapshot.sql"
GENERATED="${RUNNER_TEMP:-/tmp}/schema.generated.sql"

cargo pgrx schema "pg${PG}" \
  --package edtf-postgres \
  --manifest-path "${CRATE_DIR}/Cargo.toml" \
  --no-default-features \
  --features "pg${PG}" \
  --out "${GENERATED}"

if [[ ! -f ${SNAPSHOT} ]]; then
  cp "${GENERATED}" "${SNAPSHOT}"
  echo "::error::no schema snapshot existed; one has been written to ${SNAPSHOT}"
  echo "::error::review it and commit it — subsequent runs diff against it"
  echo "--- generated schema ---"
  cat "${SNAPSHOT}"
  exit 1
fi

if ! diff -u "${SNAPSHOT}" "${GENERATED}"; then
  echo "::error::the generated SQL surface differs from ${SNAPSHOT}"
  echo "::error::if the change is intended, commit the new snapshot and add the"
  echo "::error::corresponding ${CRATE_DIR}/sql/edtf_postgres--<previous>--<this>.sql"
  exit 1
fi

echo "::notice::SQL surface matches the snapshot (pg${PG})"
