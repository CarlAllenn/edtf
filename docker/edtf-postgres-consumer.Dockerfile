# The published `COPY --from` snippet, executed against the PUBLISHED
# artifact image (issue #144).
#
# The post-push half of the artifact proof. The pre-push half is the
# `verify` target in edtf-postgres.Dockerfile, which resolves the artifact
# stage in-graph on native hardware for both architectures; this half
# covers the publish step itself, mirroring how the runnable image is
# pulled back by digest and smoked after its manifest list exists.
#
# It has to be a separate file rather than another target there: the
# subject is an image in a registry, not a stage, and there is no way to
# splice a pushed reference into that graph.
#
# ARTIFACT_IMAGE is a build arg because the digest is only known at run
# time — the builder's container driver can pull it, which is precisely
# why this half runs after the push and the other half does not.
ARG ARTIFACT_IMAGE
ARG BASE_IMAGE

FROM ${ARTIFACT_IMAGE} AS ext

FROM ${BASE_IMAGE}
ARG PG
COPY --from=ext /usr/lib/postgresql/${PG}/lib/ /usr/lib/postgresql/${PG}/lib/
COPY --from=ext /usr/share/postgresql/${PG}/extension/ /usr/share/postgresql/${PG}/extension/
