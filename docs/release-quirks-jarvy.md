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
cosign verify-blob \
  --signature jarvy-linux-x86_64.tar.gz.sig \
  --certificate jarvy-linux-x86_64.tar.gz.pem \
  --certificate-identity-regexp "https://github.com/Cliftonz/Jarvy/" \
  --certificate-oidc-issuer "https://token.actions.githubusercontent.com" \
  jarvy-linux-x86_64.tar.gz
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

## Release published as draft

The workflow sets `draft: true` on `softprops/action-gh-release`. After the
workflow completes, manually verify all artifacts are present and signatures
verify, then publish the draft:

```bash
gh release edit vX.Y.Z --draft=false
```

This is an extra manual step compared to the skills' "tag pushes → release
publishes" assumption. The point of no return is the **draft publish**, not
the tag push — though once the tag is pushed the assets are built and
signed atomically.

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

## Universal install scripts are also broken pending tarballs

`dist/scripts/install.sh` (line 282) builds:

```bash
https://github.com/${JARVY_REPO}/releases/download/v${version}/jarvy-v${version}-${platform}.tar.gz
```

`dist/scripts/install.ps1` builds the same pattern with `.zip`.
Neither of these asset shapes is produced by `release.yml` — same
gap that breaks Homebrew. Install scripts have been broken since
v0.0.1; symptom is `curl: (56) The requested URL returned error:
404` immediately after the "Download URL" log line.

Confirmed on a fresh macOS arm64 machine 2026-05-12 trying to
install rc.10 via `JARVY_CHANNEL=beta curl … | bash`. The
`JARVY_CHANNEL` env var also doesn't propagate when set on the
left of the curl invocation (sets it for curl, not for the piped
bash) — but even if it did, the URL 404s. Use
`curl … | JARVY_CHANNEL=beta bash` if you need the channel to
take effect, and combine that with the upstream tarball fix.

Workaround for users right now: `cargo install --git
https://github.com/Cliftonz/Jarvy --tag v0.1.0-rc.10 jarvy`
(compiles from source).

This needs to be fixed before any soak claims `install.sh` /
`install.ps1` cohort coverage — Path 1 (fresh install) cannot
exercise these channels until tarballs are produced. Add
`dist/scripts/install.{sh,ps1}` to the channels excluded from
required cohort coverage in `docs/release-testing.md` until the
upstream tarball fix lands (same blocker as Homebrew).

## Homebrew pipeline is non-functional pending tarball artifacts

The Homebrew formula in `Cliftonz/homebrew-tap` is authored against
`.tar.gz` tarball assets (`jarvy-v<version>-<arch>-<os>.tar.gz`) that
`release.yml` does **not** produce. Verified on `v0.0.5`:

```bash
gh release view v0.0.5 --json assets --jq '.assets[].name' | grep tar.gz
# (no output — no tarballs in the release)
```

Actual artifact set: `.dmg` (macOS), `.deb` (Linux deb-based),
`.rpm` (Linux rpm-based), `.msi` / `.exe` (Windows), `.AppImage`
(Linux portable), `SHA256SUMS.txt`, SBOMs. None of these match the
formula's URL pattern.

Consequences:

1. `publish-packages.yml::update-homebrew::Update formula` step greps
   `SHA256SUMS.txt` for `x86_64-apple-darwin` (etc.) — these substrings
   never appear, so the placeholder substitution writes empty SHAs.
2. `publish-packages.yml::update-homebrew::Push to Homebrew tap` is
   additionally gated on the `HOMEBREW_TAP_DEPLOY_KEY` secret, which
   is currently unset. The step has been **skipped** on every release
   (v0.0.1 → v0.0.5). The tap repo's `jarvy.rb` still contains literal
   `VERSION_PLACEHOLDER` / `SHA256_PLACEHOLDER_*` strings from initial
   setup on 2026-01-18.
3. `brew install jarvy` (or `brew install Cliftonz/tap/jarvy`)
   currently fails — the formula is unparseable and references
   non-existent URLs.

Until the pipeline is rebuilt, Homebrew is **not a viable distribution
channel** and should not count toward
[`docs/release-testing.md#cohort-environment`](release-testing.md#cohort-environment)
coverage. macOS soak coverage runs through `install.sh` and `cargo install`
instead.

Three real fix paths (none done as of this writing):

1. Add `.tar.gz` artifact production to `release.yml` for macOS arm64,
   macOS x86_64, Linux x86_64, Linux arm64. Formula then works after
   the `HOMEBREW_TAP_DEPLOY_KEY` secret is configured per
   `docs/MAINTAINER_RELEASE_GUIDE.md`.
2. Rewrite the formula as a Homebrew Cask that installs from the
   `.dmg` asset (macOS-only; doesn't help Linux brew users).
3. Rewrite the formula to compile from crates.io via `Language::Rust`
   (slow user-side install, simplest pipeline — no new artifact work).

Path 1 is the cleanest long-term — adds Homebrew to real cohort
coverage and lets the existing publish-packages workflow finish what
it started. Tracked separately; not a v0.1.0 blocker.

## Multi-channel propagation

Jarvy ships through eight distribution channels. The skills assume one
GitHub release plus one downstream catalog (TrueNAS apps). Jarvy's channel
propagation is documented in
[`docs/release-testing.md`](release-testing.md#post-stable-channel-propagation).
Summary:

| Channel | Auto / Manual |
|---|---|
| GitHub release | Auto (CI) |
| crates.io | Auto (`publish-packages.yml::publish-crates-io` job) |
| Homebrew tap | Auto (CI PR to `Cliftonz/homebrew-tap`) |
| `.deb`, `.rpm`, `.msi`, `.dmg`, `.AppImage` | Auto (release.yml asset) |
| AUR (`jarvy-bin` and `jarvy`) | Manual |
| winget | Manual (`wingetcreate`, then Microsoft approves) |
| Chocolatey | Manual (`choco pack` + `choco push`) |
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
