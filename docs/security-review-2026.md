# Security review — August 2026

A documented review of the project against its stated security
requirements ([SECURITY.md](https://github.com/monumental-archive/.github/blob/main/SECURITY.md)) and the security boundary
described in the [assurance case](assurance-case.md). Conducted
2026-08-08 by the maintainer with model-assisted adversarial review;
recorded here so the review, its findings and their dispositions are
auditable rather than asserted.

> **Editor's note, 2026-08-21.** This is a dated record of a review
> conducted on 2026-08-08, when the repository lived at
> `github.com/CarlAllenn/edtf`. The links and issue numbers below are left
> as they were written; GitHub redirects them to
> `monumental-archive/edtf`. Several of the mechanisms it credits —
> `upgrade-smoke-extension.sh`, the repository's own CodeQL workflow, its
> `publish.yml` — were replaced by the organisation's shared machinery when
> edtf was imported. What they were fixing is still fixed; what does the
> fixing changed. See monumental-archive/.github#669.

## Scope and method

- **Requirements reviewed**: no panic/non-termination on any input; no
  memory-unsafety; outputs valid by construction; artifacts verifiable
  end to end.
- **Boundary reviewed**: the input-string boundary (all five entry
  points), the process boundary per deployment shape, and the publish
  boundary (what CI builds vs what consumers fetch).
- **Method**: OpenSSF Scorecard breakdown as the external probe; an
  adversarial walkthrough of the release pipeline (an external review of
  the v1.1.2 pipeline, independently reproduced claim by claim — issue
  [#83](https://github.com/CarlAllenn/edtf/issues/83)); dependency and
  advisory audit (cargo-deny, Renovate state); coverage measurement
  (cargo-llvm-cov); and inspection of the CI trust posture (token scopes,
  egress policies, cache usage, action pinning).

## Findings and dispositions

| # | Finding | Disposition |
| --- | --- | --- |
| 1 | The extension upgrade chain (`ALTER EXTENSION UPDATE`) shipped in every tarball but had never been executed by CI, and the untested path was scheduled to run in a real downstream database | Fixed — `upgrade-smoke-extension.sh` executes the real upgrade from the previous published tarball in every release matrix cell ([#97](https://github.com/CarlAllenn/edtf/pull/97)) |
| 2 | Two unpinned download-then-run sites: `curl \| sh` of rustup inside the extension build, and a registry response piped into an interpreter | Fixed — version-pinned, SHA256-verified rustup-init; fetch-to-file for the registry response ([#92](https://github.com/CarlAllenn/edtf/pull/92)) |
| 3 | No SAST beyond clippy | Fixed — CodeQL (Rust + Actions) gating PRs and re-scanning `main` weekly ([#93](https://github.com/CarlAllenn/edtf/pull/93)) |
| 4 | Sigstore attestations existed only in GitHub's attestation store; releases themselves carried no signature material a consumer or scanner could see | Fixed — attestation bundles published as `*.intoto.jsonl` release assets ([#94](https://github.com/CarlAllenn/edtf/pull/94)) |
| 5 | The CLI was the one artifact with no prebuilt, attested binaries, forcing a checksum-less, provenance-less pin downstream | Fixed — native-built, tested, attested binaries for four targets ([#95](https://github.com/CarlAllenn/edtf/pull/95)) |
| 6 | RUSTSEC-2021-0127 (`serde_cbor` unmaintained, via pgrx) | Accepted — not exploitable exposure but an unmaintained-crate notice; documented ignore in `deny.toml` with upstream pointer, guarded so a stale ignore fails the build; re-reviewed when pgrx moves |
| 7 | The v1.0.0 attestations permanently misattribute their build commit | Accepted, documented — Sigstore is append-only; SECURITY.md documents the limitation and pinned verification correctly fails against v1.0.0 |
| 8 | `strip = "none"` on the shipped `.so` rests on a wrong rationale (dlsym uses `.dynsym`, which strip preserves) | Fixed — the shipped library is stripped at packaging and the debug info ships as a `-dbgsym` tarball per cell, Debian-style; the corrected rationale lives in `Cargo.toml` ([#108](https://github.com/CarlAllenn/edtf/pull/108)) |
| 9 | Branch protection lacked linear history and conversation resolution | Fixed — enabled 2026-08-08 |
| 10 | Statement coverage measured 90.0%; `edtf-normalize/src/lib.rs` is the weakest file (50%) | Noted — above the stated bars; the weak file is tracked as test-improvement work |

## Conclusion

The security requirements hold as stated: the never-panic and
memory-safety requirements are enforced by construction (forbid-unsafe)
and continuously exercised (fuzzing, property tests); the verifiability
requirement is now uniform across every artifact class. The residual
risks are those the assurance case names — spec-interpretation logic
errors and maintainer-account compromise — both mitigated and both
failing toward detectability.

Next review: within five years, or upon any material change to the
security boundary (a new deployment shape, a new dependency in the core,
or a change to the release pipeline's trust model).
