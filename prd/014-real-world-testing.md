# PRD-014: Real-World Testing and Example Configurations

## Overview

Establish comprehensive real-world testing across all supported platforms and create example jarvy.toml configurations for common developer personas, ensuring Jarvy works reliably in production environments.

## Problem Statement

While Jarvy has unit and integration tests, real-world validation is lacking:

1. **Platform coverage gaps**: No systematic testing on all target platforms (macOS Intel/ARM, Ubuntu, Fedora, Arch, Windows)
2. **Missing personas**: No example configs for common developer workflows
3. **CI limitations**: Tests don't verify actual tool installations with caching
4. **No smoke tests**: Critical paths aren't regularly validated end-to-end

## Evidence

- User reports of platform-specific issues are discovered post-release
- New contributors struggle without example configurations
- CI only runs on ubuntu-latest, missing platform-specific bugs
- No automated verification that core workflows actually work

## User Stories

### Platform Testing Matrix

**As a Jarvy maintainer**, I want automated testing on all supported platforms so that platform-specific bugs are caught before release.

Acceptance Criteria:
- CI matrix covers macOS Intel (x86_64) and ARM (aarch64)
- CI matrix covers Ubuntu LTS (22.04, 24.04)
- CI matrix covers Fedora (latest)
- CI matrix covers Arch Linux (latest)
- CI matrix covers Windows 11 with winget
- Each platform runs smoke tests for critical paths
- Platform-specific tools are tested on their native platforms

### Example Configurations by Persona

**As a new Jarvy user**, I want example configurations for my role so that I can quickly bootstrap my development environment.

Acceptance Criteria:
- Frontend Developer config (Node.js, pnpm, TypeScript tooling)
- Backend Developer config (Go, Rust, Docker, databases)
- DevOps Engineer config (Terraform, kubectl, AWS CLI, Docker)
- Data Scientist config (Python, conda, Jupyter, R)
- Mobile Developer config (Flutter, Android SDK, Xcode tools)
- Each example includes inline comments explaining choices
- Examples demonstrate both simple and detailed tool syntax

### CI Testing with Cached Installations

**As a CI pipeline**, I want tool installations to be cached so that tests run quickly while still verifying real installations.

Acceptance Criteria:
- Tool installation results are cached per platform
- Cache is invalidated when tool versions change
- First-run installs tools, subsequent runs use cache
- Cache key includes jarvy.toml hash
- Fallback to fresh install on cache miss

### Smoke Tests for Critical Paths

**As a Jarvy maintainer**, I want automated smoke tests so that critical user workflows are verified on every release.

Acceptance Criteria:
- `jarvy setup` with minimal config completes successfully
- `jarvy setup --dry-run` shows correct output
- `jarvy bootstrap` provisions Homebrew/apt/winget correctly
- `jarvy get <tool>` reports version information
- `jarvy tools` lists available tools
- Hook execution completes without errors
- Service management commands work (start/stop/status)

## Non-Goals

- Testing every possible tool combination (combinatorial explosion)
- Performance benchmarking (separate PRD)
- Security scanning of installed tools (out of scope)
- Testing deprecated/legacy package managers

## Requirements

### Functional Requirements

1. **Platform CI Matrix**: GitHub Actions workflow with comprehensive platform coverage
2. **Example Configs**: `examples/` directory with persona-based configurations
3. **Smoke Test Suite**: Critical path tests that run on all platforms
4. **Caching Strategy**: Efficient CI caching for tool installations

### Non-Functional Requirements

1. Full CI suite completes in < 30 minutes
2. Smoke tests complete in < 5 minutes per platform
3. Examples are validated by CI (parseable, tools exist)
4. Documentation updated with example usage

## Example Directory Structure

```
examples/
├── README.md                    # Overview and usage instructions
├── minimal.toml                 # Bare minimum config
├── frontend-developer.toml      # Web frontend stack
├── backend-developer.toml       # Backend/API development
├── devops-engineer.toml         # Infrastructure/ops tooling
├── data-scientist.toml          # Data science stack
├── mobile-developer.toml        # iOS/Android development
├── full-stack.toml              # Combined frontend + backend
└── rust-developer.toml          # Rust ecosystem tools
```

### Example: Frontend Developer

```toml
# examples/frontend-developer.toml
# Jarvy configuration for frontend/web developers
# Run: jarvy setup

[tools]
# JavaScript runtime and package manager
node = "20"
pnpm = "latest"

# Build and bundling
esbuild = "latest"

# Code quality
prettier = { version = "latest", version_manager = true }
eslint = "latest"

# Browser testing
playwright = "latest"

# Development utilities
jq = "latest"
httpie = "latest"

[hooks.node]
description = "Configure pnpm as default package manager"
script = """
corepack enable
corepack prepare pnpm@latest --activate
"""
```

### Example: DevOps Engineer

```toml
# examples/devops-engineer.toml
# Jarvy configuration for DevOps/Platform engineers
# Run: jarvy setup

[tools]
# Container orchestration
docker = "latest"
kubectl = "latest"
helm = "latest"
k9s = "latest"

# Infrastructure as Code
terraform = "1.7"
pulumi = "latest"

# Cloud CLIs
awscli = "2"
gcloud = "latest"
azure-cli = "latest"

# Git and version control
git = "latest"
gh = "latest"

# Security scanning
trivy = "latest"

# Observability
stern = "latest"

[services]
enabled = true
auto_start = false

[services.local-registry]
image = "registry:2"
port = 5000
```

## CI Configuration

### Platform Matrix

```yaml
# .github/workflows/real-world-tests.yml
name: Real-World Testing

on:
  push:
    branches: [main]
  pull_request:
  schedule:
    - cron: '0 6 * * 1'  # Weekly Monday 6 AM UTC

jobs:
  smoke-tests:
    strategy:
      fail-fast: false
      matrix:
        include:
          # macOS
          - os: macos-13        # Intel
            name: macos-intel
          - os: macos-14        # ARM
            name: macos-arm
          # Linux
          - os: ubuntu-22.04
            name: ubuntu-22.04
          - os: ubuntu-24.04
            name: ubuntu-24.04
          # Windows
          - os: windows-latest
            name: windows-11

    runs-on: ${{ matrix.os }}
    name: Smoke Tests (${{ matrix.name }})

    steps:
      - uses: actions/checkout@v4

      - name: Cache Rust toolchain
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/bin/
            ~/.cargo/registry/
            ~/.cargo/git/
            target/
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}

      - name: Build Jarvy
        run: cargo build --release

      - name: Run smoke tests
        run: cargo test --test smoke_tests -- --show-output
        env:
          JARVY_TEST_MODE: 1

  linux-distros:
    strategy:
      fail-fast: false
      matrix:
        distro:
          - fedora:latest
          - archlinux:latest

    runs-on: ubuntu-latest
    container: ${{ matrix.distro }}
    name: Linux (${{ matrix.distro }})

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        run: |
          curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
          echo "$HOME/.cargo/bin" >> $GITHUB_PATH

      - name: Build and test
        run: |
          source $HOME/.cargo/env
          cargo build --release
          cargo test --test smoke_tests -- --show-output
        env:
          JARVY_TEST_MODE: 1

  validate-examples:
    runs-on: ubuntu-latest
    name: Validate Example Configs

    steps:
      - uses: actions/checkout@v4

      - uses: dtolnay/rust-toolchain@stable

      - name: Build Jarvy
        run: cargo build --release

      - name: Validate all example configs
        run: |
          for config in examples/*.toml; do
            echo "Validating $config..."
            ./target/release/jarvy setup --dry-run --config "$config"
          done
        env:
          JARVY_TEST_MODE: 1

  cached-installation:
    runs-on: ubuntu-latest
    name: Cached Tool Installation

    steps:
      - uses: actions/checkout@v4

      - name: Cache installed tools
        uses: actions/cache@v4
        with:
          path: |
            ~/.local/bin
            /usr/local/bin
            /opt/homebrew/bin
          key: tools-${{ runner.os }}-${{ hashFiles('examples/minimal.toml') }}

      - uses: dtolnay/rust-toolchain@stable

      - name: Build Jarvy
        run: cargo build --release

      - name: Install tools (uses cache if available)
        run: ./target/release/jarvy setup --config examples/minimal.toml
        env:
          JARVY_TEST_MODE: 1
```

## Smoke Test Implementation

```rust
// tests/smoke_tests.rs
use assert_cmd::Command;
use predicates::prelude::*;

/// Critical path: jarvy setup with minimal config
#[test]
fn smoke_setup_minimal() {
    let mut cmd = Command::cargo_bin("jarvy").unwrap();
    cmd.arg("setup")
       .arg("--dry-run")
       .arg("--config")
       .arg("examples/minimal.toml")
       .env("JARVY_TEST_MODE", "1")
       .assert()
       .success()
       .stdout(predicate::str::contains("Would install"));
}

/// Critical path: jarvy tools command
#[test]
fn smoke_tools_list() {
    let mut cmd = Command::cargo_bin("jarvy").unwrap();
    cmd.arg("tools")
       .assert()
       .success()
       .stdout(predicate::str::contains("git"))
       .stdout(predicate::str::contains("node"));
}

/// Critical path: jarvy get version info
#[test]
fn smoke_get_version() {
    let mut cmd = Command::cargo_bin("jarvy").unwrap();
    cmd.arg("get")
       .arg("git")
       .assert()
       .success();
}

/// Critical path: config validation
#[test]
fn smoke_invalid_config_error() {
    let mut cmd = Command::cargo_bin("jarvy").unwrap();
    cmd.arg("setup")
       .arg("--config")
       .arg("nonexistent.toml")
       .assert()
       .failure()
       .code(2);  // CONFIG_ERROR
}

/// Critical path: bootstrap command
#[test]
#[cfg(target_os = "macos")]
fn smoke_bootstrap_macos() {
    let mut cmd = Command::cargo_bin("jarvy").unwrap();
    cmd.arg("bootstrap")
       .arg("--dry-run")
       .assert()
       .success()
       .stdout(predicate::str::contains("Homebrew"));
}

#[test]
#[cfg(target_os = "linux")]
fn smoke_bootstrap_linux() {
    let mut cmd = Command::cargo_bin("jarvy").unwrap();
    cmd.arg("bootstrap")
       .arg("--dry-run")
       .assert()
       .success();
}

#[test]
#[cfg(target_os = "windows")]
fn smoke_bootstrap_windows() {
    let mut cmd = Command::cargo_bin("jarvy").unwrap();
    cmd.arg("bootstrap")
       .arg("--dry-run")
       .assert()
       .success()
       .stdout(predicate::str::contains("winget"));
}
```

## Implementation Steps

1. Create `examples/` directory with persona-based configs
2. Write `examples/README.md` with usage instructions
3. Create smoke test file `tests/smoke_tests.rs`
4. Add GitHub Actions workflow for platform matrix
5. Implement caching strategy for tool installations
6. Add CI job to validate all example configs
7. Document testing strategy in CONTRIBUTING.md
8. Create minimal.toml for quick-start testing

## Success Metrics

| Metric | Current | Target |
|--------|---------|--------|
| Platform CI coverage | 1 (ubuntu-latest) | 7 platforms |
| Example configs | 0 | 8 personas |
| Smoke tests | 0 | 10+ critical paths |
| CI cache hit rate | N/A | > 80% |
| Time to detect platform bugs | Post-release | Pre-merge |

## Risks

1. **CI time/cost**: Matrix testing increases CI minutes
   - Mitigation: Run full matrix on main, subset on PRs
2. **Flaky platform tests**: Platform-specific timing issues
   - Mitigation: Add retries, increase timeouts
3. **Cache invalidation**: Stale caches cause false positives
   - Mitigation: Include version info in cache keys
4. **Example drift**: Examples become outdated
   - Mitigation: CI validates examples are parseable

## Dependencies

- `assert_cmd` - Command testing
- GitHub Actions with macOS/Windows runners
- Container support for Linux distros

## Effort Estimate

- Example configs creation: 1 day
- Smoke test implementation: 1 day
- CI matrix setup: 1 day
- Caching strategy: 0.5 days
- Documentation: 0.5 days
- Testing and validation: 1 day

**Total: 5 days**

## Files to Create/Modify

### Create
- `examples/README.md`
- `examples/minimal.toml`
- `examples/frontend-developer.toml`
- `examples/backend-developer.toml`
- `examples/devops-engineer.toml`
- `examples/data-scientist.toml`
- `examples/mobile-developer.toml`
- `examples/full-stack.toml`
- `examples/rust-developer.toml`
- `tests/smoke_tests.rs`
- `.github/workflows/real-world-tests.yml`

### Modify
- `CONTRIBUTING.md` - Add testing documentation
- `README.md` - Link to examples
