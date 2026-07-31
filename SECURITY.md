# Security policy

## Supported versions

Only the latest released version of each crate in the `edtf` family
(`edtf-core`, `edtf-normalize`, `edtf-calendars`, `edtf-cli`, `edtf-wasm`,
`edtf-postgres`, and the `edtf-wasm` npm package) receives security fixes.

## Reporting a vulnerability

Please report vulnerabilities privately via GitHub's
[private vulnerability reporting](https://github.com/CarlAllenn/edtf/security/advisories/new)
— do not open a public issue for a security problem.

You can expect an acknowledgement within a week. Once a fix is released, the
advisory will be published with credit to the reporter (unless you prefer
otherwise).

## Scope notes

- `edtf-core` and `edtf-calendars` are `#![no_std]`, zero-dependency parsers
  and converters; the primary risk surface is hostile input, which is
  continuously fuzzed (never-panic and round-trip properties, nightly
  coverage-guided fuzzing in CI). A reproducible panic or non-termination on
  any input is considered a security bug.
- Releases are published exclusively through the tag-immutable, trusted-
  publishing release workflow, which builds, publishes and attests in a
  single run whose git ref is the release tag.
- `edtf-postgres` is marked `trusted` in its control file, so a non-superuser
  with `CREATE` on a database can install it and Postgres runs the install
  script as the bootstrap superuser. Its SQL surface is six `IMMUTABLE
  STRICT` functions with no dynamic SQL, no filesystem or network access and
  no `SECURITY DEFINER`, so there is nothing for a search-path attack to
  redirect. Every release installs it as a non-superuser in a clean Postgres
  and runs the conformance corpus against it, so the claim is tested rather
  than asserted.
- Prebuilt extension tarballs are compiled inside the Postgres image they
  target, from the pgdg `pg_config` consumers actually run, and are installed
  and exercised on two Debian releases before the release is published.

## Verifying a release

Verify an artifact against the tag it claims to come from:

```sh
gh attestation verify edtf-core-1.0.1.crate \
  --repo CarlAllenn/edtf \
  --source-ref refs/tags/v1.0.1 \
  --signer-workflow CarlAllenn/edtf/.github/workflows/publish.yml
```

The three flags are the check. A bare `gh attestation verify` confirms only
that *some* run in this repository signed those bytes — it does not tell you
which commit they were built from, or which workflow was allowed to sign.

The same command verifies a prebuilt `edtf-postgres` tarball; substitute the
asset name. Verify **before** extracting, since the archive unpacks into `/`.

### Which attestation is which

Three kinds are produced, and they answer different questions:

| Attestation | Subject | Answers |
| --- | --- | --- |
| Build provenance | each `.crate`, the npm tarball, each extension tarball, `SHA256SUMS` | which commit, ref and workflow built these bytes |
| SBOM | each `.crate` | what that crate's dependency closure was |
| Release | the GitHub release | which tag, commit and asset set the release contains |

The first is the one to check with `gh attestation verify`. The third is
generated automatically because releases are immutable, and is a property of
the release rather than of any single file.

**Known limitation, v1.0.0 only.** The v1.0.0 attestations record the run
that *signed* the artifacts, not the run that *built* them: publishing
happened across three runs, and the final one downloaded the crates from
crates.io and signed them. Their digests are genuine and a bare verify
passes, but the provenance names commit `1e80e45`, which built none of
them. Sigstore attestations are append-only, so this cannot be corrected in
place — v1.0.1 onwards is published through the workflow above, where the
run that signs is the run that built. Verification pinned to `--source-ref`
will correctly fail against v1.0.0.
