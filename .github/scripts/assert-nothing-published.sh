#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) the edtf contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
# Post-condition for a dry run: prove this rehearsal published nothing.
#
# Dry-run safety otherwise rests entirely on every publishing step carrying
# an `if: env.DRY_RUN != 'true'` guard — a convention, and one a future edit
# can forget silently. This turns the convention into a checked
# post-condition: if a rehearsal ever does reach a registry, the run goes red
# here instead of quietly shipping a version nobody meant to release.
#
# It cannot un-publish anything. It exists so the failure is loud and
# immediate rather than discovered later — the same reason the self-verify
# step exists.
#
# Compares against the snapshot taken before any publish step could run, so
# it detects a CHANGE rather than an absence. Rehearsing at an already-live
# version is normal, so "this version is on crates.io" proves nothing on its
# own.
set -euo pipefail

VERSION="${VERSION:?VERSION must be set}"
RUNNER_TEMP="${RUNNER_TEMP:?RUNNER_TEMP must be set}"

before="${RUNNER_TEMP}/registry-before.txt"
after="${RUNNER_TEMP}/registry-after.txt"

if [[ ! -f ${before} ]]; then
  echo "::error::no registry snapshot at ${before} — cannot prove this run published nothing"
  exit 1
fi

.github/scripts/snapshot-registry-state.sh "${after}"

if ! diff -u "${before}" "${after}"; then
  echo "::error::registry state changed during a dry run — something published"
  exit 1
fi

echo "::notice::dry run clean — registry state unchanged for ${VERSION}"
cat "${after}"
