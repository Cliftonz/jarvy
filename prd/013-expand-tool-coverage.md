# PRD-013: Expand Tool Coverage

## Overview

Expand Jarvy's tool registry from the current 102 tools to 150+ tools, establishing comprehensive coverage across all major developer tool categories.

## Problem Statement

While Jarvy supports 102 tools, many popular and essential developer tools are missing:

1. **Modern CLI alternatives**: atuin, helix, nushell, yazi, mise, watchexec
2. **Database tools**: dbeaver-cli, pgcli, mycli, litecli, usql
3. **Container ecosystem**: buildah, skopeo, cri-o, containerd, nerdctl, lazydocker
4. **Language tooling**: deno, bun, zig, gleam tools, elixir tools
5. **DevOps utilities**: ansible, packer-specific plugins, vagrant
6. **Security tools**: cosign, grype, syft, gitleaks

This gap prevents teams from fully automating their development environment setup, forcing manual installation of commonly-used tools.

## Evidence

- Community requests for missing tools (GitHub issues, discussions)
- Competitors (asdf, mise, devbox) support broader tool catalogs
- Developer surveys show high adoption of modern CLI tools (e.g., atuin 20k+ stars, helix 35k+ stars)

## Goals

1. **Quantity**: Expand from 102 to 150+ tools
2. **Quality**: Every tool uses `define_tool!` macro with cross-platform support where possible
3. **Coverage**: Fill gaps in all major categories
4. **Community**: Establish process for community tool requests

## Non-Goals

1. **GUI applications** (except developer IDEs like VS Code, JetBrains, Cursor)
2. **Games** or entertainment software
3. **System utilities** not development-related
4. **Deprecated or unmaintained tools**
5. **Tools without clear package manager availability**

## User Stories

### US-001: CLI Utilities Expansion

**As a developer, I want modern CLI utilities available in Jarvy so that I can use the latest productivity tools.**

**Priority**: P0 | **Impact**: High

**Tools to Add**:
| Tool | Description | Stars | Priority |
|------|-------------|-------|----------|
| atuin | Shell history sync with SQLite | 22k+ | P0 |
| helix | Modal text editor (Kakoune-inspired) | 35k+ | P0 |
| nushell | Modern shell with structured data | 33k+ | P0 |
| yazi | Terminal file manager | 18k+ | P0 |
| mise | Polyglot runtime manager (asdf successor) | 12k+ | P0 |
| watchexec | File watcher for command execution | 5k+ | P1 |
| dust | Disk usage analyzer (du replacement) | 9k+ | P1 |
| tokei | Code statistics tool | 11k+ | P1 |
| hyperfine | Benchmarking tool | 23k+ | P1 |
| sd | sed replacement | 6k+ | P1 |
| choose | cut replacement | 2k+ | P2 |
| grex | Regex generator | 7k+ | P2 |
| broot | Tree navigation/search | 11k+ | P1 |
| lsd | ls replacement | 14k+ | P1 |
| dog | DNS client (dig replacement) | 6k+ | P2 |
| gping | Ping with graph | 11k+ | P2 |

**Acceptance Criteria**:
- [ ] All P0 tools implemented with `define_tool!` macro
- [ ] All P1 tools implemented with `define_tool!` macro
- [ ] Cross-platform support (macOS, Linux, Windows where available)
- [ ] Each tool has integration test
- [ ] Default hooks added where beneficial (e.g., atuin shell init)

### US-002: Language Runtimes & Tooling

**As a polyglot developer, I want all major language runtimes and their tooling available so that I can provision any development environment.**

**Priority**: P0 | **Impact**: High

**Tools to Add**:
| Tool | Description | Priority |
|------|-------------|----------|
| deno | JavaScript/TypeScript runtime | P0 |
| bun | Fast JavaScript runtime | P0 |
| zig | Systems programming language | P1 |
| kotlin | JVM language compiler | P1 |
| scala | JVM language with sbt | P1 |
| julia | Scientific computing | P2 |
| nim | Systems language | P2 |
| crystal | Ruby-like compiled lang | P2 |
| ocaml | Functional language | P2 |
| haskell (ghcup) | Haskell toolchain | P2 |
| lua | Scripting language | P1 |
| luarocks | Lua package manager | P2 |
| pyenv | Python version manager | P1 |
| rbenv | Ruby version manager | P1 |
| sdkman | JVM version manager | P2 |

**Acceptance Criteria**:
- [ ] All P0 language runtimes implemented
- [ ] Version managers have default hooks for shell init
- [ ] Each tool tested on at least macOS and Linux

### US-003: DevOps & Infrastructure Tools

**As a DevOps engineer, I want comprehensive infrastructure tooling so that I can provision complete CI/CD and infrastructure management environments.**

**Priority**: P0 | **Impact**: High

**Tools to Add**:
| Tool | Description | Priority |
|------|-------------|----------|
| ansible | Configuration management | P0 |
| vagrant | VM provisioning | P1 |
| molecule | Ansible testing | P2 |
| terraform-docs | Terraform documentation | P1 |
| terragrunt | Terraform wrapper | P1 |
| checkov | IaC security scanner | P1 |
| infracost | Cloud cost estimation | P2 |
| localstack | AWS local emulator | P2 |
| act | GitHub Actions locally | P1 |
| dagger | CI/CD pipelines as code | P2 |
| earthly | Build automation | P2 |
| velero | Kubernetes backup | P2 |
| istioctl | Istio service mesh CLI | P2 |
| linkerd | Linkerd CLI | P2 |
| cilium | Cilium CLI | P2 |

**Acceptance Criteria**:
- [ ] All P0 DevOps tools implemented
- [ ] Tools with complex install (ansible via pip) have custom_install
- [ ] Default hooks for PATH setup where needed

### US-004: Database Tools & Clients

**As a developer, I want database clients and tools available so that I can work with various databases from the command line.**

**Priority**: P1 | **Impact**: Medium

**Tools to Add**:
| Tool | Description | Priority |
|------|-------------|----------|
| pgcli | PostgreSQL CLI with autocomplete | P1 |
| mycli | MySQL CLI with autocomplete | P1 |
| litecli | SQLite CLI with autocomplete | P1 |
| usql | Universal SQL CLI | P1 |
| dbmate | Database migrations | P1 |
| sqlc | SQL compiler for Go | P2 |
| prisma | Node.js ORM CLI | P2 |
| diesel_cli | Rust ORM CLI | P2 |
| clickhouse-client | ClickHouse CLI | P2 |
| cassandra (cqlsh) | Cassandra CLI | P2 |
| duckdb | Analytical database CLI | P1 |

**Acceptance Criteria**:
- [ ] All P1 database tools implemented
- [ ] pip-installed tools (pgcli, mycli) use custom_install
- [ ] Connection testing examples in documentation

### US-005: Container & Kubernetes Ecosystem

**As a platform engineer, I want comprehensive container tooling so that I can work with OCI containers beyond just Docker.**

**Priority**: P1 | **Impact**: High

**Tools to Add**:
| Tool | Description | Priority |
|------|-------------|----------|
| buildah | OCI image builder | P0 |
| skopeo | Container image operations | P0 |
| nerdctl | containerd CLI | P1 |
| lazydocker | Docker TUI | P0 |
| dive | Docker image explorer | P1 |
| crane | Container registry tool | P2 |
| ko | Go container builder | P2 |
| kaniko | In-cluster builds | P2 |
| krew | kubectl plugin manager | P1 |
| kubens | Namespace switcher | P1 |
| popeye | Kubernetes cluster scanner | P2 |
| k3d | k3s in Docker | P1 |
| vcluster | Virtual clusters | P2 |

**Acceptance Criteria**:
- [ ] All P0 container tools implemented
- [ ] Tools work alongside existing Docker/Podman tools
- [ ] krew has default hook for PATH setup

### US-006: Security & Compliance Tools

**As a security engineer, I want security scanning tools so that I can audit code and infrastructure.**

**Priority**: P1 | **Impact**: Medium

**Tools to Add**:
| Tool | Description | Priority |
|------|-------------|----------|
| cosign | Container signing | P0 |
| grype | Vulnerability scanner | P0 |
| syft | SBOM generator | P0 |
| gitleaks | Secret detection | P0 |
| trufflehog | Secret scanner | P1 |
| semgrep | Static analysis | P1 |
| snyk | Security scanner CLI | P1 |
| scorecard | OSS security scores | P2 |
| nuclei | Vulnerability scanner | P2 |
| nikto | Web server scanner | P2 |
| sqlmap | SQL injection tool | P2 |
| nmap (already exists) | Network scanner | Done |

**Acceptance Criteria**:
- [ ] All P0 security tools implemented
- [ ] Tools requiring API keys have documentation notes
- [ ] Integration with CI detection for optimal defaults

### US-007: Editor & IDE Support

**As a developer, I want editor/IDE support beyond VS Code so that I can use my preferred development environment.**

**Priority**: P1 | **Impact**: Medium

**Tools to Add**:
| Tool | Description | Priority |
|------|-------------|----------|
| cursor | AI-powered VS Code fork | P0 |
| zed | High-performance editor | P0 |
| jetbrains-toolbox | JetBrains manager | P1 |
| lapce | Rust-based editor | P2 |
| emacs | GNU Emacs | P1 |
| vim | Classic vim | P1 |
| micro | Terminal editor | P2 |
| sublime-text | Sublime Text | P2 |
| sublime-merge | Git GUI | P2 |

**Note**: helix and nvim already exist; VS Code already exists.

**Acceptance Criteria**:
- [ ] All P0 editors implemented
- [ ] GUI apps use cask on macOS where appropriate
- [ ] CLI tools use standard package managers

## Implementation Process

### Adding a New Tool

1. **Create tool directory**: `cargo run -p cargo-jarvy -- new-tool <name>`
2. **Implement with `define_tool!`** macro in `src/tools/<name>/<name>.rs`
3. **Register in `src/tools/mod.rs`** via `register_all()`
4. **Add integration test** if tool has unique install behavior
5. **Add default hook** if tool requires shell configuration
6. **Test on all platforms** where the tool is available
7. **Update documentation** if tool has special requirements

### Tool Acceptance Criteria

Every tool MUST have:
- [ ] Uses `define_tool!` macro (no manual implementations)
- [ ] Package manager mappings for available platforms
- [ ] Command name for verification
- [ ] Integration test (can be shared with category)

Every tool SHOULD have:
- [ ] Cross-platform support (macOS + Linux minimum)
- [ ] Windows support where package exists
- [ ] Default hook if shell configuration needed
- [ ] Version command for verification

### Community Tool Requests

**Process for requesting new tools**:

1. **Open GitHub Issue** with template:
   - Tool name and homepage
   - Package availability (brew, apt, winget, etc.)
   - Use case and why it should be included
   - Priority suggestion (P0/P1/P2)

2. **Triage by maintainers**:
   - Verify package availability
   - Check popularity/maintenance status
   - Assign priority and milestone

3. **Implementation**:
   - Community PRs welcome with `define_tool!` usage
   - Maintainer review for quality and consistency
   - CI must pass on all platforms

4. **Rejection criteria**:
   - Unmaintained (no commits in 2+ years)
   - No package manager availability
   - GUI-only without CLI component
   - Duplicate functionality of existing tool

## Implementation Phases

### Phase 1: High-Impact CLI Tools (Week 1-2)
**Target: +15 tools**

| Tool | Category | Platform Support |
|------|----------|------------------|
| atuin | CLI | macOS, Linux, Windows |
| helix | Editor | macOS, Linux, Windows |
| nushell | Shell | macOS, Linux, Windows |
| yazi | CLI | macOS, Linux |
| mise | Version Mgr | macOS, Linux |
| deno | Runtime | macOS, Linux, Windows |
| bun | Runtime | macOS, Linux, Windows |
| lazydocker | Container | macOS, Linux, Windows |
| gitleaks | Security | macOS, Linux, Windows |
| cosign | Security | macOS, Linux, Windows |
| grype | Security | macOS, Linux, Windows |
| syft | Security | macOS, Linux, Windows |
| ansible | DevOps | macOS, Linux |
| buildah | Container | Linux |
| skopeo | Container | Linux |

### Phase 2: Language & DevOps Tools (Week 3-4)
**Target: +20 tools**

Focus: Language tooling, DevOps utilities, database clients

### Phase 3: Remaining P1 Tools (Week 5-6)
**Target: +15 tools**

Focus: Editor support, remaining container tools, infrastructure tools

### Phase 4: P2 Tools & Polish (Ongoing)
**Target: Reach 150+**

Focus: Lower-priority tools, Windows support improvements, documentation

## Success Metrics

| Metric | Current | Target |
|--------|---------|--------|
| Total tools | 102 | 150+ |
| Categories covered | 10 | 12+ |
| Cross-platform (all 3) | ~60% | 70%+ |
| Tools with default hooks | 15 | 30+ |
| Community contributions | - | 10+ tools |

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Package availability varies | High | Medium | Document platform limitations |
| Maintenance burden increases | Medium | High | Enforce `define_tool!` macro usage |
| Tool deprecation | Low | Medium | Monitor upstream projects |
| Windows package gaps | High | Medium | Prioritize cross-platform tools |

## Dependencies

- Existing `define_tool!` macro infrastructure
- Package manager detection in `src/tools/common.rs`
- Default hook support from PRD-003

## Effort Estimate

| Phase | Effort | Tools Added |
|-------|--------|-------------|
| Phase 1 | 1 week | 15 tools |
| Phase 2 | 1.5 weeks | 20 tools |
| Phase 3 | 1 week | 15 tools |
| Phase 4 | Ongoing | 2-3/week |

**Total to reach 150**: ~5 weeks

## Files to Modify

- `src/tools/mod.rs` - Register new tools
- `src/tools/<tool>/mod.rs` - New tool directories
- `src/tools/<tool>/<tool>.rs` - Tool implementations
- `tests/` - Integration tests for new tools
- `docs/` - Tool documentation updates

## Appendix: Complete Tool Wishlist

### Already Implemented (102 tools)
actionlint, age, air, argocd, aria2, atlas, awscli, azure_cli, bat, bottom, brew, btop, cue, curl, delta, direnv, docker, docker_desktop, dotnet, duf, eksctl, elixir, eza, fd, flux, fzf, gh, git, git_lfs, glab, gleam, go, hadolint, helm, htop, httpie, hugo, iterm2, jq, just, k6, k9s, kind, kubectl, kubectx, kubescape, kustomize, lazygit, lnav, lynis, make, minikube, mongosh, mtr, mysql, ncdu, ngrok, nmap, node, nvim, nvm, openssh, opentofu, p7zip, packer, php, podman, podman_desktop, powershell, procs, psql, pulumi, python, rancher_desktop, rclone, redis, ripgrep, ruby, ruff, rust, shellcheck, shfmt, sops, sqlite, starship, stern, talosctl, terraform, tfsec, tilt, tmux, tree, trivy, up, vault, vscode, wget, xz, yamllint, yq, zoxide, zsh

### Priority Queue (Recommended Order)
1. atuin, helix, nushell, yazi, mise (P0 CLI)
2. deno, bun (P0 Runtimes)
3. lazydocker, buildah, skopeo (P0 Containers)
4. gitleaks, cosign, grype, syft (P0 Security)
5. ansible (P0 DevOps)
6. cursor, zed (P0 Editors)
7. ... (continue with P1, then P2)
