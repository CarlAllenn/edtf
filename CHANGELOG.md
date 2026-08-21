# Changelog

All notable changes to this project are recorded here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and versions follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html)
over the public APIs of the crates this repository publishes.

## History before v1.3.0 lives per crate

Until the 2026-08-21 import into `monumental-archive` this repository
released each crate on its own version line, and release-plz wrote a
changelog per crate. Those files are kept exactly as they were:

- [`crates/edtf-core/CHANGELOG.md`](crates/edtf-core/CHANGELOG.md)
- [`crates/edtf-calendars/CHANGELOG.md`](crates/edtf-calendars/CHANGELOG.md)
- [`crates/edtf-normalize/CHANGELOG.md`](crates/edtf-normalize/CHANGELOG.md)
- [`crates/edtf-cli/CHANGELOG.md`](crates/edtf-cli/CHANGELOG.md)
- [`crates/edtf-wasm/CHANGELOG.md`](crates/edtf-wasm/CHANGELOG.md)
- [`crates/edtf-postgres/CHANGELOG.md`](crates/edtf-postgres/CHANGELOG.md)

From v1.3.0 the repository has one version, inherited by every member, and
this file is the changelog. It is written by the release machinery from the
commits in each release's range; edit the commits, not this file.
