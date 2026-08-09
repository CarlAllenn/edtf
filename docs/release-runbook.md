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

**What is portable, and what is not.** These scripts are generic:
`package-all`, `assert-deterministic`, `assert-publish-ref`,
`assert-nothing-published`, `assert-scripts-executable`, `resolve-*`,
`snapshot-registry-state`, `publish-crates`, `publish-npm`,
`verify-registry-bytes`, `self-verify-attestations`, `canary`,
`upload-sboms`, `tag-release`, `push-umbrella-tag`, `publish-releases` —
along with both workflows and `release-plz.toml`. Crate names are enumerated
inside them deliberately (a glob is what silently skipped a crate at
v1.0.0), so every one needs its list edited.

These are edtf-specific and should be deleted unless the repository also
ships a Postgres extension: `base-images` (the digest-pin table the
build and smoke scripts source), `verify-rustup-pin` (and its rustup
custom manager in `renovate.json`), `build-extension*`, `smoke-extension`,
`upgrade-smoke-extension`, `upload-extension-assets`, `canary-extension`,
`assert-upgrade-path`, `schema-snapshot`, the `sql/` directory, the
`extension` matrix job, and the extension half of `prepare-release` —
plus the "pinned base images" custom manager in `renovate.json`. The
same goes for the CLI binary leg (`build-cli`, `upload-cli-assets`, the
`cli` matrix job) if there is no binary to ship; `checksum-artifacts`
serves both legs and needs its target lists edited. The
`lint:fuzz` gate goes too if there is no `fuzz/`.

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
   Pin the **environment** too (`publish` here): the publish job runs in a
   reviewer-less deployment environment purely so the OIDC claim can be
   narrowed to that one job — with the environment named on the registry
   side, no other workflow in the repository can mint a publish token.
   Order matters: create the environment and merge the workflow change
   first, then set the environment on the registries; an unset registry
   field accepts any claim, so this order is safe and the reverse is not.
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
8. **Optional — Zenodo archival, for a citable DOI.** Worth doing only for
   software people actually cite; for edtf they do, since its users are
   libraries, archives and digital-humanities projects. Sign in to
   [Zenodo](https://zenodo.org) with GitHub, authorise the integration, and
   switch the repository on in
   [Settings → GitHub](https://zenodo.org/account/settings/github/). Every
   GitHub release then deposits a snapshot and mints a **version DOI**, under
   a stable **concept DOI** that always resolves to the newest version.

   The concept DOI is the one that belongs in the README badge and in
   `CITATION.cff` — a version DOI in either place is wrong within one
   release. Zenodo shows both on the deposit page; the concept one is
   labelled as resolving to all versions.

   Two facts set the order of operations. Zenodo archives only releases
   created *after* the switch is flipped, so the DOI does not exist until the
   next release — enabling this does nothing retroactively. And the deposit's
   metadata is read from the archived tree, so `CITATION.cff` wants to be in
   place first, or the first deposit records a guess made from the repository
   name. A `.zenodo.json` would override `CITATION.cff` and give finer
   control; it is deliberately absent, because two metadata files describing
   one project is a drift problem waiting to happen, and `CITATION.cff` is
   the one GitHub and every citation tool already read.

## Normal release

1. On the release branch, run **`task release:prepare`** and commit what it
   changes. Two things sit outside release-plz's reach and fail the lint gate
   on every single release until someone does them by hand: the extension
   upgrade script (`default_version` is `@CARGO_VERSION@`, so each release
   mints a version needing an `ALTER EXTENSION UPDATE` path) and
   `fuzz/Cargo.lock` (fuzz/ is excluded from the workspace, so its edtf-core
   pin goes stale). The task does both, and refuses to invent SQL — if
   `schema.snapshot.sql` changed, it stops and the migration is written by
   hand.
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

### One-off: point `COPY --from` consumers at `-artifact` (delete after use)

**This step is a transition note, not standing process. It applies to the
first release after #156, and whoever runs that release deletes this
subsection in the same PR that carries the note.** It is written down only
because it has to survive the gap between two releases; leaving it here
afterwards would calcify a one-time action into a step nobody remembers the
reason for.

Before merging the release PR, add a paragraph to the `edtf-postgres`
section of `crates/edtf-postgres/CHANGELOG.md`. release-plz uses that
section as the GitHub release body, so writing it there — rather than
editing the release afterwards — puts the note in front of consumers
without touching a release that is already published and immutable. Merging
is still the commitment point; nothing about that changes.

Roughly:

> If you consume this image as a build stage, switch `FROM …:<version>-pg<major>`
> to `…-artifact` and keep your `COPY --from` lines exactly as they are. The
> build-stage pull drops from ~164 MB to ~1.7 MB for byte-identical extension
> files, and the CVE surface goes from the base image's — refreshed only when
> edtf releases — to structurally empty. The `FROM` line keeps its shape, so
> Renovate semantics are unchanged. The runnable `:<version>-pg<major>` image
> keeps its `COPY --from` capability and is not deprecated; it is documented as
> a convenience for `docker run` and local trials.

Why it needs saying at all: #156 published `-artifact` and
`crates/edtf-postgres/README.md` now names it the supported build-stage
artifact, but existing `COPY --from` consumers of the runnable image keep
working — which is exactly the problem. A strict improvement nobody notices
is one nobody adopts, and the generated changelog line ("publish a
`FROM scratch` artifact variant") does not tell a consumer that it is about
them. The release body is the one channel that reaches them.

## Recovery paths

**Phase 2 died partway** (network, registry lag, anything after tagging):
re-dispatch at the tag — `gh workflow run publish.yml --ref v<version>
-f dry_run=false`. Idempotency skips what already succeeded. Side effect: the
attest steps re-run, so artifacts can carry duplicate byte-identical
attestations. Harmless, but prefer fixing forward over redundant dispatches.

v1.2.0, v1.2.1 and v1.2.2 all died mid-canary, and the cause was the same
every time: `canary.sh` exported `CARGO_HOME` to an empty scratch directory
and then invoked the toolchain. mise installs Rust **through rustup** and
provisions it against `CARGO_HOME`, so that hangs the first cargo call.
That call was `cargo new`, which sat *before* the script's first heartbeat
— which is why five deaths across three releases produced completely empty
step logs. Fixed by removing the repoint; v1.2.3 published clean, canary in
9s. `finish` runs on a fresh runner that never builds, so the job boundary
supplies the cold-fetch guarantee the repoint used to buy.

Four lessons, and the last three cost a day each.

First, the stranded state is identical whether the canary *fails* or the
runner *dies during* it: registries published and attested, six draft
releases with no assets — the same recovery dispatch covers both, so do not
let the messier failure mode suggest a different procedure.

Second, only lines already streamed to the Actions page survive a dead
runner, which is why the release scripts heartbeat their phase; when
reading a dead run, trust the stream, not the archive.

Third, **an empty log does not mean the step never started.** It was read
that way here for three releases. It is equally consistent with the step
stopping before its first write — so every call, including the cheap local
ones, needs a marker before it and a bound around it. `cargo new` had
neither because it looked too trivial to matter.

Fourth, **when production evidence is destroyed, stop reading production.**
Five failed releases taught nothing. Four lab jobs varying one thing each
settled it in ten minutes. Reach for `edtf-release-lab` on the second
unexplained failure, not the fifth.

Three theories were pursued and disproved. Record them so nobody spends the
day again: it was not runner memory exhaustion (the one "Out of memory."
annotation on v1.2.0 was real but incidental); it was not the CI provider
(the lab ran green *while* production hung); and it was not egress
enforcement — `block` and `audit` were measured identical, both stalling.
**Do not re-run a release with `egress-policy: audit` to discriminate.** It
discriminates nothing and costs a version number.

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
| `release-pr` ran on the release commit | a duplicate Release PR for the version currently publishing (#74, #78) — release-plz diffs against crates.io, which lags by the whole ~25-minute publish | `release-pr` skips release commits; `needs: tag` was a real but *different* fix |
| markdownlint MD024 scoped document-wide | generated changelogs went red on the first repeated section type | `siblings_only` |
| `fuzz/` excluded, so release-plz cannot bump its lockfile | stale pin after every version bump, silently | `cargo metadata --locked` in the lint gate |
| `--draft` releases create no git ref | recovery dispatch cut six drafts, zero tags, and exited GREEN having triggered nothing | `tag-release.sh` creates refs explicitly, then reads them back |
| `upload-sboms.sh` clobbered unconditionally | any resume after a partial publish died on an immutable release, permanently | read-back-and-skip, no `--clobber` |
| Assets skipped by NAME on resume | rebuilt tarballs are not bit-identical, so the release would carry run 1's bytes beside run 2's `SHA256SUMS` | overwrite while the release is a draft |
| Canary compared published bytes to LOCAL checksums | could not see a stale or mis-attached manifest — the one thing it existed to catch | both sides downloaded from the release |
| Upgrade path asserted by filename only | 0.2.0, 1.0.0 and 1.0.1 had no route to 1.1.0 while the gate stayed green | reachability over the whole graph, plus `pg_extension_update_paths` in the smoke test |
| `release-pr` raced `tag`; one pending run per group | a duplicate Release PR, and a merge commit's run could be displaced silently | `needs: tag`, and a per-commit concurrency group |
| cargo-deny skips were exact versions | every Renovate lockfile patch bump turned the gate red | ranges over the lagging minor |
| SBOM generation hoisted above the publish | cargo-cyclonedx writes beside each manifest, so the tree was dirty and `cargo publish` refused on the FIRST crate (v1.1.1, nothing published) | files are moved, not copied, and a clean-tree assertion runs before the publish |
| Two chores release-plz cannot do | the upgrade script and `fuzz/Cargo.lock` failed the gate on every single release PR | `task release:prepare` does both and proves the result |
| No deadline on any network fetch or job | the v1.2.0 canary sat ~40 minutes, then the runner died and took the archived logs with it | per-job `timeout-minutes`; `CARGO_HTTP_TIMEOUT` + `timeout` caps and bounded retries on every fetch; streamed heartbeats so a dead runner leaves a last known position |
| The canary's bounds and heartbeats started at probe 1 | `cargo new` — the scaffold, judged too trivial to instrument — was the call that actually stalled, so three releases died with an empty log and the cause stayed invisible | every call gets a marker before it and a bound around it, cheap local ones included |
| The canary repointed `CARGO_HOME` at an empty directory | mise provisions the Rust toolchain against `CARGO_HOME`, so the next cargo call hung forever; it cost v1.2.0, v1.2.1 and v1.2.2 | removed — `finish` runs on a fresh runner that never builds, so the job boundary already guarantees a cold fetch |

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
only during a real release.**

Note what that means for anyone copying this. The rows below the v1.0.x block
were all found in a single day, and three of them were introduced by the very
changes that fixed the others — each time in the half of the pipeline that
nothing can exercise except a live release: tag creation, asset upload,
draft publishing, recovery. The build half is covered by CI and by the
rehearsal; the release half is covered by nothing.

So this document transfers an architecture and a list of paid-for mistakes.
It does not transfer a *tested* pipeline, and it should not be read as one.
The missing piece is a scratch repository with throwaway crates where the
whole thing — including the recovery paths — can run end to end against real
GitHub APIs. Every defect added to this table today would have died there in
minutes instead of costing a release. Treat the first release through a new pipeline as the
test it actually is — schedule it when a failed run is cheap, keep each step
independently resumable, and make every check print the evidence for its
verdict rather than a summary of it.
