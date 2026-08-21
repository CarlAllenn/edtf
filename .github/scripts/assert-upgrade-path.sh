#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) the edtf contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
# Every PUBLISHED version must be able to reach the current one by
# `ALTER EXTENSION edtf_postgres UPDATE`.
#
# A CI-time repository invariant, not a release step: it runs in the lint
# gate so a release PR goes red while the fix is still cheap.
#
# Why it matters. `default_version` in the control file is `@CARGO_VERSION@`,
# so every release mints a new extension version whether or not the SQL
# surface moved. Without a path joining an installed version to the new one,
# a user's only route is DROP and CREATE — and the README recommends these
# functions inside CHECK constraints and expression indexes, so DROP either
# errors on those dependencies or, with CASCADE, silently removes them.
#
# REACHABILITY, not just "is the current version a target". The first
# version of this check parsed only the trailing half of each filename and
# passed on the first script whose `to` matched. That let three published
# versions (0.2.0, 1.0.0, 1.0.1) sit with no route to 1.1.0 while the gate
# stayed green, and contradicted what sql/README.md claimed it enforced.
# Postgres walks the shortest path across the whole graph, so the graph is
# what has to be checked.
#
# The walk is in python3 rather than bash. The bash version needed
# `declare -A`, which is bash 4+ and therefore fails on macOS's stock 3.2 —
# green on CI, red on a laptop, against a repository whose README promises
# `task ci` is "identical locally and on GitHub". python3 is already a
# pipeline dependency (verify-registry-bytes.sh, schema-snapshot.sh), and a
# graph search reads better in it than in string-splitting bash.
#
# RELEASED is derived from the upgrade graph itself, and still never from
# git tags: the lint job checks out shallow and without tags, and the
# pre-1.0 releases predate the per-crate tag scheme anyway.
#
# It used to be a hand-maintained array with "append to it on release" in
# this comment. That instruction had no owner once the org's release path
# took over: the Release PR is machine-generated, so nothing appends, and
# the first canonical release (v1.3.0) reddened its own gate asking for a
# line only a human could add — to a file the machinery had already
# finished writing.
#
# Every published version is an endpoint of some upgrade script, so the
# union of the `from` and `to` halves of `edtf_postgres--<from>--<to>.sql`
# IS the released set. Measured at the change: the derived union equals the
# eleven versions the array listed, exactly. And it is the same source the
# org's own derivation reads to pick an upgrade baseline
# (monumental-archive/.github#762), so the two agree by construction.
#
# The glob worry that motivated enumerating is still right and still
# handled: a glob matching NO scripts is a hard error below, not an empty
# set that vacuously passes.
#
# Cheap to satisfy: `module_pathname` is set, so pgrx's versioned-.so mode
# is off and existing definitions keep resolving after the library is
# replaced in place. A release whose SQL surface did not change needs only
# an EMPTY script. schema-snapshot.sh is what decides whether it changed.
set -euo pipefail

CRATE_DIR="crates/edtf-postgres"
SQL_DIR="${CRATE_DIR}/sql"

# Every version ever published, derived from the upgrade graph's endpoints.
RELEASED=()
# shellcheck disable=SC2312  # process substitution over local state: the
# only interesting failure is "no scripts matched", and capturing the
# pipeline first would turn that into one blank line. The empty case is
# checked explicitly below, which is stronger than the masked status.
while IFS= read -r released_version; do
  [[ -n ${released_version} ]] && RELEASED+=("${released_version}")
done < <(
  find "${SQL_DIR}" -maxdepth 1 -name 'edtf_postgres--*--*.sql' -exec basename {} \; \
    | sed -nE 's/^edtf_postgres--(.+)--(.+)\.sql$/\1\n\2/p' \
    | sort -uV
)
if ((${#RELEASED[@]} == 0)); then
  echo "::error::no upgrade scripts in ${SQL_DIR}, so no released versions could be derived"
  echo "::error::a glob matching nothing must fail, never pass vacuously"
  exit 1
fi

VERSION=""
VERSION=$(cargo pkgid --manifest-path "${CRATE_DIR}/Cargo.toml" | sed 's/.*[@#]//')

if [[ -z ${VERSION} ]]; then
  echo "::error::could not resolve the edtf-postgres version"
  exit 1
fi

python3 - "${VERSION}" "${SQL_DIR}" "${RELEASED[@]}" << 'PY'
import collections
import glob
import os
import re
import sys

version, sql_dir, *released = sys.argv[1:]

paths = sorted(glob.glob(os.path.join(sql_dir, "edtf_postgres--*--*.sql")))
if not paths:
    print(f"::error::no upgrade scripts in {sql_dir}")
    print(f"::error::every published version needs a route to {version}")
    sys.exit(1)

pattern = re.compile(r"^edtf_postgres--(.+?)--(.+)\.sql$")
edges = collections.defaultdict(list)

for path in paths:
    match = pattern.match(os.path.basename(path))
    if not match:
        print(f"::error::cannot parse upgrade script name: {os.path.basename(path)}")
        print("::error::expected edtf_postgres--<from>--<to>.sql")
        sys.exit(1)
    edges[match.group(1)].append(match.group(2))


def reaches(start):
    """Can `start` get to `version` by any chain of upgrade scripts?"""
    seen, queue = set(), collections.deque([start])
    while queue:
        node = queue.popleft()
        if node == version:
            return True
        if node in seen:
            continue
        seen.add(node)
        queue.extend(edges.get(node, ()))
    return False


stranded = [v for v in released if v != version and not reaches(v)]

if stranded:
    print(f"::error::no ALTER EXTENSION UPDATE path to {version} from: {' '.join(stranded)}")
    print(f"::error::add {sql_dir}/edtf_postgres--<from>--<to>.sql to connect them")
    print("::error::an empty file is correct when the SQL surface did not change")
    sys.exit(1)

# If the current version is not in RELEASED, the list was not updated for
# this release — so future releases would not be checked against it.
if version not in released:
    print(f"::error::{version} is missing from RELEASED in assert-upgrade-path.sh")
    print("::error::add it so future releases are checked against this one")
    sys.exit(1)

print(f"::notice::every published version reaches {version}")
for path in paths:
    print(f"ok  {os.path.basename(path)}")
PY
