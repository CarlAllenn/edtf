# Release runbook — tag-anchored publishing with honest provenance

How this repository releases, why it is shaped this way, and how to reproduce the
setup in another repository without rediscovering its defects. Every rule below
was paid for: v1.0.0 shipped attestations naming a commit that built none of the
bytes (issue #54), and v1.0.1 surfaced seven further defects, each observable
only during a real release (issue #66).

## The invariant

> Package → publish → prove local bytes == registry bytes → attest → verify the
> attestation names the tag → consume as a stranger.

Attestation happens **last** and **only on proof**. A run that attests before
proving, or attests bytes it downloaded rather than built, produces a signature
that verifies green while asserting something false — and Sigstore is
append-only, so a wrong attestation is permanent. That is not a hypothetical:
the v1.0.0 attestations are misattributed forever.

## Architecture: two phases split by a tag

**Phase 1** (`release-plz.yml`, runs on pushes to `main`) tags and cuts GitHub
releases. It publishes nothing. Its final step pushes an umbrella tag
`v<version>` pointing at the same commit as the per-crate tags.

**Phase 2** (`publish.yml`, triggered by the umbrella tag) builds, publishes,
proves and attests — in one job whose `github.ref` *is* the tag. That is the
entire reason for the split: provenance records the ref of the run that signs,
and a run on `main` can only ever record `refs/heads/main` plus whatever commit
happened to be checked out. Publishing from the tag makes correct provenance a
property of the architecture instead of a hope.

Supporting rules:

- **Tag immutability ruleset on, for all tags.** The tag is the anchor every
  verification points at; a movable anchor is no anchor.
- **The umbrella tag is pushed with a PAT**, not `GITHUB_TOKEN` — tags pushed
  with the default token do not trigger workflows (GitHub's recursion guard),
  and a release that silently triggers nothing looks exactly like a success.
  The per-crate tags are pushed with `GITHUB_TOKEN` for the same reason
  inverted: they are *meant* to trigger nothing.
- **Every step is idempotent.** Publishes skip versions already on the
  registry; tag creation skips existing tags that point at the right commit and
  fails on ones that do not. A run that dies partway is resumed by
  re-dispatching at the same tag, never by hand-finishing.
- **Dry-run rehearsal exists and is reachable** (`workflow_dispatch` with
  `dry_run=true`), and its post-condition compares registry state before/after
  rather than asserting absence — a rehearsal is usually run at a version that
  is already live.

## One-time setup for a new repository

1. Copy `release-plz.toml`, `.github/workflows/release-plz.yml`,
   `.github/workflows/publish.yml` and `.github/scripts/` (the tag, publish,
   verification and canary scripts). Enumerate your crates explicitly in the
   scripts — a glob is what let a crate be silently skipped at v1.0.0.
2. Create the fine-grained PAT (`RELEASE_PLZ_TOKEN`: contents + pull-requests
   write) and add it as a repository secret. It creates release PRs that
   trigger CI, and pushes the umbrella tag.
3. **Configure trusted publishing on every registry to name the *publishing*
   workflow** — `publish.yml`, not the workflow that opens release PRs. The
   OIDC exchange binds token issuance to a workflow filename; if it names the
   wrong file, the failure appears only at the first real publish, as
   `Trusted Publishing config … does not match the workflow filename`.
   crates.io: per-crate Settings → Trusted Publishing (also tick "require
   trusted publishing"). npm: package Settings → Trusted Publisher.
4. Enable the tag-immutability ruleset (all tags) and, if used, signed-commit
   rulesets.
5. If runners enforce an egress allowlist (harden-runner `egress-policy:
   block`), the publish workflow needs, beyond the registries and toolchain
   hosts: `fulcio.sigstore.dev`, `rekor.sigstore.dev`,
   `tuf-repo-cdn.sigstore.dev`, **`tuf-repo.github.com`** and
   **`tmaproduction.blob.core.windows.net`**. The last two are GitHub's own
   TUF trust root and its target store — `gh attestation verify` fetches them,
   and without them self-verification fails on every artifact while the real
   error stays invisible unless stderr is surfaced.
6. Run the rehearsal before the first release: dispatch `publish.yml` from a
   branch with `dry_run=true`. It exercises packaging, determinism and version
   agreement, and prints the provenance a real release would record.

## Normal release

1. release-plz maintains the release PR. **Merging it is the commitment
   point.** Everything downstream is automatic: phase 1 tags, the umbrella tag
   fires phase 2, phase 2 publishes/proves/attests/verifies/canaries and
   attaches SBOMs to the per-crate releases.
2. Watch phase 2 to completion. Green means: registry bytes are byte-identical
   to what the run built, and every attestation was re-verified in-run to name
   `refs/tags/v<version>` and the tagged commit.

Afterwards, spot-check from any machine:

```bash
gh attestation verify <artifact> --repo <owner>/<repo> \
  --source-ref refs/tags/v<version> --source-digest <tagged-commit-sha> \
  --signer-workflow "<owner>/<repo>/.github/workflows/publish.yml"
```

A bare `gh attestation verify` is **not** sufficient — the misattributed
v1.0.0 artifacts pass it today. The three pinning flags are the check.

## Recovery paths

**Phase 2 died partway** (network, registry lag, anything after tagging):
re-dispatch at the tag — `gh workflow run publish.yml --ref v<version>
-f dry_run=false`. Idempotency skips what already succeeded. Side effect: the
attest steps re-run, so artifacts can carry duplicate byte-identical
attestations. Harmless, but prefer fixing forward over redundant dispatches.

**A defect was found after the release PR merged** (the release is stranded:
manifests bumped, nothing tagged, nothing published): merge the fix as an
ordinary PR, then dispatch phase 1 — `gh workflow run release-plz.yml --ref
main`. The dispatch job creates the per-crate tags and releases at the head of
`main`, guarded by "not on the registry, not yet tagged", then pushes the
umbrella tag as usual. This exists because release-plz refuses to tag any
commit that is not a release-PR merge commit, so without it the automated path
has no way back.

**The pipeline itself is broken in the tag** (the failure mode of v1.0.1's
self-verify): the tag freezes the workflow and scripts, and immutability means
that copy can never be fixed. If the defect is in verification rather than
publishing, do the verification out-of-band (the spot-check above, plus the
canary script run locally), document it on the tracking issue, fix `main`, and
let the next version demonstrate the green path. Never lift the ruleset to
move a tag — attestations name the tagged commit, and a moved tag breaks every
one of them.

**Nothing releasable but the pipeline changed**: release-plz opens no release
PR for CI-only changes. Hand-author the bump (manifests, workspace dependency
versions, lockfiles — including any out-of-workspace lockfile such as
`fuzz/Cargo.lock` — and a changelog section per crate), merge it, then use the
phase-1 dispatch above, since a hand-authored PR is not a release-PR merge
commit either.

## Defect ledger

Failure modes this design now absorbs, each found during a live release:

| Defect | Symptom | Absorbed by |
| --- | --- | --- |
| Publish on `main`, resume re-attested downloads | provenance names a commit that built nothing (v1.0.0, permanent) | two-phase split; attest-last-on-proof |
| Umbrella guard compared commit SHAs | release-plz releases from the PR head, not the squash commit — every release skipped, green | guard asks "is this version published?" instead |
| Guards ran before checkout / asserted absence | rehearsal unreachable or failing at live versions | guard ordering; state comparison |
| Sibling crates resolved from the registry during packaging | package verifies against the *previous* release's sibling | package siblings together, explicitly |
| Trusted publisher named the tagging workflow | OIDC refused at first real publish | trusted publishers name `publish.yml` |
| Single `npm view` immediately after publish | successful publish reported as failure (read-after-write lag) | polling post-check |
| GitHub TUF hosts absent from egress allowlist; stderr swallowed | every attestation "does not match" when the truth was "could not fetch trust root" | allowlist entries; surfaced stderr; retries |
| Upload targeted a release that is never created | SBOM step aimed at an umbrella release; only per-crate releases exist | per-crate SBOM attachment |
| Release-PR-merge-only tagging | any post-merge fix strands the release | phase-1 `workflow_dispatch` recovery |

The meta-lesson: **every defect here was invisible to CI and appeared only
during a real release.** Treat the first release through a new pipeline as the
test it actually is — schedule it when a failed run is cheap, keep each step
independently resumable, and make every check print the evidence for its
verdict rather than a summary of it.
