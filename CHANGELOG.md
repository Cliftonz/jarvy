# Changelog

All notable changes to Jarvy will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Policy

- **Stable releases (`vX.Y.Z`)** get a curated entry below **before the tag is
  pushed**. The release workflow's `Build release notes` step awk-extracts the
  matching `## [vX.Y.Z]` section into the GitHub release body, then appends a
  `**Full Changelog**` compare link plus Jarvy's standing install/security
  footer. Forgetting this entry causes the workflow to fall through to a raw
  `git log` listing — technically valid, but reads like a commit dump rather
  than a curated narrative. Update CHANGELOG before tagging.
- **Pre-releases (`vX.Y.Z-rc.N`, `-beta.N`, `-alpha.N`)** do **not** get a
  CHANGELOG entry. The awk extraction returns empty, the workflow falls
  through to `git log <prev-tag>..<tag>` notes, and that fallback is the
  intended pre-release path. The curated stable entry below is written once
  when the corresponding stable cuts.
- Entry headers must match the awk pattern: `## [vX.Y.Z]` or
  `## [vX.Y.Z] — Title` (em-dash optional). Other shapes won't be matched.

See [`docs/release-testing.md`](docs/release-testing.md) for the full release
process and [`docs/release-quirks-jarvy.md`](docs/release-quirks-jarvy.md) for
divergences from generic release skills.

## [Unreleased]

Initial public release. Everything below ships in `v0.1.0`.

### Features

- **provisioner:** Cross-platform tool provisioner driven by `jarvy.toml`
  (macOS, Linux, Windows) with native package managers
- **tools:** 154+ tool registry covering compilers, runtimes, CLIs, container
  tools, Kubernetes ecosystem (kubectl, helm, k9s, kagent, kmcp, arctl), cloud
  CLIs (gcloud, aws, az), security tools, observability (opentelemetry-collector),
  Dockerfile converter (dfc) (PRD-013)
- **tools:** Parallel version checking with rayon for ~5x speedup; batch
  package-manager operations
- **tools:** Declarative `define_tool!` macro for tool definitions (~2000 lines
  reduced)
- **tools:** Strict (`depends_on`) and flexible (`depends_on_one_of`) tool
  dependencies with topological install ordering (PRD-034)
- **hooks:** 29+ default post-install hooks for shell completion and
  configuration; idempotent, advisory, user-overridable
- **roles:** Role-based configurations with deep inheritance, version overrides,
  `roles list|show|diff` commands (PRD-033)
- **packages:** Language package deps via `[npm]`, `[pip]`, `[cargo]` —
  package-manager auto-detection, virtualenv support, lockfile install (PRD-039)
- **git:** Git configuration automation — identity, SSH/GPG signing, default
  branch, aliases, credential helper auto-detect per OS (PRD-041)
- **drift:** Configuration drift detection with SHA-256 file hashing, version
  policies, `jarvy drift check|status|accept|fix` (PRD-043)
- **update:** Self-updating with stable/beta/nightly channel selection,
  throttled checks, rollback, multi-method install detection (Homebrew, Cargo,
  apt, dnf, winget, Chocolatey, Scoop, binary fallback) (PRD-035)
- **telemetry:** OTEL-unified logs, metrics, optional traces; OTLP HTTP/gRPC
  endpoints; CI auto-disable; `jarvy telemetry status|enable|disable|test|preview`
  (PRD-022, PRD-050)
- **logging:** Persistent file logging with rotation, gzip compression,
  sensitive-data redaction; `jarvy logs view|stats|clean|config` (PRD-050)
- **ticket:** Debug bundles via `jarvy ticket create|show|list|clean` — ZIP with
  system info, tool versions, sanitized logs (PRD-050)
- **network:** Corporate proxy support — HTTP/HTTPS/SOCKS, NO_PROXY, custom CA
  bundles, per-tool overrides, secure password sources (PRD-019)
- **services:** Docker Compose and Tilt backend support
- **ci:** Auto-detection for 11 CI/CD providers with provider-specific output
- **env:** Environment variable management with `.env` generation and shell rc
  updates
- **mcp:** MCP server exposing tools and resources for AI assistants
- **interactive:** Menu mode when running `jarvy` without a subcommand
- **bootstrap:** `jarvy bootstrap`, `jarvy configure`, `jarvy diagnose` for
  onboarding (PRD-023)

### Distribution

- Multi-channel: crates.io, Homebrew tap, AUR (source + binary), `.deb`, `.rpm`,
  winget, Chocolatey, universal install scripts for macOS/Linux/Windows (PRD-012)
- Sigstore keyless signing for all release artifacts (PRD-020)
- SBOM generation in SPDX 2.3 and CycloneDX 1.4 formats per release (PRD-020)
- GitHub build provenance attestation per release (PRD-020)
- Opt-in early-release channel: `JARVY_CHANNEL=beta` env var on install
  scripts; `[update] channel = "beta"` in `~/.jarvy/config.toml`;
  `jarvy update --channel beta`

### Quality & Security

- Clippy gate, mutation testing, fuzzing, coverage, benchmarks, OpenSSF
  Scorecard (PRD-018)
- Hybrid cross-platform E2E testing harness (PRD-038)
- Tag-signing enforcement (SSH or GPG) on release workflow
- Cosign keyless signing via GitHub OIDC for all release artifacts

### Infrastructure

- Semantic version checking with proper semver operators
- Cross-platform shell detection and hook execution
- Workspace lint configuration; Rust 2024 edition; MSRV 1.85

[Unreleased]: https://github.com/bearbinary/jarvy/compare/HEAD
