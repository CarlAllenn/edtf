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
  publishing release workflow with build-provenance attestations; artifacts
  can be verified with `gh attestation verify`.
