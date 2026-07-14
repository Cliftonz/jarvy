# Release Skill Quirks for Jarvy

The `cutting-pre-release`, `validating-pre-release`, and `cutting-release`
skills were authored for a Go project that uses different release plumbing
than Jarvy. This document records every divergence so the skills can be
applied here without confusion.

The canonical Jarvy release process is in
[`docs/release-testing.md`](release-testing.md). When this file disagrees
with that file, defer to that file.

## Workflow filename

- Skills reference `release.yaml`. Jarvy's file is `.github/workflows/release.yml`.
- Whenever a skill says `gh run list --workflow=release.yaml`, substitute
  `--workflow=release.yml` (or use the workflow's display name `Build and Release`).

## Tag format and matcher

Jarvy's release workflow triggers on:

```yaml
on:
  push:
    tags:
      - 'v[0-9]+.[0-9]+.[0-9]+'
      - 'v[0-9]+.[0-9]+.[0-9]+-*'
```

Both stable and pre-release tags fire the same workflow. The workflow's
prerelease-detection step inspects `${{ github.ref_name }}` for a `-` and
sets `prerelease: true` on the GitHub release accordingly.

## Tag signing — enforced

The workflow has a `Verify tag is signed` step that runs `git tag -v` and
fails the build if the tag is not signed. Use `git tag -s` (GPG) or
`git -c gpg.format=ssh tag -s` (SSH signing) when cutting any tag. Without
a signed tag, no release artifact is produced.

The maintainer's signing key (GPG public key or SSH allowed-signer line) must
be:

- Registered on the maintainer's GitHub profile (Settings → SSH and GPG keys
  for GPG; Settings → SSH and GPG keys → Signing for SSH)
- Configured locally in `~/.gitconfig` (`user.signingkey`, `gpg.format`,
  `commit.gpgsign`, `tag.gpgsign`)

Skill assumption holds: the workflow rejects unsigned tags, same as the
omni-infra-provider-truenas pattern.

## Cosign artifact signing — keyless OIDC

Jarvy uses [Sigstore cosign](https://docs.sigstore.dev/) keyless signing via
GitHub Actions OIDC. No long-lived keys are stored as secrets. The workflow
mints an ephemeral certificate per release run and signs every binary,
package, SBOM, and checksum artifact. Verification:

```bash
# Substitute a triple that matches your target — one of:
#   aarch64-apple-darwin, x86_64-unknown-linux-musl,
#   x86_64-unknown-linux-gnu, aarch64-unknown-linux-gnu,
#   armv7-unknown-linux-gnueabihf, x86_64-pc-windows-msvc (.zip).
ARTIFACT=jarvy-v0.5.1-x86_64-unknown-linux-musl.tar.gz
cosign verify-blob \
  --signature "${ARTIFACT}.sig" \
  --certificate "${ARTIFACT}.pem" \
  --certificate-identity-regexp "https://github.com/Cliftonz/jarvy/" \
  --certificate-oidc-issuer "https://token.actions.githubusercontent.com" \
  "$ARTIFACT"
```

Same pattern as `omni-infra-provider-truenas`. No skill divergence.

## Release notes — awk-extracted from CHANGELOG.md (omni pattern)

Jarvy uses the **same awk extraction** pattern as the
omni-infra-provider-truenas release workflow. The `Build release notes`
step in `.github/workflows/release.yml`:

1. Runs `awk` against `CHANGELOG.md` looking for the `## [vX.Y.Z]` (or
   `## [X.Y.Z]`) section that matches the tag, taking everything until
   the next `## [` header.
2. Falls back to `git log <prev-tag>..<tag> --pretty='- %s (%h)'` when
   no CHANGELOG entry exists. **This fallback is the intended path for
   pre-release tags** — by policy (see `CHANGELOG.md` "Policy" section),
   pre-releases do not get a CHANGELOG entry.
3. Appends a `**Full Changelog**: <compare-link>` line.
4. Appends Jarvy's standing footer (install commands, signature verify,
   SBOM info, tag-verify command).
5. Writes the result to `RELEASE_NOTES.md` and feeds it to
   `softprops/action-gh-release` via `body_path:`.

Implications:

- The `cutting-release` skill's "update CHANGELOG before tagging" step
  is **load-bearing** — if you forget, the awk extraction returns empty
  and the release falls through to `git log` notes, which read like a
  raw commit list rather than a curated narrative.
- The awk pattern is `/^## \[v?VERSION\]/` to `/^## \[/`. CHANGELOG
  entries must use exactly that header shape (`## [vX.Y.Z]` or
  `## [vX.Y.Z] — Title`); other formats won't match.
- The `cutting-pre-release` skill's "no provisional rc entry" rule
  applies for the *opposite* reason than its docs imply on Jarvy: it's
  not that an RC entry would be ignored, it's that RCs are **expected**
  to fall through to the git-log path. Adding an RC CHANGELOG entry
  would override the fallback and produce inconsistent rc release notes
  between iterations.

## Release auto-publishes — behind a draft-verification gate

**Updated 2026-07-14 (gate model):** the workflow creates a draft, uploads
all assets, then runs a **draft-verification gate** — re-downloads the
draft's assets via the API and checks checksums, cosign signatures, SBOM
shape, and a binary `--version` smoke against the tag — and only on green
auto-promotes to published (`--draft=false`, `--latest` for stables).

Implications:

- The tag push commits you to the *pipeline*, but a bad build now
  self-aborts at the draft: nothing user-facing happens, no withdrawal
  needed. Recovery from a red gate is delete draft + tag, fix, re-tag.
- Never hand-publish a draft around a red gate (`gh release edit
  --draft=false` by hand) — the gate is the checkpoint that replaced the
  manual verify step; bypassing it is how v0.6.1 class mistakes ship.
- Post-publish, verify-release.yml re-checks the PUBLIC assets and the
  rest of the fleet exercises the public install/upgrade paths — those
  are rollback triggers, not gates.

(History: pre-2026-07-13 the workflow stopped at the draft for a manual
publish; 2026-07-13 it auto-published with verification only afterwards —
which let the mislabeled v0.6.1 ship, see issue #62; the gate landed
2026-07-14.)

## `release:published` events use the tag's commit, not main HEAD

When `publish-packages.yml` (or any other workflow listening on
`release: published`) fires, GitHub Actions checks out the workflow YAML
from **the commit the release tag points at**, not the default branch
HEAD at event time. This breaks the intuitive "fix it on main before
publishing the draft" workflow.

Discovered when adding a prerelease gate to `publish-packages.yml`:

1. Cut `v0.1.0-rc.10` at commit `bcdff1d`.
2. Added a `!github.event.release.prerelease` gate to `publish-packages.yml`,
   committed as `e1bb4ca` on main.
3. Published the draft for `v0.1.0-rc.10`.
4. `publish-packages` ran, but `gh run view 25737549987 --json headSha`
   showed `headSha = bcdff1d` (the tag's commit) — not main HEAD. The
   gate didn't exist in that workflow file. `cargo publish` started
   running before the run was manually cancelled.

**Implications**:

- Workflow changes intended to affect a specific release **must be
  committed before the release tag is created**, and the release tag
  must descend from that commit.
- Re-running a cancelled `release: published` workflow run will replay
  against the same tag-SHA workflow file — fixes on main do not apply
  retroactively. To bring a fix to bear, cut a new tag (`-rc.(N+1)`)
  whose commit includes the fix.
- `workflow_dispatch` triggers DO use the workflow file from whichever
  branch/SHA the user dispatches against — so manual reruns can pick up
  main's fixes if invoked with the appropriate ref.

**Mitigation in this repo**: the prerelease gate landed in
`e1bb4ca` and is included in every rc cut from main going forward. The
risk window for `v0.1.0-rc.10` itself is closed because the run was
cancelled before `cargo publish` completed.

## Universal install scripts — FIXED (issue #30)

`dist/scripts/install.sh` and `.ps1` build:

```
jarvy-v${version}-${triple}.{tar.gz,zip}
```

Historically these 404'd because `release.yml` only shipped
`.dmg` / `.msi` / `.exe` / `.deb` / `.rpm` / `.AppImage`. As of
issue #30 (landed pre-v0.5.1), `release.yml` now also produces:

- `jarvy-v${VER}-aarch64-apple-darwin.tar.gz` (macOS arm64)
- `jarvy-v${VER}-x86_64-pc-windows-msvc.zip` (Windows)
- (Linux tarballs across all three triples were already shipping.)

Install scripts should Just Work starting with the first release
cut after this section landed. The `JARVY_CHANNEL` piping gotcha
(env var must be right of `curl … |`, not left) is unrelated to
tarballs and still applies.

## Homebrew pipeline — partially unblocked (issue #30)

Issue #30 added the `.tar.gz` / `.zip` artifacts the Homebrew
formula in `Cliftonz/homebrew-tap` was authored against. The
tarball naming convention now matches the formula's URL pattern:

- `jarvy-v${VER}-aarch64-apple-darwin.tar.gz`
- `jarvy-v${VER}-x86_64-unknown-linux-gnu.tar.gz`
- `jarvy-v${VER}-x86_64-unknown-linux-musl.tar.gz`
- `jarvy-v${VER}-aarch64-unknown-linux-gnu.tar.gz`

Two remaining blockers before `brew install Cliftonz/tap/jarvy`
works end-to-end:

1. **`HOMEBREW_TAP_DEPLOY_KEY` secret must be configured** so
   `publish-packages.yml::update-homebrew::Push to Homebrew tap`
   can commit the SHA-substituted formula to the tap repo.
   Configuration steps in `docs/MAINTAINER_RELEASE_GUIDE.md`.
2. **`jarvy.rb` in the tap repo must be reset** — as of the last
   audit it still contained literal `VERSION_PLACEHOLDER` /
   `SHA256_PLACEHOLDER_*` strings from initial setup 2026-01-18,
   because every prior update run silently skipped (secret unset).
   Any release cut after fix (1) will substitute the placeholders
   automatically.

Until (1) + (2) land, Homebrew still isn't a working distribution
channel. macOS soak coverage runs through `install.sh` (now
functional post-#30) and `cargo install`.

Path 1 is the cleanest long-term — adds Homebrew to real cohort
coverage and lets the existing publish-packages workflow finish what
it started. Tracked separately; not a v0.1.0 blocker.

## Package publish — dispatch wiring + per-channel bootstrap (2026-07-05 audit)

Cutting v0.5.0 surfaced three separate breakages in the package-publish
path. Two are fixed in-repo; two are one-time external setup that only a
maintainer with the relevant account/secret can do.

### `publish-packages.yml` was never triggered (FIXED)

`publish-packages.yml` keys on `release: published`. That event does
**not** fire downstream workflows when the release is published by the
release workflow's built-in `GITHUB_TOKEN` (GitHub's recursion guard —
same root cause as the `release-paths` / `verify-release` /
`prerelease-soak` dispatches documented above). v0.5.0's release was
published by `github-actions[bot]`, so nothing published — crates.io
had no 0.5.0 and `jarvy update` (cargo path) failed with
`could not find jarvy in registry with version =0.5.0`.

Fix (`release.yml::upload_to_release`):

1. Added `actions: write` to the job. Without it the `gh workflow run`
   dispatch step 403'd (`Resource not accessible by integration`) and
   `set -e` aborted — which had *also* been silently starving
   release-paths / verify-release / prerelease-soak of their per-release
   triggers (they ran on cron only).
2. Added a stable-only `gh workflow run publish-packages.yml` dispatch.
   Stable-only is load-bearing: publish-packages' crates-io gate reads
   `github.event.release.prerelease`, which is null on a
   `workflow_dispatch` and therefore treated as "not a prerelease" — a
   dispatch **always** publishes. An unguarded dispatch on an rc tag
   would push the stable `Cargo.toml` version to crates.io as latest.

v0.6.0 onward auto-publishes. To publish a release cut *before* the fix,
dispatch manually: `gh workflow run publish-packages.yml -f version=X.Y.Z`.

### AUR — SSH key must have no passphrase (needs maintainer)

Every release the `update-aur` job fails with:

```
Load key "/home/builder/.ssh/aur": incorrect passphrase supplied to decrypt private key
```

`KSXGitHub/github-actions-deploy-aur` has no passphrase input — the
`AUR_SSH_PRIVATE_KEY` secret must be an **unencrypted** private key.
Regenerate and re-register:

```bash
ssh-keygen -t ed25519 -C "aur@jarvy" -f aur_key -N ""   # -N "" = no passphrase
# add aur_key.pub to the AUR account (My Account → SSH keys)
# set repo secret AUR_SSH_PRIVATE_KEY = contents of aur_key, then delete local copies
```

### winget — package must be bootstrapped once (needs maintainer)

Every release the `update-winget` job fails with:

```
Package Jarvy.Jarvy does not exist in the winget-pkgs repository.
Please add atleast one version of the package before using this action.
```

`winget-releaser` only submits **updates** — it bases new manifests on
an existing version. `Jarvy.Jarvy` has never been added to
`microsoft/winget-pkgs`, so the first version is a one-time manual
submission (subject to the `public-pr-guard` skill — Microsoft
maintainers review the PR):

```bash
winget install wingetcreate
wingetcreate new https://github.com/Cliftonz/jarvy/releases/download/v0.5.0/jarvy_0.5.0_x64_en-US.msi
# identifier Jarvy.Jarvy → --submit
```

After that PR merges, the action handles all future versions. Two
adjacent fixes already applied to `publish-packages.yml`:

- Action owner renamed `vedantmgoyal2009` → `vedantmgoyal9`; pinned the
  new owner (redirect can lapse).
- `WINGET_TOKEN` must be a **classic** PAT with `public_repo` scope —
  winget-releaser (Komac) does not accept fine-grained PATs. The prior
  comment mis-documented it as fine-grained.

## Multi-channel propagation

Jarvy ships through eight distribution channels. The skills assume one
GitHub release plus one downstream catalog (TrueNAS apps). Jarvy's channel
propagation is documented in
[`docs/release-testing.md`](release-testing.md#post-stable-channel-propagation).
Summary:

| Channel | Auto / Manual |
|---|---|
| GitHub release | Auto (CI) |
| crates.io | Auto (`publish-packages.yml::publish-crates-io` job) — publishes via **Trusted Publishing** (OIDC), no stored token. Requires a one-time Trusted Publisher config on crates.io for **both** `jarvy` and `jarvy-templates` (owner `Cliftonz`, repo `jarvy`, workflow `publish-packages.yml`). Once a release proves it, the `CRATES_IO_TOKEN` secret can be deleted. |
| Homebrew tap | Auto (CI PR to `Cliftonz/homebrew-tap`) |
| `.deb`, `.rpm`, `.msi`, `.dmg`, `.AppImage` | Auto (release.yml asset) |
| AUR (`jarvy-bin`) | Auto-attempt (`publish-packages.yml::update-aur`) — blocked until the SSH key is re-keyed without a passphrase (see bootstrap section) |
| winget | Auto-attempt (`update-winget`) — blocked until `Jarvy.Jarvy` is bootstrapped with a one-time manual PR; updates auto after |
| Chocolatey | Auto (`publish-packages.yml::update-chocolatey`) |
| Universal install scripts (`install.sh`, `install.ps1`) | No update needed; scripts auto-fetch latest from GitHub API |

The `public-pr-guard` skill applies for winget submissions
(`microsoft/winget-pkgs` is a non-user-owned repository).

## Asset name patterns

The skills' install snippets use generic asset names. Jarvy's actual
patterns, set by the build matrix in `release.yml`:

| Platform | Asset pattern |
|---|---|
| macOS arm64 | `jarvy_<version>_aarch64.dmg` |
| macOS x86_64 | `jarvy_<version>_x64.dmg` |
| Linux x86_64 (musl) | `jarvy_<version>_amd64.deb`, `jarvy_<version>_x86_64.AppImage`, `jarvy-<version>-1.x86_64.rpm` |
| Linux arm64 | `jarvy_<version>_arm64.deb`, `jarvy-<version>-1.aarch64.rpm` |
| Linux armv7 | `jarvy_<version>_armhf.deb` |
| Windows x86_64 | `jarvy_<version>_x64_en-US.msi`, `jarvy_<version>_x64-setup.exe` |
| Tarball binaries (install scripts) | `jarvy-v<version>-<arch>-<os>.tar.gz` (Unix), `.zip` (Windows) |

Plus per-asset:
- `*.sig` — Sigstore signature
- `*.pem` — Sigstore certificate
- `*.bundle` — Sigstore bundle
- `SHA256SUMS.txt` — checksums file
- `sbom.spdx.json`, `sbom.cdx.json` — SBOMs

## Opt-in early-release channel

Jarvy has an opt-in beta channel that the skills don't model:

- Install scripts honor `JARVY_CHANNEL=beta` (or `nightly`) and fetch the
  latest GitHub prerelease via the `/releases` API instead of `/releases/latest`.
- `~/.jarvy/config.toml` `[update] channel = "beta"` makes `jarvy update` pull
  prereleases.
- `JARVY_UPDATE_CHANNEL=beta` env var overrides the config value at runtime.
- `jarvy update --channel beta` for one-shot beta installs.

The implementation is `src/update/release.rs::matches_channel` plus
`src/update/config.rs::Channel::matches_version`. Beta accepts `-rc` and
`-beta` suffixes; Nightly accepts everything; Stable rejects any version
containing `-`.

For the soak cohort to actually exist, this opt-in path must work — see
[Cohort Environment](release-testing.md#cohort-environment) in the canonical doc.

## What stays the same as the skills

These skill behaviors apply to Jarvy unchanged:

- "Tag is the point of no return" — the workflow builds and signs atomically
  on tag push.
- Pre-release tag identifier convention: `-rc.N` (default), `-beta.N`,
  `-alpha.N`. Always start at `.1` for a new target version.
- Per-version explicit authorization for tag pushes (don't reuse a prior
  session's approval).
- `cutting-pre-release` step "no provisional rc entry in CHANGELOG" applies
  with the same reasoning (RCs see auto-notes; CHANGELOG entry is written
  once when stable cuts).
- `validating-pre-release` overall flow (trigger matrix → matrix → soak →
  promotion criteria → catalog bump) — the canonical procedure
  ([`release-testing.md`](release-testing.md)) is structured the same way,
  just with Jarvy-specific paths.

## What the skills should not do on Jarvy

- Do **not** edit `microsoft/winget-pkgs` directly. Use `wingetcreate` and
  let Microsoft maintainers review the PR.
- Do **not** auto-publish the GitHub draft release until the maintainer has
  verified artifacts.
- Do **not** generate a CHANGELOG entry for an rc tag. The auto-notes are
  the contract for pre-releases.
- Do **not** assume the awk extraction step from the cutting-release skill
  applies — it does not.
