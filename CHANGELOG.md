# Changelog

All notable changes to this project are recorded here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and versions follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html)
over the public APIs of the crates this repository publishes.

## [1.3.0](https://github.com/monumental-archive/edtf/compare/v1.2.3...v1.3.0) - 2026-08-21

### Added

- publish the coverage number to Codecov, without a threshold gate ([#143](https://github.com/monumental-archive/edtf/pull/143))
- make the project citable — CITATION.cff, and the Zenodo path to a DOI ([#145](https://github.com/monumental-archive/edtf/pull/145))
- gate CITATION.cff against the CFF schema, not just YAML syntax ([#150](https://github.com/monumental-archive/edtf/pull/150))
- fold edtf-postgres into the coverage metric ([#155](https://github.com/monumental-archive/edtf/pull/155))
- publish a FROM scratch artifact variant ([#156](https://github.com/monumental-archive/edtf/pull/156))

### Fixed

- exclude the Apache licence's own http:// URLs from lychee ([#138](https://github.com/monumental-archive/edtf/pull/138))
- pin the OCI image base and record it in provenance ([#140](https://github.com/monumental-archive/edtf/pull/140))
- allow the host Codecov actually uploads to, and silence its telemetry ([#146](https://github.com/monumental-archive/edtf/pull/146))
- add the root CHANGELOG the release path writes into ([#173](https://github.com/monumental-archive/edtf/pull/173))
- keep a version number out of the CHANGELOG's headings ([#174](https://github.com/monumental-archive/edtf/pull/174))
- derive the released versions instead of listing them ([#180](https://github.com/monumental-archive/edtf/pull/180))

### Documentation

- correct the 1.2.x post-mortem with the actual root cause ([#137](https://github.com/monumental-archive/edtf/pull/137))
- let licence detection land on the real licence texts ([#139](https://github.com/monumental-archive/edtf/pull/139))
- storage.googleapis.com is proven, not assumed ([#148](https://github.com/monumental-archive/edtf/pull/148))
- the runnable image is a convenience, not an artifact ([#157](https://github.com/monumental-archive/edtf/pull/157))
- tell COPY --from consumers to move to -artifact (#161) ([#165](https://github.com/monumental-archive/edtf/pull/165))
- document verifying an image attestation (#159) ([#166](https://github.com/monumental-archive/edtf/pull/166))

### Testing

- cover every remaining defensive guard ([#152](https://github.com/monumental-archive/edtf/pull/152))
- prove the CloudNativePG ImageVolume claim (#158) ([#162](https://github.com/monumental-archive/edtf/pull/162))

### CI

- gate the image path on pull requests (#160) ([#164](https://github.com/monumental-archive/edtf/pull/164))

### Dependencies

- update dependency jdx/mise to v2026.8.0 ([#167](https://github.com/monumental-archive/edtf/pull/167))
- update jdx/mise-action action to v4.2.4 ([#169](https://github.com/monumental-archive/edtf/pull/169))

## Earlier history lives per crate

Until the 2026-08-21 import into `monumental-archive` this repository
released each crate on its own version line, and release-plz wrote a
changelog per crate. Those files are kept exactly as they were:

- [`crates/edtf-core/CHANGELOG.md`](crates/edtf-core/CHANGELOG.md)
- [`crates/edtf-calendars/CHANGELOG.md`](crates/edtf-calendars/CHANGELOG.md)
- [`crates/edtf-normalize/CHANGELOG.md`](crates/edtf-normalize/CHANGELOG.md)
- [`crates/edtf-cli/CHANGELOG.md`](crates/edtf-cli/CHANGELOG.md)
- [`crates/edtf-wasm/CHANGELOG.md`](crates/edtf-wasm/CHANGELOG.md)
- [`crates/edtf-postgres/CHANGELOG.md`](crates/edtf-postgres/CHANGELOG.md)

From the first unified release the repository has one version, inherited
by every member, and
this file is the changelog. It is written by the release machinery from the
commits in each release's range; edit the commits, not this file.
