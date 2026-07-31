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
releases **as drafts**. It publishes nothing. Its final step pushes an
umbrella tag `v<version>` pointing at the same commit as the per-crate tags.

**Phase 2** (`publish.yml`, triggered by the umbrella tag) builds, publishes,
proves and attests — in one job whose `github.ref` *is* the tag. That is the
entire reason for the split: provenance records the ref of the run that signs,
and a run on `main` can only ever record `refs/heads/main` plus whatever commit
happened to be checked out. Publishing from the tag makes correct provenance a
property of the architecture instead of a hope.

Supporting rules:

- **Tag immutability ruleset on, for all tags.** The tag is the anchor every
  verification points at; a movable anchor is no anchor.
- **Releases are drafts until phase 2 finishes.** Immutability is applied
  when a release is *published*, not when it is created, so every asset has
  to be attached first. The old shape — publish in phase 1, attach SBOMs in
  phase 2 with `gh release upload --clobber` — is refused outright once
  immutable releases are on, because the clobber cannot touch a published
  immutable release. Drafts are the better shape anyway: a run that dies
  leaves nothing public, and assets appear together with the release.
  Both paths must set it — `git_release_draft` in `release-plz.toml` for the
  normal path, and `--draft` in `tag-release.sh`, which calls
  `gh release create` directly and is not covered by that setting.
- **A release published while immutable releases are off is mutable
  forever.** It cannot be retrofitted. The obvious guard — reading
  `GET /repos/{owner}/{repo}/immutable-releases` before publishing — is not
  available to CI: `administration` is not a grantable workflow permission
  scope, so `GITHUB_TOKEN` cannot read repository settings. Instead
  `publish-releases.sh` publishes the six releases **one at a time** and
  verifies each is immutable before continuing, so a disabled setting costs
  one release rather than six. Check it by hand before releasing:
  `gh api repos/<owner>/<repo>/immutable-releases`.
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
   rulesets. Enable immutable releases too —
   `gh api --method PUT repos/<owner>/<repo>/immutable-releases`, a bodyless
   PUT; passing `-f enabled=true` is rejected with
   `"enabled" is not a permitted key` — **after** the draft-release changes
   are merged, never before: with the old publish-then-attach shape, turning
   it on breaks asset upload outright. Confirm with
   `gh api repos/<owner>/<repo>/immutable-releases`.
5. If the repository ships a Postgres extension, add an upgrade script per
   release under `crates/<ext>/sql/` — `assert-upgrade-path.sh` runs in the
   lint gate and names the file it expects. An empty file is correct when the
   SQL surface did not move, which `task pg:schema-snapshot` decides.
6. If runners enforce an egress allowlist (harden-runner `egress-policy:
   block`), the publish workflow needs, beyond the registries and toolchain
   hosts: `fulcio.sigstore.dev`, `rekor.sigstore.dev`,
   `tuf-repo-cdn.sigstore.dev`, **`tuf-repo.github.com`** and
   **`tmaproduction.blob.core.windows.net`**. The last two are GitHub's own
   TUF trust root and its target store — `gh attestation verify` fetches them,
   and without them self-verification fails on every artifact while the real
   error stays invisible unless stderr is surfaced.
7. Run the rehearsal before the first release: dispatch `publish.yml` from a
   branch with `dry_run=true`. It exercises packaging, determinism, version
   agreement and the full extension build-and-smoke-test matrix, and prints
   the provenance a real release would record. The extension matrix is the
   newest and heaviest machinery in the pipeline, and the rehearsal is the
   only place it can be proven without releasing.

## Normal release

1. Before merging: the release PR bumps the version, so `edtf-postgres` needs
   its upgrade script for the new version — `assert-upgrade-path.sh` fails the
   lint gate and names the file. Empty is correct unless
   `task pg:schema-snapshot` shows the SQL surface moved.
2. release-plz maintains the release PR. **Merging it is the commitment
   point.** Everything downstream is automatic: phase 1 tags and cuts draft
   releases, the umbrella tag fires phase 2, phase 2 builds and smoke-tests
   the extension matrix, publishes/proves/attests/verifies/canaries, attaches
   SBOMs and extension tarballs, and publishes the releases last.
3. Watch phase 2 to completion. Green means: registry bytes are byte-identical
   to what the run built, every attestation was re-verified in-run to name
   `refs/tags/v<version>` and the tagged commit, every extension tarball
   installed into a clean Postgres as a non-superuser on two Debian releases,
   and all six releases are public and immutable.

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

**The pipeline itself is broken in the tag** (v1.0.1's self-verify, and again
at v1.1.0): the tag freezes the workflow and scripts, and immutability means
that copy can never be fixed. If the defect is in verification rather than
publishing, do the verification out-of-band (the spot-check above, plus the
canary script run locally), document it on the tracking issue, fix `main`, and
let the next version demonstrate the green path. Never lift the ruleset to
move a tag — attestations name the tagged commit, and a moved tag breaks every
one of them.

v1.1.0 is the worked example, and the one place where hand-finishing was
correct. `canary-extension.sh` was committed non-executable, so phase 2 died
at `Permission denied` — *after* six crates reached crates.io, npm was
published, every attestation was created and self-verified, and all assets
were attached, but *before* `publish-releases.sh`. Because that step was
skipped, the six releases were still drafts: nothing public, nothing wrong,
and re-dispatching would only have re-run the same frozen broken script.

The completion was, in order:

1. Download every attached asset and check it against the released
   `SHA256SUMS` — all ten.
2. `gh attestation verify` one tarball pinned to `refs/tags/v1.1.0` and the
   publishing workflow.
3. Install the released bytes into a clean Postgres via
   `smoke-extension.sh` — non-superuser, `extversion`, full corpus.
4. Only then run `publish-releases.sh` locally to publish the six drafts.

The order is the point: everything the frozen canary would have proven was
proven first, by hand, against the real published artifacts — and the step
that makes a release permanent ran last, exactly as it does in the workflow.

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
| Script committed `100644` | release died at `Permission denied` after crates.io and npm had published (v1.1.0) | `assert-scripts-executable.sh` gates the git index mode |
| `releases/tags/{tag}` 404s on drafts | the step that publishes drafts could not read the drafts it exists to publish | list endpoint, which returns drafts and exposes `.immutable` |
| Docker Hub blob host unlisted | image pull refused against a bare IP; the hostname appears only in harden-runner's log | both `cloudflare` and `cloudfront` listed, in every job that runs docker |
| `release-pr` raced `tag` on the merge commit | a duplicate Release PR opened for the version being released (#74) | `needs: tag` serialises phase 1 |
| markdownlint MD024 scoped document-wide | generated changelogs went red on the first repeated section type | `siblings_only` |
| `fuzz/` excluded, so release-plz cannot bump its lockfile | stale pin after every version bump, silently | `cargo metadata --locked` in the lint gate |

Two further defects belong in this list but not in that table, because they
were caught *before* they cost a release. The first: enabling immutable releases
against the publish-then-attach shape would have refused every asset upload
and left releases uncompletable. It was found by reading the pipeline end to
end while planning issue #55, not by shipping. That is the cheaper way to
find them, and it is available for the asking.

The second: the Docker Hub allowlist gap was caught by the dry-run
rehearsal rather than by the release. It also carries a lesson of its own —
the arm64 legs passed while every amd64 leg failed, because harden-runner
does not enforce egress on the arm64 runners. A green arm64 job is not
evidence that an allowlist is complete; judge egress on amd64.

The meta-lesson: **every defect in the table was invisible to CI and appeared
only during a real release.** Treat the first release through a new pipeline as the
test it actually is — schedule it when a failed run is cheap, keep each step
independently resumable, and make every check print the evidence for its
verdict rather than a summary of it.
