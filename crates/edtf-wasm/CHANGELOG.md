# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.2](https://github.com/CarlAllenn/edtf/compare/edtf-wasm-v1.0.1...edtf-wasm-v1.0.2) - 2026-07-31

### Changed

- no library changes — republished through the hardened release pipeline:
  in-run provenance self-verification and per-crate SBOM attachment
  ([#67](https://github.com/CarlAllenn/edtf/pull/67), [#69](https://github.com/CarlAllenn/edtf/pull/69))

## [1.0.0](https://github.com/CarlAllenn/edtf/compare/edtf-wasm-v0.2.0...edtf-wasm-v1.0.0) - 2026-07-27

### Added

- #8 lint canon — wholesale clippy tiers + full fallout fix ([#45](https://github.com/CarlAllenn/edtf/pull/45))
- tooling baseline — rustfmt canon, release-plz, doc/coverage gates ([#8](https://github.com/CarlAllenn/edtf/pull/8)) ([#43](https://github.com/CarlAllenn/edtf/pull/43))
- edtf-normalize — deterministic prose-date → EDTF normalizer (en+ru) ([#37](https://github.com/CarlAllenn/edtf/pull/37))
- adopt cargo-deny supply-chain baseline (renovate-config#5) ([#38](https://github.com/CarlAllenn/edtf/pull/38))

### Other

- #32 content pass — README normalize row, SECURITY.md, crate metadata ([#48](https://github.com/CarlAllenn/edtf/pull/48))
