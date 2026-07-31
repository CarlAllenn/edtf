#!/usr/bin/env bash
# Attach each crate's SBOM to that crate's GitHub release.
#
# Per-crate, not umbrella (issue #66, defect 1): phase 1 creates only the
# per-crate releases (edtf-core-v1.0.1, …) — there is no release at the
# umbrella tag, so the old `gh release upload "${GITHUB_REF_NAME}"` targeted
# something that never exists. Per-crate is also the honest shape: each SBOM
# describes exactly one crate's dependency closure, so it belongs on that
# crate's release and no other.
set -euo pipefail

VERSION="${VERSION:?VERSION must be set}"

CRATES=(edtf-core edtf-calendars edtf-normalize edtf-wasm edtf-cli edtf-postgres)

for name in "${CRATES[@]}"; do
  gh release upload "${name}-v${VERSION}" "sbom/${name}.cdx.json" --clobber
  echo "::notice::attached ${name}.cdx.json to ${name}-v${VERSION}"
done
