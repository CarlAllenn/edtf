#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) the edtf contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
# Every tracked *.sh must be executable IN GIT.
#
# Paid for at v1.1.0. canary-extension.sh was committed 100644 while its
# twenty-three siblings were 100755, and workflows invoke these by path
# (`run: .github/scripts/x.sh`). The release published six crates to
# crates.io, an npm package, ten attested tarballs — and then died at
# `Permission denied`, exit 126, with the releases left as drafts. The
# working tree was fine; only the committed mode was wrong, so nothing local
# could have caught it.
#
# The tag freezes the scripts, so a defect like this cannot be fixed at the
# tag it broke: the release has to be completed out-of-band. That is what
# makes a cheap mode check worth a gate of its own.
#
# Repo-wide, not just .github/scripts: fuzz/seed.sh is invoked by path from
# fuzz.yml and sat outside the first version of this gate — the same class
# of defect, one directory over.
#
# `git ls-files -s` reports the INDEX mode, which is the thing that actually
# ships — not the filesystem bit, which can differ and which a fresh clone
# on a mode-ignoring filesystem would not reproduce.
set -euo pipefail

# Captured first, then iterated. Reading straight from a process
# substitution masks git's exit status (SC2312): a failed `git ls-files`
# would feed the loop nothing and this check would pass having examined
# nothing — the same vacuous-success shape self-verify-attestations.sh
# guards against.
LISTING=""
LISTING=$(git ls-files -s '*.sh')

if [[ -z ${LISTING} ]]; then
  echo "::error::no tracked .sh files found — refusing to pass vacuously"
  exit 1
fi

bad=()
while read -r mode _ _ path; do
  if [[ ${mode} != "100755" ]]; then
    bad+=("${mode} ${path}")
  fi
done <<< "${LISTING}"

if [[ ${#bad[@]} -gt 0 ]]; then
  echo "::error::${#bad[@]} tracked script(s) are not executable in git:"
  printf '::error::  %s\n' "${bad[@]}"
  echo "::error::fix with: git update-index --chmod=+x <path>"
  exit 1
fi

count=$(git ls-files '*.sh' | wc -l | tr -d ' ')
echo "::notice::all ${count} tracked scripts are executable"
