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
# A consumer copying those two paths never receives this image's base
# layers, which is why the base tag deliberately floats at build time: each
# release resolves postgres:<major>-trixie fresh, and there is no standing
# CVE-rebuild obligation for layers no build-stage consumer inherits.
# Running the image directly also works — it IS the official postgres image
# with the extension installed.
#
# TARBALL_SHA256 is the per-architecture line from the release's
# SHA256SUMS, resolved by the workflow; BuildKit verifies it before the
# bytes are used (fail-closed on mismatch).
ARG PG
FROM postgres:${PG}-trixie

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
