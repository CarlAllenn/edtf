#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) the edtf contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
# Fold one postgres major's per-architecture image digests into a single
# manifest list and push it under the given tags (issues #82, #144).
#
# One manifest list per major, under one tag — two arch-suffixed tags is
# the common wrong answer and breaks `docker run` on arm (the #82
# checklist). Called once per published variant: the runnable image and
# the `FROM scratch` artifact image each get their own list, built from
# their own digest directory.
#
# The exact-count assertion is the load-bearing part. A missing cell would
# otherwise publish a single-architecture manifest list that looks healthy
# and fails on the other architecture at pull time.
#
# Prints the resulting manifest digest on stdout and nothing else, so the
# caller can capture it; progress goes to stderr.
set -euo pipefail

DIGEST_DIR="${1:?usage: push-image-manifest.sh <digest-dir> <tag> [tag...]}"
shift

if [[ $# -eq 0 ]]; then
  echo "::error::at least one tag is required" >&2
  exit 1
fi

IMAGE="${IMAGE:?IMAGE must be set}"

refs=()
for f in "${DIGEST_DIR}"/*; do
  [[ -f ${f} ]] || continue
  refs+=("${IMAGE}@$(cat "${f}")")
done

if [[ ${#refs[@]} -ne 2 ]]; then
  echo "::error::expected 2 architecture digests in ${DIGEST_DIR}, found ${#refs[@]}" >&2
  exit 1
fi

tag_args=()
for tag in "$@"; do
  tag_args+=(--tag "${IMAGE}:${tag}")
done

docker buildx imagetools create "${tag_args[@]}" "${refs[@]}" >&2

digest=$(docker buildx imagetools inspect "${IMAGE}:${1}" \
  --format '{{.Manifest.Digest}}')

echo "published ${IMAGE}@${digest} as ${*}" >&2
echo "${digest}"
