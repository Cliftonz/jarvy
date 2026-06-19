# Jarvy Release Testing Process

Canonical procedure for validating a Jarvy pre-release before promoting it to a
stable tag. This document is the authoritative source — when the
`validating-pre-release` skill (or any other process) conflicts with this file,
defer to this file.

Jarvy ships through **eight distinct distribution channels** simultaneously
(crates.io, Homebrew tap, AUR, `.deb`, `.rpm`, winget, Chocolatey, universal
install scripts) and is invoked with **elevated privileges** to install
arbitrary system tools. A bad stable release lands directly in users' shells as
"Update Available" and can break their dev environments at scale. The whole
point of this process is to catch breakage on a controlled cohort before it
reaches the marketplace surface.

## Required Inputs

Before starting validation, confirm:

- **Pre-release tag** to validate (e.g. `v0.1.0-rc.1`). Must already exist on
  GitHub. If not, redirect to the `cutting-pre-release` skill.
- **Bump kind**: patch / minor / major. Determines soak duration and whether
  fault-injection drills are required.
- **Cohort environment**: see [Cohort Environment](#cohort-environment) below.

## Validation Checklist

Copy and track progress on the soak issue:

```
Validation Progress for vX.Y.Z-rc.N:
- [ ] Step 1: Confirm the trigger matrix actually requires validation
- [ ] Step 2: Open the soak tracking issue
- [ ] Step 3: Run pre-soak test matrix (fresh / upgrade / skip / rollback / multi-tool / asset-sweep)
- [ ] Step 4: Run major-only matrix (breaking-config + migration replay) [majors only]
- [ ] Step 5: Open the soak window; record signals as they appear
- [ ] Step 6: Run fault-injection drills [majors only]
- [ ] Step 7: Soak window elapses; evaluate promotion criteria
- [ ] Step 8: Promote to stable OR cut -rc.(N+1) and restart from Step 3
- [ ] Step 9: After stable ships, propagate to non-CI channels (AUR, winget, Choco)
```

## Trigger Matrix

Validation is **required** if the diff between the last stable tag and the
candidate tag touches any of these surfaces:

| Surface | Files / paths |
|---|---|
| Tool registry | `src/tools/registry.rs`, `src/tools/spec.rs`, `src/tools/common.rs` |
| Setup orchestration | `src/setup.rs`, `src/commands/setup_cmd.rs`, `src/bootstrap.rs` |
| Provisioner | `src/provisioner.rs`, `src/os_setup.rs` |
| Package deps | `src/packages/**` |
| Network/proxy | `src/network/**` |
| Self-update | `src/update/**` (especially `installer.rs`, `release.rs`, `signature.rs`) |
| Roles | `src/roles/**` |
| Drift detection | `src/drift/**` |
| Git config | `src/git/**` |
| Logging/ticket | `src/logging/**`, `src/ticket/**` |
| Telemetry | `src/telemetry.rs`, `src/observability/**` |
| Install scripts | `dist/scripts/install.sh`, `dist/scripts/install.ps1` |
| Distro packaging | `dist/homebrew/**`, `dist/debian/**`, `dist/rpm/**`, `dist/aur/**`, `dist/windows/**` |
| Release workflow | `.github/workflows/release.yml` |
| Cargo metadata | `Cargo.toml` (MSRV bump, license change, dep change in default features) |

Validation is **not required** (cut a normal patch via `cutting-release`) if
the diff only touches:

- `docs/**`, `Readme.md`, `CHANGELOG.md`, `CONTRIBUTING.md`, `prd/**`
- Comments-only changes
- Test-only changes (`tests/**`, `benches/**`) with full green CI
- A single new tool added via `define_tool!` whose e2e test is green and which
  has no dependencies on or from other tools

If a row matches, list it back to the maintainer so they can see *why*
validation is required. If a new surface needs to be covered going forward,
update this file as part of the same PR — the trigger matrix is the contract.

## Cohort Environment

The minimum viable cohort is the maintainer's own machines. The cohort exists
to catch breakage before it hits the public; it does not need to be large, but
it must cover the platforms Jarvy ships to.

**Required platforms** (every soak):

| Platform | Channels exercised |
|---|---|
| macOS arm64 (Apple Silicon, current major) | Homebrew, Cargo, install.sh |
| Ubuntu 22.04+ x86_64 (VM or container) | `.deb`, install.sh, Cargo |
| Fedora 40+ x86_64 (VM or container) | `.rpm`, install.sh, Cargo |
| Windows 11 x86_64 (VM) | winget, Chocolatey, install.ps1 |

**Note on macOS Intel (x86_64)**: jarvy v0.1.0+ does not ship prebuilt
Intel macOS binaries. Apple stopped selling Intel Macs in 2022; macOS 15+
retired most Intel hardware. The `install.sh` script detects Intel macOS
and **automatically falls through to `cargo install jarvy`** when cargo
is present (or prompts the user to install Rust via rustup if not).
Homebrew also remains a viable path (compiles from source on Intel).
Re-add `macos-13` to the release matrix if there is demand for prebuilt
Intel .dmg artifacts.

**Optional but recommended**:

- Arch Linux — for AUR validation
- Alpine Linux — to exercise the musl build path

**Opt-in beta cohort** (post-v0.1.0): users who set `JARVY_CHANNEL=beta` on
install or `[update] channel = "beta"` in `~/.jarvy/config.toml` self-select
into receiving pre-releases. Their feedback flows through the soak issue (see
[Signals to Watch](#signals-to-watch)). The maintainer's platforms above remain
the baseline regardless of how large the opt-in cohort gets.

**Channels temporarily excluded from cohort coverage**: any channel whose
distribution pipeline is documented as non-functional in
[`docs/release-quirks-jarvy.md`](release-quirks-jarvy.md) is excluded from
required coverage until its pipeline gap closes. As of v0.1.0 soak, that
list is:

- **Homebrew** — formula references `.tar.gz` assets that `release.yml`
  does not produce; `HOMEBREW_TAP_DEPLOY_KEY` also unset. Re-add when the
  pipeline produces real tarballs.
- **`install.sh` and `install.ps1`** — both scripts build URLs for
  `.tar.gz` (Unix) and `.zip` (Windows) tarballs that `release.yml`
  does not produce. Same root cause as the Homebrew gap; confirmed
  broken since v0.0.1. macOS / Linux coverage runs through
  `cargo install --git <repo> --tag <tag>` (compiles from source)
  instead. Re-add when the pipeline produces real tarballs.

## Pre-Soak Test Matrix

Run all five paths on the minimum platform set. Do not skip any. Even on a
"small" minor bump, the rollback path catches subtle state-machine bugs the
other paths miss.

### Path 1 — Fresh Install

State: clean VM with no Jarvy artifacts.

```bash
# Example for Ubuntu via install.sh + beta channel
JARVY_CHANNEL=beta curl -fsSL \
  https://raw.githubusercontent.com/bearbinary/jarvy/main/dist/scripts/install.sh | bash

jarvy --version  # confirms vX.Y.Z-rc.N
jarvy configure
jarvy setup      # against a representative jarvy.toml with 5–10 tools
```

Pass criteria:
- Install completes without error
- `jarvy --version` matches the rc tag
- `jarvy setup` exits 0 with all tools installed
- `~/.jarvy/state.json` written with correct `config_hash` and tool entries
- No panics in `~/.jarvy/logs/jarvy.log`

### Path 2 — Upgrade From N-1

State: clean VM with Jarvy at the previous stable version installed.

```bash
# Install N-1 first
JARVY_VERSION=<previous-stable> curl -fsSL \
  https://raw.githubusercontent.com/bearbinary/jarvy/main/dist/scripts/install.sh | bash
jarvy setup

# Upgrade to rc
jarvy update --channel beta
jarvy --version  # confirms vX.Y.Z-rc.N
jarvy drift check  # should be clean
```

Pass criteria:
- Upgrade succeeds via the install method that was used (per-platform table above)
- State file migrates cleanly; no schema-version errors
- `jarvy drift check` exits 0 (no drift) or correctly reports tool version
  differences if the rc bumps default tool versions
- `~/.jarvy/update-state.json` records `previous_version` for rollback

### Path 3 — Skip-Version Upgrade (minor and major bumps only)

State: clean VM with Jarvy at N-2 (two stable versions back) installed.

Skip this path for patch bumps — patch bumps from N-1 cover the same surface.

Pass criteria: same as Path 2 plus state file migrates from N-2 directly to rc
(skipping the N-1 schema if any).

### Path 4 — Rollback

State: VM that just upgraded from N-1 to rc in Path 2.

```bash
jarvy update --rollback
jarvy --version  # confirms previous stable
jarvy drift check  # should be clean against N-1's state
```

Pass criteria:
- Rollback restores the previous binary at the same install path
- `~/.jarvy/state.json` either restores the N-1 baseline or `jarvy drift accept`
  succeeds on the post-rollback state without error
- No orphaned files in `~/.jarvy/`

### Path 5 — Multi-Tool Real-World Setup

State: clean VM.

Run `jarvy setup` against a `jarvy.toml` with **at least 10 tools across 3+
package managers** plus a role with inheritance, an `[npm]`, `[pip]`, or
`[cargo]` section, and a default-hook tool (e.g. starship). This is the
realistic-load test.

Pass criteria:
- Setup completes within 1.5x the time the same config takes on the last
  stable (record the baseline once per release; regressions over 1.5x are
  treated as a sev-2 unless the diff intentionally added work)
- Parallel installs complete without lockfile or PATH corruption
- All hooks run; user-defined hooks override default hooks correctly
- Telemetry events fire when telemetry is enabled (verify with
  `jarvy telemetry preview`)

If any path fails: stop, classify the failure (sev-1 / sev-2 / sev-3 per the
[Severity Scale](#severity-scale)), comment on the soak issue, and either cut
`-rc.(N+1)` (restart from the matrix) or abandon the bump.

### Path 8 — Asset Download Sweep

State: any host with `gh`, `cosign`, `jq`, `curl`, `tar`, `ar`, and one of
`sha256sum` / `shasum -a 256`. Independent of any install path — fetches every
asset directly from the GitHub release manifest.

The point: install-path coverage (Paths 1–5) only exercises the assets the
install method happens to pull. A botched upload, a missing checksum entry,
or an unsigned artifact slipping through can sit invisible until a user
points an unusual install path at the release. This sweep is the
defense-in-depth check that the [Promotion Criteria](#promotion-criteria)
cosign / SBOM lines depend on — those criteria assume the assets were
fetched and verified, and this is the path that does it.

```bash
./dist/scripts/verify-release-assets.sh vX.Y.Z-rc.N
```

The script also runs unattended on every published release via
[`.github/workflows/verify-release.yml`](https://github.com/bearbinary/jarvy/blob/main/.github/workflows/verify-release.yml).
A green check run on the rc is the canonical signal; running locally is for
when CI is unavailable or to reproduce a failure.

Pass criteria:

- Every asset in the release manifest returns HTTP 200 on its
  `browser_download_url`
- `SHA256SUMS.txt` is present and lists a checksum for every `jarvy*` artifact
- Every recomputed sha256 matches the manifest entry
- Every `jarvy*` artifact has a matching `.sig`, `.pem`, and `.bundle`
- `cosign verify-blob --bundle <bundle> <file>` succeeds for every artifact,
  with `--certificate-identity-regexp` pinned to the
  `bearbinary/Jarvy/.github/workflows/release.yml@refs/tags/` subject and
  `--certificate-oidc-issuer` pinned to `token.actions.githubusercontent.com`
- SBOM artifacts (`sbom.spdx.json`, `sbom.cdx.json`) parse as valid JSON and
  carry the format-distinguishing key (`bomFormat: "CycloneDX"` or
  `spdxVersion: "SPDX-2.x"`)
- On a Linux host with `ar` and `tar`: the matching `.deb` extracts cleanly
  and `jarvy --version` reports a string containing the rc tag (without the
  leading `v`)

Hosts that cannot match a `.deb` (macOS, Windows, mismatched arch) skip the
binary `--version` probe with a warning — install-path coverage on those
platforms is exercised by Paths 1–5.

Failures are sev-1. Do not promote with any unchecked asset.

Skip this path only for documentation-only releases that the trigger matrix
already excluded from validation.

## Major-Only Matrix

If the bump is a **major** version, additionally run:

### Path 6 — Breaking-Config Negative Test

Take a `jarvy.toml` written against the last stable's schema and confirm Jarvy
either accepts it (with a deprecation warning) or fails with a clear error
message that names the breaking change and points at the migration doc. A
silent acceptance with broken behavior is sev-1.

### Path 7 — Documented Migration Replay

For any documented migration step (e.g. "users on `v0.x` need to run
`jarvy migrate`"), replay it from a **fresh snapshot of N-1 state**. Replaying
against a state that has already been touched by the rc defeats the point.

## Soak Window

After the pre-soak matrix passes, declare the soak window open by commenting
on the soak issue. Minimum durations:

| Bump kind | Minimum soak |
|---|---|
| Patch | 24 hours |
| Minor | 72 hours |
| Major | 7 days |

The window is a **minimum**, not a target. Do not promote early "because
everything looks fine" — state accumulation matters.

A `loop` skill invocation can pace observation: `/loop 6h /check soak signals
on issue #<number>`. Tune the interval to the soak duration: 6h checks for a
72h minor soak, 12h for a 7d major soak.

## Signals to Watch

Record every notable observation as a comment on the soak issue, even if it
doesn't trip a threshold — the cumulative record matters at promotion time.

| Signal source | What to watch for |
|---|---|
| `~/.jarvy/logs/jarvy.log` on cohort hosts | Any line at `ERROR` or `WARN` not present in the N-1 baseline |
| `jarvy ticket create` outputs | Any new error categories in `system_info` or `tool_info` |
| GitHub Issues opened against the rc tag | Any issue tagged `release-blocker` or `regression` blocks promotion |
| crates.io download spikes/dips | Unusual patterns may indicate a botched install path |
| Install-script completion rate | If the install scripts are instrumented, watch for a regression in success rate |
| `jarvy drift check` exit codes on cohort hosts | Unexpected drift suggests the rc changed tool defaults silently |
| Telemetry (when adoption justifies it) | Once the OTLP cohort is non-trivial, watch for spikes in `setup.failure` and `tool.install.failure` events |

**Telemetry note**: telemetry is opt-in and disabled by default. Until the
opt-in beta cohort and an OTLP collector are set up, this row is aspirational
— the soak relies on log inspection on the maintainer's hosts plus GitHub
issue traffic.

## Severity Scale

- **sev-1** — User data loss, broken install on a supported platform, security
  regression, panic on `jarvy setup` golden path. Blocks promotion. Must fix
  in `-rc.(N+1)` or abandon the bump.
- **sev-2** — Performance regression >1.5x, broken non-golden path, broken
  edge case, telemetry event missing. Either fix in `-rc.(N+1)` or document
  as a known issue in the stable CHANGELOG entry with the maintainer's
  explicit decision recorded on the soak issue.
- **sev-3** — Cosmetic, log noise, doc inaccuracy, recoverable warning.
  Does not block promotion; track as a follow-up issue.

## Promotion Criteria

When the minimum soak duration has elapsed, **all** must be true to promote:

- [ ] Paths 1–5 PASS on every required cohort platform
- [ ] Path 8 (asset download sweep) PASS — green check run on the rc tag
      from `.github/workflows/verify-release.yml`, OR a manual local run
      attached to the soak issue
- [ ] [Majors only] Paths 6 and 7 PASS
- [ ] [Majors only] All four fault-injection drills PASS
- [ ] No open `release-blocker` or `regression` issues against the rc
- [ ] No sev-1 surfaced during soak
- [ ] Any sev-2 surfaced during soak is either fixed in a later rc or
      documented as a known issue with explicit maintainer sign-off
- [ ] HEAD commit equals the rc commit, OR a new rc was cut to absorb any
      extra commits (do not promote a tag that doesn't match what was soaked)
- [ ] Tag signature verifies (`git tag -v vX.Y.Z-rc.N`)
- [ ] All Sigstore artifact signatures verify (`cosign verify-blob …`)
- [ ] SBOM artifacts present and well-formed

If any box is unchecked, do not promote. Get explicit go-ahead from the
maintainer before proceeding — promotion is per-version, not per-process.

## Major-Only Fault Injection

For a major bump, the soak window must include four drills. Schedule them
across the window — do not stack them on day 1. The point is to confirm the
new code recovers as well as N-1 across days of state accumulation, not just
from a fresh start.

1. **Kill mid-`jarvy setup`** — run setup against a config with 10+ tools,
   send SIGKILL halfway through. Re-run setup; expect resumption with no
   duplicate installs and a clean exit.
2. **Network drop during update** — start `jarvy update --channel beta`,
   drop the network mid-download (e.g. `sudo ifconfig en0 down`). Restore.
   Re-run; expect retry success without corrupting the binary on disk.
3. **Partial PATH write** — simulate a partial shell-rc write (truncate
   `~/.zshrc` mid-`jarvy setup` with a default hook that edits it).
   Confirm idempotency: a second run repairs without duplicate entries.
4. **Lock file corruption** — corrupt `~/.jarvy/state.json` mid-run.
   Confirm `jarvy drift check` reports a clear error and `jarvy drift accept`
   re-baselines without crashing.

Document each drill on the soak issue: timestamp, expected recovery,
observed recovery, any deviation.

## Promotion to Stable

When all promotion criteria are met:

```bash
RC_COMMIT=$(git log -1 --format=%H vX.Y.Z-rc.N)
HEAD_COMMIT=$(git log -1 --format=%H HEAD)
[ "$RC_COMMIT" = "$HEAD_COMMIT" ] || { echo "DRIFT"; exit 1; }
```

If the commits match, hand off to the `cutting-release` skill. The stable
CHANGELOG entry is fresh narrative — do **not** copy-paste from the rc
auto-notes. Call out any sev-2 known-issues that surfaced during soak.

Comment on the soak issue: `Promoted to vX.Y.Z at <timestamp>. Soak closed.`
Then close the issue.

## Post-Stable Channel Propagation

After the stable GitHub release is published and assets verified, propagate
to channels that don't auto-update from the GitHub release:

| Channel | Process | Auto / Manual |
|---|---|---|
| GitHub release | Triggered by tag push | Auto |
| crates.io | `cargo publish` (CI workflow `publish-packages.yml::publish-crates-io`) | Auto |
| Homebrew tap | CI updates `bearbinary/homebrew-tap` formula | Auto |
| `.deb`, `.rpm`, `.msi`, `.dmg`, `.AppImage` | Built and attached to the release | Auto |
| AUR (`jarvy-bin`) | Update `PKGBUILD-bin` checksums, `makepkg --printsrcinfo`, push to `ssh://aur@aur.archlinux.org/jarvy-bin.git` | Manual |
| AUR (`jarvy`, source) | Same as `jarvy-bin` for `PKGBUILD` | Manual |
| winget | `wingetcreate update Jarvy.Jarvy --version X.Y.Z --urls <…> --submit` (PR to microsoft/winget-pkgs) | Manual; Microsoft approves |
| Chocolatey | Update `jarvy.nuspec` + `chocolateyinstall.ps1`, `choco pack`, `choco push --api-key $CHOCOLATEY_API_KEY` | Manual; first submission moderated |

Watch the soak issue for **48 hours** after channel propagation completes —
auto-updating users via Homebrew and winget are the post-promotion canary.
Any breakage report in that window is sev-1 and triggers the rollback path.

For full credentials and command details, see
[`docs/MAINTAINER_RELEASE_GUIDE.md`](MAINTAINER_RELEASE_GUIDE.md).

## Rollback Path

If a sev-1 surfaces after the stable tag has shipped:

1. **Yank crates.io**: `cargo yank --version X.Y.Z` (does not delete; prevents
   new dependents resolving to it).
2. **Mark GitHub release**: edit the release body to prepend a withdrawal
   banner with the issue link and the recommended downgrade version. Do not
   delete the release — the assets are immutable and other channels point
   at them.
3. **Revert Homebrew formula**: push the previous formula version to
   `bearbinary/homebrew-tap`.
4. **Cut a fix**: bump patch (`vX.Y.(Z+1)`), restart from the rc soak.
5. **Notify channels with manual propagation**: the catalog/marketplace
   channels (AUR, winget, Choco) need explicit notification — they will not
   auto-rollback.
6. **Post-mortem**: create a `release-postmortem` issue documenting what
   broke, what soak missed, and what trigger-matrix row should have caught it.

## Anti-Patterns to Refuse

- "Skip the soak, it's a tiny change" — if the trigger matrix matched, the
  soak is required. The trigger matrix is the contract.
- "Promote early, the cohort env looks fine" — the soak window is a fixed
  minimum, not a target.
- "Just edit the GitHub release notes if something breaks" — assets are
  immutable. A withdrawal banner is the only in-band signal; the rollback
  path is what actually protects users.
- "Bundle the AUR/winget bump with the stable cut" — manual channels propagate
  only after the stable release has been on GitHub long enough for the
  auto-update channels to surface any immediate breakage.
- "We don't have a beta cohort yet, skip the channel work" — the opt-in
  channel is what makes the cohort exist. Without it, the only beta tester
  is the maintainer.
