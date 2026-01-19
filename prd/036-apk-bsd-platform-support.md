# PRD-036: APK (Alpine Linux) and BSD (FreeBSD) Platform Support

## Status: Complete

## Summary

Add comprehensive package definitions for Alpine Linux (apk) and FreeBSD (pkg) across all Jarvy tools. The framework infrastructure is complete and all tools have been updated.

## Current State Analysis

### Framework Readiness (100% Complete)

The Jarvy codebase has full infrastructure support:

- **`BsdInstall` struct** - `src/tools/spec.rs:176-188` - Fully implemented with `pkg()` helper
- **`detect_bsd_pm()`** - `src/tools/common.rs:239-254` - Detects FreeBSD's pkg manager
- **`install_bsd()`** - `src/tools/spec.rs:443-459` - Complete installation logic
- **`LinuxInstall.apk`** - `src/tools/spec.rs:91` - APK field in Linux struct
- **`uniform()` includes apk** - `src/tools/spec.rs:103` - Uniform packages auto-include apk
- **NEW: `brew + apk` macro pattern** - `src/tools/spec.rs:560-571` - For tools with Alpine but not other distro packages

### Coverage Summary (157 Total Tools)

| Category | Count | APK Status | BSD Status |
|----------|-------|------------|------------|
| Uniform Linux packages | 70 | Auto-included | Added |
| Explicit apt/dnf/pacman/apk | 41 | Defined | Added |
| Linuxbrew + APK | 10 | Added | Added |
| Linuxbrew-only on Linux | 29 | N/A (no Alpine pkg) | Added |
| GUI/Desktop apps | 7 | N/A | N/A |
| Custom install (nvm, rust, brew) | 3 | N/A | N/A |

### Package Naming Patterns

| Platform | Python Tools | Standard Tools |
|----------|-------------|----------------|
| Alpine (apk) | `py3-{name}` | Same as other distros |
| FreeBSD (pkg) | `py39-{name}` | Same as other distros |

## Tools With APK Support Added (This PR)

These 10 tools had Alpine packages found and were updated with the new `brew + apk` pattern:

| Tool | Alpine Package | Source |
|------|----------------|--------|
| argocd | `argocd` | edge/testing |
| cosign | `cosign` | edge/community |
| dive | `dive` | edge/testing |
| grype | `grype` | edge/community |
| krew | `kubectl-krew` | edge/testing |
| lazydocker | `lazydocker` | v3.21/community |
| nerdctl | `nerdctl` | edge/community |
| ruff | `ruff` | v3.21/community |
| syft | `syft` | edge/community |
| trivy | `trivy` | edge/testing |

## Tools Without BSD Support (Intentionally N/A)

These tools cannot have BSD support due to their nature:

### GUI/Desktop Applications
- `cursor` - macOS/Windows GUI editor
- `docker_desktop` - Docker Desktop GUI
- `freelens` - Kubernetes IDE (Electron)
- `iterm2` - macOS terminal emulator
- `jetbrains_toolbox` - JetBrains IDEs installer
- `podman_desktop` - Podman Desktop GUI
- `rancher_desktop` - Rancher Desktop GUI
- `vscode` - Visual Studio Code (Electron)
- `zed` - macOS/Linux GUI editor

### Custom Install Tools
- `brew` - Homebrew (macOS/Linux installer script)
- `nvm` - Node Version Manager (shell script)
- `rust` - Rustup installer (shell script)

## Tools Using Linuxbrew Only (No Alpine Package)

These 29 tools use Homebrew on Linux and do not have packages in Alpine repos:

```
act, actionlint, bun, checkov, dbmate, deno, flux, gitleaks, hadolint,
k3d, kind, kubectx, kubens, mise, mongosh, pyenv, rbenv, sd, semgrep,
sops, terraform_docs, terragrunt, trufflehog, up, usql, vfox
```

**Research Source**: https://pkgs.alpinelinux.org/packages (searched January 2026)

## Implementation Checklist

### Phase 1: Infrastructure (Complete)
- [x] BsdInstall struct in spec.rs
- [x] detect_bsd_pm() in common.rs
- [x] install_bsd() method in spec.rs
- [x] LinuxInstall.apk field
- [x] uniform() includes apk
- [x] Update _template.rs to include BSD placeholder
- [x] Add `brew + apk` macro pattern for hybrid support

### Phase 2: BSD Support (Complete)
- [x] Add `bsd: { pkg: "..." }` to 145 tools
- [x] Document N/A tools (GUI apps, custom installs)

### Phase 3: APK Research (Complete)
- [x] Research Alpine packages for Linuxbrew-only tools
- [x] Add explicit apk for 10 tools that exist in Alpine repos
- [x] Document unavailable tools (29 tools)

## Testing Strategy

1. **Compile check**: `cargo check --all-features` ✅
2. **Format**: `cargo fmt --all` ✅
3. **Alpine Docker**: Build in `alpine:edge` container (future)
4. **FreeBSD CI**: Cirrus CI with FreeBSD image (future)

## Success Criteria

1. ✅ All tools have either BSD package defined OR documented as N/A
2. ✅ All tools using `uniform:` automatically have APK support
3. ✅ _template.rs includes BSD placeholder for new tools
4. ✅ Code compiles successfully
5. ✅ New `brew + apk` macro pattern for hybrid Linux support

## References

- Alpine packages: https://pkgs.alpinelinux.org/packages
- FreeBSD ports: https://www.freshports.org/
- spec.rs LinuxInstall: lines 82-147
- spec.rs BsdInstall: lines 176-188
- spec.rs brew+apk pattern: lines 560-571
