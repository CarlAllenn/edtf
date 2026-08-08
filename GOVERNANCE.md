# Governance

## Model

The project uses the simplest governance model that is honest about how it
operates: a single maintainer ([@CarlAllenn](https://github.com/CarlAllenn))
makes all final decisions — a "benevolent dictator" model. Proposals,
disagreements and design discussion happen in the open, on the issue
tracker and in pull requests; the maintainer decides, and records
spec-interpretation decisions as numbered entries in
[docs/spec-notes.md](docs/spec-notes.md) and
[docs/normalize-notes.md](docs/normalize-notes.md) so they bind future
work rather than being re-litigated.

## Roles and responsibilities

- **Maintainer** (currently the only role held): triages issues, reviews
  and merges pull requests, cuts releases through the release pipeline
  (see [docs/release-runbook.md](docs/release-runbook.md)), responds to
  security reports per [SECURITY.md](SECURITY.md), and owns the
  D-/N-decision registers.
- **Contributors**: anyone submitting issues or pull requests under the
  requirements in [CONTRIBUTING.md](CONTRIBUTING.md). No CLA — the DCO
  sign-off is the legal mechanism.

Should the project gain regular contributors, committer status and this
document evolve with it; until then, documenting a committee that does
not exist would be less honest than documenting the dictatorship that
does.

## Access continuity

The practical single-maintainer risk is mitigated as follows:

- Everything needed to build, test and release lives in the repository:
  pinned toolchain (`mise.toml`/`mise.lock`), release pipeline
  (`.github/workflows/`), and a step-by-step
  [release runbook](docs/release-runbook.md) written so that a stranger
  can reproduce the setup, including registry trusted-publishing
  configuration.
- Publishing uses OIDC trusted publishing — there are no long-lived
  registry tokens to lose. Whoever controls the GitHub repository can
  release; no other secret exists apart from a fine-grained PAT that the
  runbook documents how to recreate.
- The maintainer's estate arrangements cover credential succession for
  the GitHub account, and GitHub's
  [deceased user policy](https://docs.github.com/site-policy/other-site-policies/github-deceased-user-policy)
  provides a fallback path for transferring the repository.
- The licences (MIT OR Apache-2.0) guarantee that, in the worst case, the
  project can be forked and continued by anyone without any legal
  transfer at all — and the runbook is written to make exactly that
  practical.
