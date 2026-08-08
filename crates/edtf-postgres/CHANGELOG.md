# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.2.1](https://github.com/CarlAllenn/edtf/compare/edtf-postgres-v1.2.0...edtf-postgres-v1.2.1) - 2026-08-08

### Other

- No code changes. Pipeline-proof release: completes the v1.2.0 release,
  whose publish run was lost to a CI hang after the crates reached the
  registries ([#125](https://github.com/CarlAllenn/edtf/pull/125) records
  the incident). v1.2.0's GitHub releases remain drafts; this release
  carries the full assets.

## [1.2.0](https://github.com/CarlAllenn/edtf/compare/edtf-postgres-v1.1.2...edtf-postgres-v1.2.0) - 2026-08-08

### Added

- publish edtf-postgres as an OCI image ([#82](https://github.com/CarlAllenn/edtf/pull/82)) ([#96](https://github.com/CarlAllenn/edtf/pull/96))

### Other

- *(deps)* batch dependency updates ahead of v1.2.0 ([#119](https://github.com/CarlAllenn/edtf/pull/119))
- bring the pipeline descriptions up to date with #108 ([#109](https://github.com/CarlAllenn/edtf/pull/109))
- SPDX headers, code-review standard, small tasks, security review ([#101](https://github.com/CarlAllenn/edtf/pull/101))

## [1.1.1](https://github.com/CarlAllenn/edtf/compare/edtf-postgres-v1.1.0...edtf-postgres-v1.1.1) - 2026-07-31

### Fixed

- close all 39 confirmed findings from the release-pipeline audit ([#76](https://github.com/CarlAllenn/edtf/pull/76))

## [1.1.0](https://github.com/CarlAllenn/edtf/compare/edtf-postgres-v1.0.2...edtf-postgres-v1.1.0) - 2026-07-31

### Added

- prebuilt edtf-postgres artifacts, attested and smoke-tested ([#72](https://github.com/CarlAllenn/edtf/pull/72))

## [1.0.2](https://github.com/CarlAllenn/edtf/compare/edtf-postgres-v1.0.1...edtf-postgres-v1.0.2) - 2026-07-31

### Changed

- no library changes — republished through the hardened release pipeline:
  in-run provenance self-verification and per-crate SBOM attachment
  ([#67](https://github.com/CarlAllenn/edtf/pull/67), [#69](https://github.com/CarlAllenn/edtf/pull/69))

## 1.0.1 - 2026-07-31

### Other

- #54 edtf-postgres becomes a workspace member ([#59](https://github.com/CarlAllenn/edtf/pull/59))

## 1.0.0 - 2026-07-27

Initial release, published to crates.io as part of the v1.0.0 release of the
`edtf` crate family.

Unlike the other five crates it received no git tag, no GitHub release and no
changelog entry at the time: `edtf-postgres` sat outside the release-plz
workspace, so nothing owned its versioning. That is also why the heading above
carries no comparison link — there is no `edtf-postgres-v1.0.0` tag to compare
against, and one cannot honestly be created after the fact.

Fixed in 1.0.1: the crate is now a workspace member, and release-plz owns its
version, tag, changelog and release exactly like the other five
([#54](https://github.com/CarlAllenn/edtf/issues/54)).
