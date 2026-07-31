#!/usr/bin/env bash
# Every released version must be reachable by `ALTER EXTENSION ... UPDATE`.
#
# A CI-time repository invariant, not a release step: it runs in the lint
# gate so a release PR goes red while the fix is still cheap.
#
# Why it matters. `default_version` in the control file is `@CARGO_VERSION@`,
# so every release mints a new extension version whether or not the SQL
# surface moved. Without a script joining the previous version to the new
# one, a user who installed the old one cannot update — their only path is
# DROP and CREATE, and the README recommends these functions inside CHECK
# constraints and expression indexes, so DROP either errors on dependencies
# or, with CASCADE, silently removes the user's constraints and indexes.
#
# pgrx knows about ./sql/*.sql and copies those files into the install tree,
# but it cannot author them — the SQL is hand-written by definition.
#
# Self-arming. Before the first upgrade script exists there is nothing to
# be consistent with, so this passes with a notice: the obligation begins
# with the first release that ships a prebuilt binary, because that is when
# strangers start having an installed version to upgrade FROM. Once the
# first script lands, every subsequent version must be a target.
#
# Cheap to satisfy: `module_pathname` is set in the control file, so pgrx's
# versioned-.so mode is off and existing definitions keep resolving through
# MODULE_PATHNAME after the library is replaced in place. A release whose
# SQL surface did not change needs only an EMPTY script for the path to
# exist. schema-snapshot.sh is what tells you whether the surface moved.
set -euo pipefail

CRATE_DIR="crates/edtf-postgres"
SQL_DIR="${CRATE_DIR}/sql"

VERSION=""
VERSION=$(cargo pkgid --manifest-path "${CRATE_DIR}/Cargo.toml" | sed 's/.*[@#]//')

if [[ -z ${VERSION} ]]; then
  echo "::error::could not resolve the edtf-postgres version"
  exit 1
fi

# An unmatched glob must expand to nothing rather than to itself.
shopt -s nullglob
scripts=("${SQL_DIR}"/edtf_postgres--*--*.sql)
shopt -u nullglob

if [[ ${#scripts[@]} -eq 0 ]]; then
  echo "::notice::no upgrade scripts yet — nothing to be consistent with"
  echo "::notice::the first release shipping a prebuilt binary must add ${SQL_DIR}/edtf_postgres--<previous>--${VERSION}.sql"
  exit 0
fi

targets=()
for path in "${scripts[@]}"; do
  file=$(basename "${path}")
  # edtf_postgres--<from>--<to>.sql
  to="${file##*--}"
  targets+=("${to%.sql}")
done

for target in "${targets[@]}"; do
  if [[ ${target} == "${VERSION}" ]]; then
    echo "::notice::upgrade path into ${VERSION} present"
    printf 'ok  %s\n' "${scripts[@]##*/}"
    exit 0
  fi
done

echo "::error::no upgrade script targets ${VERSION}"
echo "::error::create ${SQL_DIR}/edtf_postgres--<previous>--${VERSION}.sql"
echo "::error::an empty file is correct when the SQL surface did not change"
echo "::error::existing scripts target: ${targets[*]}"
exit 1
