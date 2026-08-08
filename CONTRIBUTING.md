# Contributing

Contributions are welcome — issues, discussion and pull requests alike.
This document is the contract for what an acceptable contribution looks
like; the enforcement layer described below applies it mechanically, so
nothing here is aspirational.

## Development quick start

Tooling is pinned by [mise](https://mise.jdx.dev) and tasks run via
[Task](https://taskfile.dev):

```bash
git clone https://github.com/CarlAllenn/edtf
cd edtf
mise install   # installs the pinned toolchain and the git hooks
task ci        # the full gate: every linter + the full test suite
```

`mise install` also installs the lefthook git hooks; from then on every
commit and push runs the same gate CI runs. There are no bypass flags —
fixing tool output is never optional.

## Requirements for an acceptable contribution

- **One pull request per issue.** Branch from `origin/main`; stage
  explicit paths, never `git add -A`.
- **Conventional commits**, enforced by the commit-msg hook.
- **Signed commits.** A signed-commit ruleset is active on the repository;
  SSH or GPG signing both work.
- **Developer Certificate of Origin.** By adding `Signed-off-by` to your
  commits (`git commit -s`) you assert the
  [DCO](https://developercertificate.org/): that you are legally entitled
  to contribute the change under the project's licences (MIT OR
  Apache-2.0).
- **Tests are part of the change, not a follow-up.** New functionality
  comes with tests that exercise it; bug fixes come with a regression
  test that fails without the fix. As a matter of policy, tests MUST be
  added as functionality is added, and the practice to date is that
  substantially all fixes carry one.
- **The gate must be green.** `task ci` is identical to GitHub CI: clippy
  at maximum strictness with warnings as errors, rustfmt (a pinned
  nightly), cargo-deny, taplo, yamllint, markdownlint, codespell,
  editorconfig-checker, actionlint + zizmor, shellcheck + shfmt,
  cargo-machete and the full test suite.
- **Spelling registers**: en-US in code and API identifiers
  (`normalize`), en-GB in prose and docs (`normaliser`).
- **Parsing judgement calls are decisions, not comments.** Anything that
  interprets the ISO text gets a numbered D-decision in
  [docs/spec-notes.md](docs/spec-notes.md) (or an N-decision in
  [docs/normalize-notes.md](docs/normalize-notes.md) for the normaliser)
  — cite and extend those documents rather than deciding ad hoc in code.

## Coding standard

The coding standard is the workspace lint configuration itself:
`[lints]` in [Cargo.toml](Cargo.toml) (clippy `all` + `pedantic` + more,
warnings are errors, `unsafe_code = "forbid"`), [rustfmt.toml](rustfmt.toml),
and the per-format linter configs at the repository root. It is enforced
twice — locally by the hooks, remotely by required status checks on
`main` — so a change that violates it cannot merge.

## Reporting problems

Bugs and feature requests go to the
[issue tracker](https://github.com/CarlAllenn/edtf/issues).
Security problems go through
[private vulnerability reporting](https://github.com/CarlAllenn/edtf/security/advisories/new)
— see [SECURITY.md](SECURITY.md), which includes the response process and
reporter credit policy.
