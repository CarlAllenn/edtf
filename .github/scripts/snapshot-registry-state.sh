#!/usr/bin/env bash
# Record which artifacts are currently on the registries, to $1.
#
# Used twice in a dry run: once before any publish step could execute, and
# once at the end. The post-condition compares the two, so it detects a
# CHANGE rather than an absence.
#
# That distinction matters because a rehearsal is usually run at a version
# that is already live — validating the pipeline against the current release
# is the normal case. "This version exists on crates.io" is therefore not
# evidence of a leak; only "it did not exist when we started, and does now".
set -euo pipefail

VERSION="${VERSION:?VERSION must be set}"
out="${1:?output path required}"

CRATES=(edtf-core edtf-calendars edtf-normalize edtf-wasm edtf-cli edtf-postgres)

: > "${out}"

for name in "${CRATES[@]}"; do
  body=""
  body=$(curl -sfA 'edtf-release-workflow' \
    "https://crates.io/api/v1/crates/${name}/${VERSION}" 2> /dev/null) || body=""
  if [[ ${body} == *'"num"'* ]]; then
    echo "${name} present" >> "${out}"
  else
    echo "${name} absent" >> "${out}"
  fi
done

npm_version=""
npm_version=$(npm view "edtf-wasm@${VERSION}" version 2> /dev/null) || npm_version=""
if [[ ${npm_version} == "${VERSION}" ]]; then
  echo "edtf-wasm(npm) present" >> "${out}"
else
  echo "edtf-wasm(npm) absent" >> "${out}"
fi
