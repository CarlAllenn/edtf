# The edtf-postgres OCI image (issue #82): postgres:<major>-trixie plus the
# extension, built FROM THE RELEASED TARBALL — never from source — so the
# image is a publication of an artifact already smoke-tested and attested,
# not a second build path that can drift.
#
# The intended consumption is as a build stage, which is what makes the
# image worth publishing at all (Renovate's native Dockerfile manager can
# bump a FROM line; a version inside a curl URL has no dependency
# semantics):
#
#   FROM ghcr.io/carlallenn/edtf-postgres:1.1.2-pg18 AS ext
#   FROM postgres:18-trixie
#   COPY --from=ext /usr/lib/postgresql/18/lib/edtf_postgres.so /usr/lib/postgresql/18/lib/
#   COPY --from=ext /usr/share/postgresql/18/extension/ /usr/share/postgresql/18/extension/
#
# A consumer copying those two paths receives none of this image's base
# layers. Running it directly also works, though — it IS the official
# postgres image with the extension installed — and either way the base is
# a build input to a signed, attested artifact. So it resolves through
# base-images.sh like every other release-path image (issue #83, gap 3)
# instead of floating on a mutable tag: the workflow looks up
# `base_image <major> trixie` and passes the pinned reference as
# BASE_IMAGE. One table, one lookup — the image the extension ships on
# cannot drift from the images that built and proved it, and the custom
# manager in renovate.json keeps the digest current as Docker Hub
# republishes the tag.
#
# TARBALL_SHA256 is the per-architecture line from the release's
# SHA256SUMS, resolved by the workflow; BuildKit verifies it before the
# bytes are used (fail-closed on mismatch).
ARG BASE_IMAGE
FROM ${BASE_IMAGE}

ARG PG
ARG VERSION
ARG TARGETARCH
ARG TARBALL_SHA256
ADD --checksum=sha256:${TARBALL_SHA256} \
    https://github.com/CarlAllenn/edtf/releases/download/edtf-postgres-v${VERSION}/edtf_postgres-${VERSION}-pg${PG}-linux-${TARGETARCH}.tar.gz \
    /tmp/edtf.tar.gz
RUN tar -xzf /tmp/edtf.tar.gz -C / && rm /tmp/edtf.tar.gz

LABEL org.opencontainers.image.source=https://github.com/CarlAllenn/edtf \
    org.opencontainers.image.description="PostgreSQL with the edtf_postgres extension, installed from the attested release tarball" \
    org.opencontainers.image.licenses=MIT
