# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.2.3](https://github.com/CarlAllenn/edtf/compare/edtf-calendars-v1.2.2...edtf-calendars-v1.2.3) - 2026-08-08

### Other

- No code changes. Carries the release assets the 1.2.x line never got:
  the v1.2.0, v1.2.1 and v1.2.2 publish runs all stalled in the registry
  canary, after their crates had reached the registries. All three remain
  valid, verifiable versions on crates.io/npm. The cause was the canary
  pointing CARGO_HOME at an empty directory while running a toolchain
  mise provisions against it, which hangs the first cargo call; fixed in
  #135 and reproduced under test before the fix landed.

## [1.2.2](https://github.com/CarlAllenn/edtf/compare/edtf-calendars-v1.2.1...edtf-calendars-v1.2.2) - 2026-08-08

### Other

- No code changes. Completes the 1.2.x line with full release assets:
  the v1.2.0 and v1.2.1 publish runs were lost to runner memory
  exhaustion during a CI provider incident, after their crates
  reached the registries. Both remain valid, verifiable versions on
  crates.io/npm; this is the release that carries the assets,
  through the split pipeline that makes the failure unrepeatable.

## [1.2.1](https://github.com/CarlAllenn/edtf/compare/edtf-calendars-v1.2.0...edtf-calendars-v1.2.1) - 2026-08-08

### Other

- No code changes. Pipeline-proof release: completes the v1.2.0 release,
  whose publish run was lost to a CI hang after the crates reached the
  registries ([#125](https://github.com/CarlAllenn/edtf/pull/125) records
  the incident). v1.2.0's GitHub releases remain drafts; this release
  carries the full assets.

## [1.2.0](https://github.com/CarlAllenn/edtf/compare/edtf-calendars-v1.1.2...edtf-calendars-v1.2.0) - 2026-08-08

### Other

- close the statement and branch coverage gaps ([#105](https://github.com/CarlAllenn/edtf/pull/105))
- SPDX headers, code-review standard, small tasks, security review ([#101](https://github.com/CarlAllenn/edtf/pull/101))

## [1.1.1](https://github.com/CarlAllenn/edtf/compare/edtf-calendars-v1.1.0...edtf-calendars-v1.1.1) - 2026-07-31

### Fixed

- close all 39 confirmed findings from the release-pipeline audit ([#76](https://github.com/CarlAllenn/edtf/pull/76))

## [1.0.2](https://github.com/CarlAllenn/edtf/compare/edtf-calendars-v1.0.1...edtf-calendars-v1.0.2) - 2026-07-31

### Changed

- no library changes — republished through the hardened release pipeline:
  in-run provenance self-verification and per-crate SBOM attachment
  ([#67](https://github.com/CarlAllenn/edtf/pull/67), [#69](https://github.com/CarlAllenn/edtf/pull/69))

## [1.0.0](https://github.com/CarlAllenn/edtf/compare/edtf-calendars-v0.1.0...edtf-calendars-v1.0.0) - 2026-07-27

### Added

- #8 lint canon — wholesale clippy tiers + full fallout fix ([#45](https://github.com/CarlAllenn/edtf/pull/45))
- tooling baseline — rustfmt canon, release-plz, doc/coverage gates ([#8](https://github.com/CarlAllenn/edtf/pull/8)) ([#43](https://github.com/CarlAllenn/edtf/pull/43))
- adopt cargo-deny supply-chain baseline (renovate-config#5) ([#38](https://github.com/CarlAllenn/edtf/pull/38))

### Other

- adopt max-enforcement linter baseline from renovate-config
