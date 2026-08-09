# The edtf-postgres OCI images (issues #82, #144): the extension published
# in two shapes from ONE verified download — a runnable postgres, and a
# `FROM scratch` artifact carrying the extension and nothing else.
#
# Both are built FROM THE RELEASED TARBALL — never from source — so each is
# a publication of an artifact already smoke-tested and attested, not a
# second build path that can drift.
#
# The intended consumption is as a build stage, which is what makes the
# images worth publishing at all (Renovate's native Dockerfile manager can
# bump a FROM line; a version inside a curl URL has no dependency
# semantics):
#
#   FROM ghcr.io/carlallenn/edtf-postgres:1.2.3-pg18-artifact AS ext
#   FROM postgres:18-trixie
#   COPY --from=ext /usr/lib/postgresql/18/lib/ /usr/lib/postgresql/18/lib/
#   COPY --from=ext /usr/share/postgresql/18/extension/ /usr/share/postgresql/18/extension/
#
# Take the `-artifact` tag for that, not the runnable one. Both carry
# byte-identical extension files, but the runnable image is a full
# `postgres:<major>-trixie`: BuildKit materialises the whole base chain to
# satisfy a `COPY --from`, so a cold build pulls ~164 MB to copy ~1.7 MB,
# and there is no layer sharing to fall back on (this image pins its base
# independently of yours — any overlap today is luck, and Renovate ends it).
# The scratch variant is those bytes and no others.
#
# TARBALL_SHA256 is the per-architecture line from the release's
# SHA256SUMS, resolved by the workflow; BuildKit verifies it before the
# bytes are used (fail-closed on mismatch).
ARG BASE_IMAGE

# --- extract ---------------------------------------------------------------
# The single ADD both published images resolve from, so "the artifact image
# and the runnable image ship the same extension" holds by construction
# rather than by two independent downloads that happen to agree.
FROM ${BASE_IMAGE} AS extract

ARG PG
ARG VERSION
ARG TARGETARCH
ARG TARBALL_SHA256
ADD --checksum=sha256:${TARBALL_SHA256} \
    https://github.com/CarlAllenn/edtf/releases/download/edtf-postgres-v${VERSION}/edtf_postgres-${VERSION}-pg${PG}-linux-${TARGETARCH}.tar.gz \
    /tmp/edtf.tar.gz
RUN mkdir -p /pkgroot && tar -xzf /tmp/edtf.tar.gz -C /pkgroot && rm /tmp/edtf.tar.gz

# A MIRROR of the Debian tree in the CloudNativePG extension-ImageVolume
# layout — `lib/` and `share/extension/` at the image root — which is what
# a CNPG cluster points `extension_control_path` and `dynamic_library_path`
# at once the volume is mounted (Postgres 18's out-of-tree extension
# support is what makes that possible at all).
#
# A mirror and not a replacement, deliberately: the artifact image ships
# BOTH trees, so one image serves `COPY --from` consumers copying the /usr
# paths and CNPG users mounting the root. Choosing one layout would have
# broken the other, and the duplicate is ~1.7 MB.
RUN mkdir -p /cnpgroot/lib /cnpgroot/share/extension \
    && cp -a "/pkgroot/usr/lib/postgresql/${PG}/lib/." /cnpgroot/lib/ \
    && cp -a "/pkgroot/usr/share/postgresql/${PG}/extension/." /cnpgroot/share/extension/

# --- artifact --------------------------------------------------------------
# The supported build-stage artifact. `FROM scratch`, so it has no OS
# packages, no shell and no Go binaries — its CVE surface is structurally
# empty and stays that way with no rebuild cadence to maintain, which the
# runnable image cannot offer (its posture is exactly its base's, and the
# base's `gosu` is not something this repository can fix or drop).
#
# It also cannot be used as a base by mistake, which the runnable image
# invites: taking that one as a base silently transfers the apt-refresh
# obligation to this repository.
FROM scratch AS artifact

ARG PG
ARG VERSION
ARG REVISION
ARG CREATED
COPY --from=extract /pkgroot/ /
COPY --from=extract /cnpgroot/ /

LABEL org.opencontainers.image.source=https://github.com/CarlAllenn/edtf \
    org.opencontainers.image.title="edtf-postgres (artifact)" \
    org.opencontainers.image.description="The edtf_postgres extension files alone, from the attested release tarball, in both the Debian and CloudNativePG layouts" \
    org.opencontainers.image.licenses="MIT OR Apache-2.0" \
    org.opencontainers.image.version=${VERSION} \
    org.opencontainers.image.revision=${REVISION} \
    org.opencontainers.image.created=${CREATED}

# --- verify ----------------------------------------------------------------
# Never published: the pre-push proof of the artifact stage. A scratch
# image cannot be booted, so the smoke test that proves every other image
# in this pipeline cannot be pointed at it directly — without this, the one
# artifact the README tells people to consume would be the only one
# published without a proof.
#
# So prove it as it is meant to be used: copy the two documented paths into
# a stock postgres and run the ordinary image smoke test against the
# result. That demonstrates the tree is complete, correctly laid out and
# sufficient for the documented consumption pattern, rather than merely
# that some files are present.
#
# In-graph, rather than against the loaded candidate tag, because the
# builder's container driver resolves FROM references from registries and
# cannot see the local docker image store. The post-push half of the proof
# runs from docker/edtf-postgres-consumer.Dockerfile, where the artifact
# image IS in a registry.
FROM ${BASE_IMAGE} AS verify

ARG PG
COPY --from=artifact /usr/lib/postgresql/${PG}/lib/ /usr/lib/postgresql/${PG}/lib/
COPY --from=artifact /usr/share/postgresql/${PG}/extension/ /usr/share/postgresql/${PG}/extension/

# --- runnable --------------------------------------------------------------
# The official postgres image with the extension installed. LAST, so a
# bare `docker build` with no --target still produces what it always did.
#
# A convenience for `docker run`, local trials and demos rather than a
# supported deployment base (issue #147): it installs nothing of its own,
# so its vulnerability posture is exactly its base image's, and that base
# is refreshed only when a release is cut.
#
# The base is a build input to a signed artifact like any other, so it
# comes from the one digest-pinned table (issue #83, gap 3) rather than a
# mutable tag resolved at build time: the workflow looks up
# `base_image <major> trixie` and passes the pinned reference as
# BASE_IMAGE. One table, one lookup — the image the extension ships on
# cannot drift from the images that built and proved it, and the custom
# manager in renovate.json keeps the digest current as Docker Hub
# republishes the tag.
FROM ${BASE_IMAGE} AS runnable

ARG PG
ARG VERSION
ARG REVISION
ARG CREATED
# BASE_NAME/BASE_DIGEST are BASE_IMAGE split by the workflow: a Dockerfile
# LABEL cannot split it itself, and the two halves are what
# `org.opencontainers.image.base.*` are specified to carry.
ARG BASE_NAME
ARG BASE_DIGEST
COPY --from=extract /pkgroot/ /

LABEL org.opencontainers.image.source=https://github.com/CarlAllenn/edtf \
    org.opencontainers.image.title="edtf-postgres" \
    org.opencontainers.image.description="PostgreSQL with the edtf_postgres extension, installed from the attested release tarball" \
    org.opencontainers.image.licenses="MIT OR Apache-2.0" \
    org.opencontainers.image.version=${VERSION} \
    org.opencontainers.image.revision=${REVISION} \
    org.opencontainers.image.created=${CREATED} \
    org.opencontainers.image.base.name=${BASE_NAME} \
    org.opencontainers.image.base.digest=${BASE_DIGEST}
