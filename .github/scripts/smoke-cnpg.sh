#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) the edtf contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
# Prove the CloudNativePG ImageVolume claim (issue #158).
#
# The artifact image carries a MIRROR of the extension in the CNPG
# extension-ImageVolume layout — `lib/` and `share/extension/` at the image
# root — and crates/edtf-postgres/README.md prints a `Cluster` snippet for
# it. Until this script existed that was the one shipped claim in this
# repository standing on inference: the files were known to land at the
# right paths, and the Debian tree was proved end to end by smoke-image.sh,
# but nobody had ever mounted the image into a cluster and watched Postgres
# resolve `extension_control_path` / `dynamic_library_path` to it.
#
# So: a throwaway kind cluster, the CNPG operator, the README's manifest,
# and then the load-bearing subset of smoke-image.sh's assertions —
# non-superuser CREATE EXTENSION (trusted = true), extversion == release,
# the shared corpus. The upgrade-path and relocatable checks stay in
# smoke-extension.sh; they are properties of the tarball this image was
# built from, already proved before it shipped.
#
# Scheduled, never on the push path (scheduled.yml). Three reasons, all the
# same reason: the inputs move without a commit here. It needs an external
# operator and a Kubernetes floor, it needs a PUBLISHED artifact image so it
# cannot run on a PR that has not released, and a CNPG regression should
# surface on a weekly log rather than block a release.
#
# pg18 only: PostgreSQL 18's out-of-tree extension support (the
# `extension_control_path` GUC) is what makes ImageVolumes work at all, so
# the older majors have no such consumption path to cover.
set -euo pipefail

IMAGE_REF="${1:?usage: smoke-cnpg.sh <artifact-image-ref>}"
VERSION="${VERSION:?VERSION must be set}"

CORPUS="crates/edtf-postgres/tests/corpus.sql"

# --- pins -------------------------------------------------------------------
# The same rule as base-images.sh: every container image any proof runs in is
# a digest-pinned input, kept current by the custom manager in renovate.json.
#
# The kind FLOOR is load-bearing rather than incidental. Image volumes need
# containerd 2.1, which first shipped in kind 0.29's node images; on anything
# older the node's runtime cannot mount the volume at all. The Kubernetes
# floor is 1.35, where ImageVolume is beta and ON by default — 1.33 and 1.34
# need the feature gate enabled by hand, which is a cluster shape no consumer
# should have to reproduce to read this proof.
KIND_NODE_IMAGE="kindest/node:v1.36.1@sha256:3489c7674813ba5d8b1a9977baea8a6e553784dab7b84759d1014dbd78f7ebd5"

# The operator. Only the version is written down; the release branch it is
# served from is derived, so there is one string to bump and no way for the
# two halves to disagree.
CNPG_VERSION="1.30.0"
CNPG_MANIFEST="https://raw.githubusercontent.com/cloudnative-pg/cloudnative-pg/release-${CNPG_VERSION%.*}/releases/cnpg-${CNPG_VERSION}.yaml"

# The operand: CNPG's own PostgreSQL image, deliberately the BOOKWORM build.
# CNPG requires an extension image to match the cluster's OS distribution,
# and build-extension.sh compiles inside `postgres:<major>-bookworm` — so
# bookworm here is the matching pair, not a default.
OPERAND_IMAGE="ghcr.io/cloudnative-pg/postgresql:18-minimal-bookworm@sha256:9219323092591c2d37e4c3144e9d1c4878803fbeb103bfaffa53bf15a65d2c71"

if [[ ! -f ${CORPUS} ]]; then
  echo "::error::corpus not found: ${CORPUS}"
  exit 1
fi

# Fail fast and legibly on the one predictable absence: no release has
# published an artifact image yet, or the publish leg stopped pushing the
# tag. Without this the symptom is a cluster that never reaches Ready and a
# ten-minute timeout with the cause buried in a kubelet event.
if ! docker manifest inspect "${IMAGE_REF}" > /dev/null 2>&1; then
  echo "::error::no such image: ${IMAGE_REF}"
  echo "::error::the artifact variant ships from the first release after #156 — check that the publish leg pushed it"
  exit 1
fi

cluster="edtf-cnpg-$$"
kubeconfig=""
kubeconfig=$(mktemp)
export KUBECONFIG="${kubeconfig}"

# Diagnostics BEFORE teardown: the cluster is the evidence, and deleting it
# first would leave a failure with nothing to read. Only on failure — a green
# run should be quiet.
diagnose() {
  echo "::group::CNPG cluster state"
  kubectl get cluster edtf -o yaml 2> /dev/null || true
  kubectl get pods -o wide 2> /dev/null || true
  kubectl get events --sort-by=.lastTimestamp 2> /dev/null | tail -30 || true
  kubectl -n cnpg-system logs deployment/cnpg-controller-manager --tail=50 2> /dev/null || true
  echo "::endgroup::"
}

cleanup() {
  local status=$?
  if [[ ${status} -ne 0 ]]; then
    diagnose
  fi
  kind delete cluster --name "${cluster}" > /dev/null 2>&1 || true
  rm -f "${kubeconfig}"
  return "${status}"
}
trap cleanup EXIT

echo "::notice::CNPG ImageVolume smoke test: ${IMAGE_REF}"

kind create cluster --name "${cluster}" --image "${KIND_NODE_IMAGE}" --wait 120s

# --server-side: the CNPG CRDs carry annotations past the 262144-byte limit
# that client-side apply stores them in, which is a hard error, not a warning.
kubectl apply --server-side -f "${CNPG_MANIFEST}"
kubectl -n cnpg-system rollout status deployment/cnpg-controller-manager --timeout=300s

# The manifest is inline rather than a checked-in YAML file because two of
# its four interesting values are substituted anyway — a file would be a
# template that is never itself valid, and the pins would live away from the
# pins above. What must NOT drift from the README is the shape below:
# `postgresql.extensions[].image.reference`, one extension named `edtf`.
kubectl apply -f - << YAML
apiVersion: postgresql.cnpg.io/v1
kind: Cluster
metadata:
  name: edtf
spec:
  instances: 1
  imageName: ${OPERAND_IMAGE}
  storage:
    size: 1Gi
  postgresql:
    extensions:
      - name: edtf
        image:
          reference: ${IMAGE_REF}
YAML

# Generous, because this waits on three cold image pulls (operator, operand,
# extension) on a runner with no layer cache. A hang guard, not a budget.
kubectl wait --for=condition=Ready cluster/edtf --timeout=600s

primary=""
primary=$(kubectl get cluster edtf -o jsonpath='{.status.currentPrimary}')

# The claim under test, stated exactly: Postgres inside the cluster resolves
# its extension paths to the MOUNT, not to a repacked image. Everything below
# would also pass if the extension had been baked into the operand image;
# this is the assertion that distinguishes the two.
psql_super() { kubectl exec "${primary}" -c postgres -- psql -U postgres "$@"; }

control_path=""
control_path=$(psql_super -tAc "SHOW extension_control_path;")
library_path=""
library_path=$(psql_super -tAc "SHOW dynamic_library_path;")

if [[ ${control_path} != *"/extensions/edtf/share"* ]]; then
  echo "::error::extension_control_path is '${control_path}', expected it to include /extensions/edtf/share"
  exit 1
fi
if [[ ${library_path} != *"/extensions/edtf/lib"* ]]; then
  echo "::error::dynamic_library_path is '${library_path}', expected it to include /extensions/edtf/lib"
  exit 1
fi
echo "ok  the ImageVolume mount is on both extension paths"
echo "    extension_control_path = ${control_path}"
echo "    dynamic_library_path   = ${library_path}"

# A plain role, owning its own database: CREATE on the database and nothing
# more. Deliberately NOT superuser — trusted = true is the claim under test.
# Over TCP rather than the socket: CNPG's pg_hba trusts the local socket for
# the operator's own connections, which would make a superuser check of this
# out.
psql_super -v ON_ERROR_STOP=1 -q \
  -c "CREATE ROLE smoke LOGIN PASSWORD 'smoke';" \
  -c "CREATE DATABASE smokedb OWNER smoke;"

psql_smoke() {
  kubectl exec "${primary}" -c postgres -- \
    env PGPASSWORD=smoke psql -h 127.0.0.1 -U smoke -d smokedb "$@"
}

psql_smoke -v ON_ERROR_STOP=1 -q -c "CREATE EXTENSION edtf_postgres;"
echo "ok  CREATE EXTENSION as a non-superuser (trusted = true holds)"

INSTALLED=""
INSTALLED=$(psql_smoke -tAc "SELECT extversion FROM pg_extension WHERE extname = 'edtf_postgres';")

if [[ ${INSTALLED} != "${VERSION}" ]]; then
  echo "::error::installed extversion is '${INSTALLED}', expected '${VERSION}'"
  exit 1
fi
echo "ok  extversion ${INSTALLED} matches the release"

# Streamed in rather than copied: `kubectl cp` needs tar in the target
# container, and the operand image is the MINIMAL build, which has none.
# Written out instead of going through psql_smoke because `kubectl exec`
# needs -i to forward stdin, and that flag belongs to kubectl, not psql.
kubectl exec -i "${primary}" -c postgres -- \
  env PGPASSWORD=smoke psql -h 127.0.0.1 -U smoke -d smokedb \
  -v ON_ERROR_STOP=1 -q -f - < "${CORPUS}"
echo "ok  shared corpus passes against the mounted extension"
