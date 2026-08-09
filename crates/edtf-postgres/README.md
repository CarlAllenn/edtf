# edtf-postgres

Postgres extension (pgrx) exposing
[`edtf-core`](https://crates.io/crates/edtf-core) in SQL — the same
validator the application runs via WebAssembly, so the two layers can never
diverge.

SQL surface: `edtf_valid(text)`, `edtf_level(text)`,
`edtf_canonical(text)`, `edtf_min(text)` / `edtf_max(text)` (index-friendly
date bounds), and `edtf_relation(text, text)` (three-valued temporal
relations).

## Installing

Prebuilt, attested tarballs are attached to each `edtf-postgres-v*` release
from v1.1.0 onward, for Postgres 14–18 on `amd64` and `arm64` — no Rust toolchain and no
`cargo-pgrx` required. Download the tarball for your major and architecture,
verify it, extract into `/`, then:

```sql
CREATE EXTENSION edtf_postgres;
```

The extension is `trusted`, so any user with `CREATE` on the database can
install it. The shipped library is stripped; a `-dbgsym` tarball with the
full debug info sits beside each tarball for crash analysis. Full
instructions, the support matrix and the glibc floor are in
the [repository README](https://github.com/CarlAllenn/edtf#installing-the-postgres-extension).

OCI images are also published to `ghcr.io/carlallenn/edtf-postgres`, built
from the released tarballs and attested the same way — the command that
checks a digest against its release tag, and the three flags that make it
mean anything, are in
[the security policy](https://github.com/CarlAllenn/edtf/blob/main/SECURITY.md#verifying-a-published-image).
Two variants per Postgres major, both multi-arch, both tagged extension
version × major (`1.2.3-pg18`) with a floating major tag (`pg18`) tracking
the latest release for that major. Pin the digest.

- **`:<version>-pg<major>-artifact`** — `FROM scratch`: the extension
  files and nothing else. The supported build-stage artifact. It carries
  no OS packages, so it has no inherited CVE surface, and a cold
  `COPY --from` pulls only the extension rather than a whole base image.
- **`:<version>-pg<major>`** — the official `postgres` image with the
  extension installed. A convenience for `docker run`, local trials and
  demos; not a supported deployment base. It installs nothing of its own,
  so its vulnerability posture is exactly its base image's, and that base
  is refreshed only when a release is cut — expect base-inherited CVEs
  between releases. Pin the digest and rebuild on your own cadence if you
  deploy it.

Neither floating tag is rebuilt between releases: `pg18` means "the latest
release for Postgres 18", not "the latest base".

**To deploy, build your own image from the `-artifact` variant** onto a
base you pin and refresh on your own cadence — the `COPY --from` above is
the whole of it. That keeps the extension's currency and your base's
currency independent, which is the point: an extension release should not
be what ships you a base image update, and a base image update should not
wait on an extension release.

Take the `-artifact` tag for a build stage:

```dockerfile
FROM ghcr.io/carlallenn/edtf-postgres:1.2.3-pg18-artifact AS ext
FROM postgres:18-trixie
COPY --from=ext /usr/lib/postgresql/18/lib/ /usr/lib/postgresql/18/lib/
COPY --from=ext /usr/share/postgresql/18/extension/ /usr/share/postgresql/18/extension/
```

The artifact image also carries a second copy of the same files in the
[CloudNativePG extension-ImageVolume][cnpg-ext] layout — `lib/` and
`share/extension/` at the image root — so the same tag can be mounted
into a CNPG cluster without a repack:

```yaml
postgresql:
  extensions:
    - name: edtf
      image:
        reference: ghcr.io/carlallenn/edtf-postgres:1.2.3-pg18-artifact
```

That path is tested, not inferred: a weekly job builds a CNPG cluster
around the published tag and installs the extension into it as a
non-superuser. It needs PostgreSQL 18 — `extension_control_path` is what
makes out-of-tree extensions loadable at all — CloudNativePG 1.29 or
later, and a Kubernetes with image volumes: 1.35 and later have them on by
default, 1.33 and 1.34 need the `ImageVolume` feature gate. The operand
image must be Debian 12 or newer, the same glibc 2.36 floor the tarballs
carry.

The tarballs remain the primary artifact; the images are wrappers around
them, never a replacement.

[cnpg-ext]: https://cloudnative-pg.io/docs/1.29/imagevolume_extensions/

Building from source instead needs `cargo-pgrx` and an initialised
`$PGRX_HOME`; see the repository for the development workflow.

Its contract is the SQL surface, not a Rust API. Part of the
[`edtf`](https://github.com/CarlAllenn/edtf) crate family.
