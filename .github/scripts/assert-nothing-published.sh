#!/usr/bin/env bash
# Post-condition for a dry run: prove this rehearsal published nothing.
#
# Dry-run safety otherwise rests entirely on every publishing step carrying
# an `if: env.DRY_RUN != 'true'` guard — which is a convention, and one a
# future edit can forget silently. This turns that convention into a checked
# post-condition: if a rehearsal ever does reach a registry, the run goes
# red here instead of quietly shipping a version nobody meant to release.
#
# It cannot un-publish anything. It exists so the failure is loud and
# immediate rather than discovered later, which is the same reason the
# self-verify step exists.
set -euo pipefail

VERSION="${VERSION:?VERSION must be set}"

CRATES=(edtf-core edtf-calendars edtf-normalize edtf-wasm edtf-cli edtf-postgres)

leaked=()

for name in "${CRATES[@]}"; do
  body=""
  body=$(curl -sfA 'edtf-release-workflow' \
    "https://crates.io/api/v1/crates/${name}/${VERSION}" 2> /dev/null) || body=""
  if [[ ${body} == *'"num"'* ]]; then
    leaked+=("${name}")
  fi
done

npm_version=""
npm_version=$(npm view "edtf-wasm@${VERSION}" version 2> /dev/null) || npm_version=""
if [[ ${npm_version} == "${VERSION}" ]]; then
  leaked+=("edtf-wasm(npm)")
fi

if [[ ${#leaked[@]} -gt 0 ]]; then
  echo "::error::dry run but ${VERSION} is live on a registry: ${leaked[*]}"
  echo "::error::either a publish guard was bypassed, or this version was already released"
  exit 1
fi

echo "::notice::dry run clean — ${VERSION} is not on any registry"
