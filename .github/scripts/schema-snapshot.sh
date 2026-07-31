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
# NORMALISED, not raw. pgrx's output is unusable as a snapshot verbatim: it
# annotates every item with its source location (`-- src/lib.rs:33`), so any
# edit that shifts a line number would diff, and its own header states that
# item ordering "is not stable, it is driven by a dependency graph". A check
# that fires on unrelated refactors is a check people learn to regenerate
# without reading. So comments are stripped and statements are sorted, which
# leaves exactly the thing being guarded: the set of SQL objects and their
# signatures.
#
# Requires an initialised $PGRX_HOME: `cargo pgrx schema` builds the
# extension. It therefore runs in ci.yml's postgres job, and locally via
# `task pg:schema-snapshot`.
set -euo pipefail

PG="${PG:?PG must be set (e.g. 18)}"

CRATE_DIR="crates/edtf-postgres"
SNAPSHOT="${CRATE_DIR}/schema.snapshot.sql"
RAW="${RUNNER_TEMP:-/tmp}/schema.raw.sql"
GENERATED="${RUNNER_TEMP:-/tmp}/schema.normalised.sql"

cargo pgrx schema "pg${PG}" \
  --package edtf-postgres \
  --manifest-path "${CRATE_DIR}/Cargo.toml" \
  --no-default-features \
  --features "pg${PG}" \
  --out "${RAW}"

# Strip /* */ blocks and -- comments, collapse whitespace, one statement per
# line, sorted. python3 rather than sed: the comment blocks span lines, and
# the pipeline already depends on python3 (verify-registry-bytes.sh).
python3 - "${RAW}" > "${GENERATED}" << 'PY'
import re
import sys

text = open(sys.argv[1], encoding="utf-8").read()
text = re.sub(r"/\*.*?\*/", " ", text, flags=re.S)
text = re.sub(r"--[^\n]*", " ", text)

statements = []
for raw in text.split(";"):
    collapsed = " ".join(raw.split())
    if collapsed:
        statements.append(collapsed + ";")

print("\n".join(sorted(statements)))
PY

if [[ ! -s ${GENERATED} ]]; then
  echo "::error::the generated schema normalised to nothing — that cannot be right"
  exit 1
fi

if [[ ! -f ${SNAPSHOT} ]]; then
  cp "${GENERATED}" "${SNAPSHOT}"
  echo "::error::no schema snapshot existed; one has been written to ${SNAPSHOT}"
  echo "::error::review it and commit it — subsequent runs diff against it"
  echo "--- generated surface ---"
  cat "${SNAPSHOT}"
  exit 1
fi

if ! diff -u "${SNAPSHOT}" "${GENERATED}"; then
  echo "::error::the SQL surface differs from ${SNAPSHOT}"
  echo "::error::if the change is intended, commit the new snapshot and add the"
  echo "::error::corresponding ${CRATE_DIR}/sql/edtf_postgres--<previous>--<this>.sql"
  exit 1
fi

echo "::notice::SQL surface matches the snapshot (pg${PG})"
