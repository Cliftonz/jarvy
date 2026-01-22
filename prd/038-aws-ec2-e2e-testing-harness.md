# PRD-038: Hybrid Cross-Platform E2E Testing Harness

## Overview

Design and implement a comprehensive end-to-end testing harness using a **hybrid approach**: GitHub-hosted runners for macOS and Windows (cost-effective), plus AWS EC2 Spot instances for Linux distributions not available on GitHub (Fedora, Arch, Alpine, FreeBSD). This validates Jarvy tool installations across all supported platforms with real package managers in isolated, reproducible environments.

## Problem Statement

The current testing infrastructure has significant gaps in real-world platform coverage:

1. **macOS gaps**: GitHub now provides both Intel (macos-13) and Apple Silicon (macos-14/15) runners, but we're not fully utilizing them; shared runners have pre-installed tools that can mask installation failures
2. **Linux distro gaps**: GitHub only offers Ubuntu; Testcontainers work but don't test real package manager interactions (sudo, system paths, service integration); missing Fedora, Arch, Alpine native testing
3. **Windows constraints**: GitHub Windows runners work but have pre-installed tools; need to verify fresh installs
4. **No BSD coverage**: FreeBSD is supported but never tested
5. **Environment pollution**: Shared CI runners have pre-installed tools that can mask installation failures or version conflicts

## Evidence

- User reports of winget failures on Windows 11 that pass on Windows Server 2022 CI
- Apple Silicon users report ARM-specific issues not caught in x86_64-only CI

## Goals

1. **True platform parity**: Test on actual macOS (Intel + Apple Silicon), real Linux distros, Windows 10/11
2. **Fresh environment guarantee**: Every test runs on a clean, just-provisioned instance
3. **Base tool validation**: Verify a core set of tools installs correctly on each platform
4. **Cost efficiency**: Minimize AWS spend through spot instances, auto-termination, and smart scheduling
5. **CI integration**: Results flow back to GitHub PRs with clear pass/fail status
6. **Reproducibility**: Any failure can be reproduced by spinning up the same instance type

## Non-Goals

- Testing every tool (176 tools × 8 platforms = 1,408 combinations is not feasible)
- Performance benchmarking (separate concern)
- Load/stress testing (Jarvy is single-user CLI)
- Testing tool functionality beyond installation verification
- Supporting cloud providers other than AWS (can be added later)

## Architecture

### High-Level Design

This architecture uses a **hybrid approach** to minimize costs while maximizing platform coverage:

- **GitHub-hosted runners** (free): macOS Intel, macOS ARM, Ubuntu, Windows
- **AWS EC2 Spot instances** (~$0.01/run): Fedora, Arch, Alpine, FreeBSD
- **MacinCloud** (future fallback): If GitHub macOS runners prove insufficient

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         GitHub Actions Workflow                              │
│                    (PR, merge to main, scheduled, manual)                    │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
            ┌─────────────────────────┼─────────────────────────┐
            ▼                         ▼                         ▼
┌───────────────────────┐ ┌───────────────────────┐ ┌───────────────────────┐
│  GitHub-Hosted Jobs   │ │  GitHub-Hosted Jobs   │ │  Self-Hosted Jobs     │
│  (macOS - FREE)       │ │  (Ubuntu/Win - FREE)  │ │  (AWS EC2 Spot)       │
│                       │ │                       │ │                       │
│  • macos-13 (Intel)   │ │  • ubuntu-22.04       │ │  • fedora-40          │
│  • macos-14 (ARM M1)  │ │  • ubuntu-24.04       │ │  • arch-linux         │
│  • macos-15 (ARM M2)  │ │  • windows-latest     │ │  • alpine             │
│                       │ │                       │ │  • freebsd-14         │
└───────────────────────┘ └───────────────────────┘ └───────────────────────┘
            │                         │                         │
            └─────────────────────────┼─────────────────────────┘
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                        Results Aggregation Job                               │
│                                                                              │
│  • Collects results from all matrix jobs                                    │
│  • Posts PR comment with platform × tool status matrix                      │
│  • Uploads artifacts to GitHub (no S3 needed)                               │
│  • Sets final commit status (success/failure)                               │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Platform → Runner Mapping

| Platform | Runner | Type | Cost | Test Duration |
|----------|--------|------|------|---------------|
| macOS Intel | `macos-13` | GitHub-hosted | **Free** | ~10 min |
| macOS ARM (M1) | `macos-14` | GitHub-hosted | **Free** | ~8 min |
| macOS ARM (M2) | `macos-15` | GitHub-hosted | **Free** | ~8 min |
| Ubuntu 22.04 | `ubuntu-22.04` | GitHub-hosted | **Free** | ~8 min |
| Ubuntu 24.04 | `ubuntu-24.04` | GitHub-hosted | **Free** | ~8 min |
| Windows | `windows-latest` | GitHub-hosted | **Free** | ~12 min |
| Fedora 40 | `self-hosted-fedora` | AWS EC2 Spot | ~$0.01/run | ~10 min |
| Arch Linux | `self-hosted-arch` | AWS EC2 Spot | ~$0.01/run | ~10 min |
| Alpine | `self-hosted-alpine` | AWS EC2 Spot | ~$0.005/run | ~8 min |
| FreeBSD 14 | `self-hosted-freebsd` | AWS EC2 Spot | ~$0.01/run | ~10 min |

### AWS EC2 Self-Hosted Runner Strategy

For Linux distros not available on GitHub (Fedora, Arch, Alpine, FreeBSD), we use ephemeral self-hosted runners on AWS EC2 Spot instances:

```
┌─────────────────────────────────────────────────────────────────┐
│                  Ephemeral EC2 Runner Lifecycle                  │
│                                                                  │
│  1. GitHub Actions workflow triggers                            │
│  2. Runner provisioner Lambda spins up EC2 Spot instance        │
│  3. Instance registers as self-hosted runner (ephemeral)        │
│  4. Job runs on the instance                                    │
│  5. Instance auto-terminates after job completes                │
│                                                                  │
│  Cost per run (t3.medium Spot, ~20 min):                        │
│  - Fedora/Arch/FreeBSD: $0.008/hr × 0.33hr = ~$0.003           │
│  - Alpine (t3.small): $0.004/hr × 0.33hr = ~$0.001             │
│  - Total for 4 distros: ~$0.01 per E2E run                     │
└─────────────────────────────────────────────────────────────────┘
```

### Future: MacinCloud Fallback

If GitHub-hosted macOS runners prove insufficient (environment pollution, version limitations), MacinCloud can be added:

- **Pay-as-you-go**: ~$0.03-0.05/min
- **Dedicated server**: ~$20-50/month
- Integration via SSH-based self-hosted runner

## Base Tool Set Definition

The E2E harness validates a carefully selected "base tool set" that covers:

1. **All installation patterns** (brew, cask, apt, dnf, pacman, apk, winget, choco, custom)
2. **All complexity levels** (simple, dependencies, custom scripts, default hooks)
3. **High-value user tools** (most commonly requested in jarvy.toml files)

### Tier 1: Core Tools (Always Tested)

These tools MUST pass on every platform for a release to ship.

| Tool | Why Selected | Patterns Covered |
|------|-------------|------------------|
| `git` | Universal, pre-requisite for everything | Simple install, all package managers |
| `jq` | Common utility, simple install | uniform linux packages, winget |
| `ripgrep` | Rust binary, popular | brew, cargo fallback, winget |
| `curl` | Network utility, sometimes pre-installed | Version detection, skip-if-installed |
| `wget` | Network utility, package manager differences | apt vs dnf naming |

### Tier 2: Language Runtimes (Tested on Supported Platforms)

| Tool | Why Selected | Patterns Covered |
|------|-------------|------------------|
| `node` | Most requested runtime | version_manager support, nvm integration |
| `python` | Second most requested | pyenv integration, system python detection |
| `rust` | Jarvy's own language | Custom install (rustup script) |
| `go` | Popular backend language | Simple binary, GOPATH setup |

### Tier 3: Container & DevOps (Tested Where Applicable)

| Tool | Why Selected | Patterns Covered |
|------|-------------|------------------|
| `docker` | Most complex install | cask (macOS), service management, default hook |
| `kubectl` | Flexible dependencies | depends_on_one_of pattern |
| `terraform` | DevOps staple | Simple binary install |
| `awscli` | Cloud tooling | pip-based install option |

### Tier 4: Tools with Dependencies (Validates Dependency System)

| Tool | Why Selected | Patterns Covered |
|------|-------------|------------------|
| `lazydocker` | Requires docker | depends_on strict dependency |
| `lazygit` | No dependencies | Baseline for comparison |
| `k9s` | Requires kubectl | Flexible dependency chain |

### Platform-Specific Test Matrix

```
                    │ macOS │ macOS │Ubuntu│Ubuntu│Windows│Fedora│ Arch │Alpine│FreeBSD│
                    │ Intel │  ARM  │22.04 │24.04 │       │  40  │      │      │  14   │
                    │(GH)   │(GH)   │(GH)  │(GH)  │(GH)   │(EC2) │(EC2) │(EC2) │(EC2)  │
────────────────────┼───────┼───────┼──────┼──────┼───────┼──────┼──────┼──────┼───────┤
Tier 1: Core        │       │       │      │      │       │      │      │      │       │
  git               │   ✓   │   ✓   │  ✓   │  ✓   │   ✓   │  ✓   │  ✓   │  ✓   │   ✓   │
  jq                │   ✓   │   ✓   │  ✓   │  ✓   │   ✓   │  ✓   │  ✓   │  ✓   │   ✓   │
  ripgrep           │   ✓   │   ✓   │  ✓   │  ✓   │   ✓   │  ✓   │  ✓   │  ✓   │   ✓   │
  curl              │   ✓   │   ✓   │  ✓   │  ✓   │   ✓   │  ✓   │  ✓   │  ✓   │   ✓   │
  wget              │   ✓   │   ✓   │  ✓   │  ✓   │   ✓   │  ✓   │  ✓   │  ✓   │   ✓   │
────────────────────┼───────┼───────┼──────┼──────┼───────┼──────┼──────┼──────┼───────┤
Tier 2: Runtimes    │       │       │      │      │       │      │      │      │       │
  node              │   ✓   │   ✓   │  ✓   │  ✓   │   ✓   │  ✓   │  ✓   │  ✓   │   ✗   │
  python            │   ✓   │   ✓   │  ✓   │  ✓   │   ✓   │  ✓   │  ✓   │  ✓   │   ✓   │
  rust              │   ✓   │   ✓   │  ✓   │  ✓   │   ✓   │  ✓   │  ✓   │  ✓   │   ✓   │
  go                │   ✓   │   ✓   │  ✓   │  ✓   │   ✓   │  ✓   │  ✓   │  ✓   │   ✓   │
────────────────────┼───────┼───────┼──────┼──────┼───────┼──────┼──────┼──────┼───────┤
Tier 3: DevOps      │       │       │      │      │       │      │      │      │       │
  docker            │   ✓   │   ✓   │  ✓   │  ✓   │   ✓   │  ✓   │  ✓   │  ✗   │   ✗   │
  kubectl           │   ✓   │   ✓   │  ✓   │  ✓   │   ✓   │  ✓   │  ✓   │  ✓   │   ✗   │
  terraform         │   ✓   │   ✓   │  ✓   │  ✓   │   ✓   │  ✓   │  ✓   │  ✓   │   ✗   │
  awscli            │   ✓   │   ✓   │  ✓   │  ✓   │   ✓   │  ✓   │  ✓   │  ✓   │   ✗   │
────────────────────┼───────┼───────┼──────┼──────┼───────┼──────┼──────┼──────┼───────┤
Tier 4: Dependencies│       │       │      │      │       │      │      │      │       │
  lazydocker        │   ✓   │   ✓   │  ✓   │  ✓   │   ✓   │  ✓   │  ✓   │  ✗   │   ✗   │
  lazygit           │   ✓   │   ✓   │  ✓   │  ✓   │   ✓   │  ✓   │  ✓   │  ✓   │   ✓   │
  k9s               │   ✓   │   ✓   │  ✓   │  ✓   │   ✓   │  ✓   │  ✓   │  ✓   │   ✗   │
────────────────────┴───────┴───────┴──────┴──────┴───────┴──────┴──────┴──────┴───────┘

Legend: ✓ = Tested, ✗ = Not applicable/supported
(GH) = GitHub-hosted runner (FREE), (EC2) = AWS EC2 Spot (~$0.01/run)
```

## User Stories

### US-001: GitHub-Hosted Platform Testing

**As a** Jarvy maintainer,
**I want** E2E tests to run on GitHub-hosted runners for macOS, Ubuntu, and Windows,
**So that** I get free, reliable testing on the most common platforms.

**Acceptance Criteria:**
- macOS tests run on both `macos-13` (Intel) and `macos-14` (Apple Silicon M1)
- Ubuntu tests run on `ubuntu-22.04` and `ubuntu-24.04`
- Windows tests run on `windows-latest` with winget available
- All GitHub-hosted jobs run in parallel via matrix strategy
- Tests verify Jarvy builds and installs base tool set successfully

### US-002: AWS EC2 Spot Testing for Additional Distros

**As a** Jarvy maintainer,
**I want** E2E tests on Fedora, Arch, Alpine, and FreeBSD via AWS EC2 Spot instances,
**So that** I can verify package manager support on distros not available on GitHub.

**Acceptance Criteria:**
- EC2 Spot instances used for Fedora 40, Arch Linux, Alpine, and FreeBSD 14
- Ephemeral self-hosted runners register, run job, then auto-terminate
- Spot interruption handled gracefully (job marked as "interrupted", not failed)
- Cost per run < $0.05 for all 4 distros combined
- Custom AMIs built with Packer include Rust toolchain pre-installed

### US-003: Multi-Platform Parallel Execution

**As a** Jarvy maintainer,
**I want** tests to run in parallel across all platforms,
**So that** the full test suite completes in under 20 minutes.

**Acceptance Criteria:**
- GitHub Actions matrix runs all platforms concurrently
- Individual platform failures don't block other platforms (`fail-fast: false`)
- Results aggregation job waits for all matrix jobs
- Total wall-clock time < 20 minutes for full 9-platform suite

### US-004: GitHub PR Integration

**As a** Jarvy maintainer,
**I want** E2E test results integrated with GitHub PRs,
**So that** I can see platform compatibility before merging.

**Acceptance Criteria:**
- PR commit status shows "pending", "success", or "failure"
- PR comment includes result matrix (platform × tool × status)
- Failed tests include links to GitHub artifacts for debugging
- Manual re-run available via workflow_dispatch or "Re-run jobs" button
- Results visible within 5 minutes of job completion

### US-005: Test Result Artifacts

**As a** Jarvy maintainer,
**I want** comprehensive test artifacts stored in GitHub Actions,
**So that** I can debug failures without re-running tests.

**Acceptance Criteria:**
- Each platform job uploads artifacts with unique naming
- Artifacts include:
  - `results.json` - Structured test results (tool, status, duration, error)
  - `jarvy-output.log` - Full stdout/stderr from jarvy commands
  - `system-info.txt` - OS version, arch, package manager versions
- Artifacts retained for 30 days (GitHub default)
- Artifacts downloadable directly from workflow run page

### US-006: Scheduled Nightly Full Suite

**As a** Jarvy maintainer,
**I want** a nightly full E2E test run,
**So that** platform regressions are caught even without PRs.

**Acceptance Criteria:**
- Nightly run at 2 AM UTC via `schedule` trigger
- Full platform matrix (all 9 platforms)
- Results posted to GitHub Actions summary
- Automatic GitHub issue creation for new failures (optional)

### US-007: MacinCloud Fallback (Future)

**As a** Jarvy maintainer,
**I want** the option to add MacinCloud as a macOS testing fallback,
**So that** I can get cleaner environments if GitHub runners prove insufficient.

**Acceptance Criteria:**
- Architecture documented for MacinCloud integration
- SSH-based self-hosted runner setup instructions provided
- Cost comparison documented (GitHub free vs MacinCloud ~$20-50/mo)
- Migration path from GitHub runners to MacinCloud defined

## Infrastructure Components

### 1. Terraform Modules (Minimal - EC2 Only)

Only needed for the 4 Linux distros not available on GitHub:

```
infra/
├── README.md
├── modules/
│   └── ec2-runner/
│       ├── main.tf          # EC2 Spot instance + security group + IAM role
│       ├── variables.tf     # Platform, AMI, instance type inputs
│       ├── outputs.tf       # Instance ID, runner token
│       └── user-data.sh     # Bootstrap + register as GH runner
├── environments/
│   └── prod/
│       ├── main.tf          # Instantiates 4 runners (fedora, arch, alpine, freebsd)
│       └── terraform.tfvars
└── packer/
    ├── fedora-40.pkr.hcl
    ├── arch-linux.pkr.hcl
    ├── alpine.pkr.hcl
    ├── freebsd-14.pkr.hcl
    └── scripts/
        └── install-rust-and-deps.sh
```

### 2. EC2 Runner User-Data Script

```bash
#!/bin/bash
# Registers as ephemeral GitHub Actions self-hosted runner

set -euo pipefail

GITHUB_TOKEN="${1}"
REPO="${2}"
LABELS="${3}"  # e.g., "self-hosted-fedora"

# Download runner
mkdir -p /opt/actions-runner && cd /opt/actions-runner
curl -o actions-runner.tar.gz -L https://github.com/actions/runner/releases/download/v2.311.0/actions-runner-linux-x64-2.311.0.tar.gz
tar xzf actions-runner.tar.gz

# Configure as ephemeral runner (auto-removes after one job)
./config.sh --url "https://github.com/${REPO}" \
  --token "${GITHUB_TOKEN}" \
  --labels "${LABELS}" \
  --ephemeral \
  --unattended

# Start runner
./run.sh
```

### 3. Test Runner Script

```bash
#!/bin/bash
# scripts/e2e-test-runner.sh
# Executed on each EC2 instance via user-data

set -euo pipefail

COMMIT_SHA="${1:-HEAD}"
RESULTS_BUCKET="${2:-jarvy-e2e-results}"
RUN_ID="${3:-$(date +%Y%m%d-%H%M%S)}"
PLATFORM="${4:-unknown}"

RESULTS_DIR="/tmp/jarvy-e2e-results"
mkdir -p "$RESULTS_DIR"

# Capture system info
capture_system_info() {
    cat > "$RESULTS_DIR/system-info.json" << EOF
{
  "platform": "$PLATFORM",
  "os": "$(uname -s)",
  "arch": "$(uname -m)",
  "kernel": "$(uname -r)",
  "hostname": "$(hostname)",
  "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
}
EOF
}

# Install Jarvy from commit
install_jarvy() {
    echo "Installing Jarvy from commit $COMMIT_SHA..."

    # Clone and build
    git clone --depth 1 https://github.com/YOUR_ORG/jarvy.git /tmp/jarvy
    cd /tmp/jarvy
    git fetch origin "$COMMIT_SHA"
    git checkout "$COMMIT_SHA"

    # Build release binary
    cargo build --release

    # Make available system-wide
    sudo cp target/release/jarvy /usr/local/bin/
    jarvy --version
}

# Run tool installation tests
run_tests() {
    local tools=("git" "jq" "ripgrep" "curl" "wget" "node" "python" "rust" "go")
    local results=()

    for tool in "${tools[@]}"; do
        echo "Testing installation of $tool..."
        local start_time=$(date +%s)
        local status="success"
        local error=""

        # Create minimal jarvy.toml for single tool
        cat > /tmp/test-jarvy.toml << EOF
[provisioner]
$tool = "latest"
EOF

        # Run jarvy setup
        if ! jarvy setup --file /tmp/test-jarvy.toml 2>&1 | tee "$RESULTS_DIR/$tool.log"; then
            status="failed"
            error=$(tail -20 "$RESULTS_DIR/$tool.log")
        fi

        # Verify tool is actually installed
        if [ "$status" = "success" ]; then
            if ! command -v "$tool" &> /dev/null; then
                status="failed"
                error="Tool not found in PATH after installation"
            fi
        fi

        local end_time=$(date +%s)
        local duration=$((end_time - start_time))

        results+=("{\"tool\": \"$tool\", \"status\": \"$status\", \"duration\": $duration, \"error\": \"$error\"}")
    done

    # Write results JSON
    echo "[$(IFS=,; echo "${results[*]}")]" > "$RESULTS_DIR/results.json"
}

# Upload results to S3
upload_results() {
    aws s3 sync "$RESULTS_DIR" "s3://$RESULTS_BUCKET/$RUN_ID/$PLATFORM/"
}

# Main execution
main() {
    capture_system_info
    install_jarvy
    run_tests
    upload_results

    # Signal completion
    echo "E2E tests completed for $PLATFORM"
}

main "$@"
```

### 4. GitHub Actions Workflow

```yaml
# .github/workflows/e2e-cross-platform.yml
name: E2E Cross-Platform Tests

on:
  push:
    branches: [main]
  pull_request:
  schedule:
    - cron: '0 2 * * *'  # Nightly at 2 AM UTC
  workflow_dispatch:

permissions:
  contents: read
  pull-requests: write

concurrency:
  group: e2e-${{ github.ref }}
  cancel-in-progress: true

jobs:
  # ============================================================
  # GitHub-Hosted Runners (FREE)
  # ============================================================
  github-hosted:
    name: E2E (${{ matrix.name }})
    strategy:
      fail-fast: false
      matrix:
        include:
          # macOS
          - os: macos-13
            name: macOS Intel
          - os: macos-14
            name: macOS ARM (M1)
          # Ubuntu
          - os: ubuntu-22.04
            name: Ubuntu 22.04
          - os: ubuntu-24.04
            name: Ubuntu 24.04
          # Windows
          - os: windows-latest
            name: Windows

    runs-on: ${{ matrix.os }}
    timeout-minutes: 30

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Cache Cargo
        uses: Swatinem/rust-cache@v2

      - name: Build Jarvy
        run: cargo build --release

      - name: Run E2E Tests
        run: cargo test --test e2e_base_tools -- --show-output
        env:
          JARVY_BIN: ${{ github.workspace }}/target/release/jarvy${{ runner.os == 'Windows' && '.exe' || '' }}
          JARVY_TEST_MODE: 1

      - name: Upload Results
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: e2e-results-${{ matrix.name }}
          path: |
            target/e2e-results/
            *.log

  # ============================================================
  # AWS EC2 Spot Runners (for distros not on GitHub)
  # ============================================================
  ec2-hosted:
    name: E2E (${{ matrix.name }})
    if: github.event_name == 'push' || github.event_name == 'schedule' || github.event_name == 'workflow_dispatch'
    strategy:
      fail-fast: false
      matrix:
        include:
          - runner: self-hosted-fedora
            name: Fedora 40
          - runner: self-hosted-arch
            name: Arch Linux
          - runner: self-hosted-alpine
            name: Alpine
          - runner: self-hosted-freebsd
            name: FreeBSD 14

    runs-on: ${{ matrix.runner }}
    timeout-minutes: 30

    steps:
      - uses: actions/checkout@v4

      - name: Build Jarvy
        run: cargo build --release

      - name: Run E2E Tests
        run: cargo test --test e2e_base_tools -- --show-output
        env:
          JARVY_BIN: ${{ github.workspace }}/target/release/jarvy
          JARVY_TEST_MODE: 1

      - name: Upload Results
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: e2e-results-${{ matrix.name }}
          path: target/e2e-results/

  # ============================================================
  # Results Aggregation
  # ============================================================
  aggregate-results:
    name: Aggregate Results
    needs: [github-hosted, ec2-hosted]
    if: always()
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v4

      - name: Download all artifacts
        uses: actions/download-artifact@v4
        with:
          path: all-results/

      - name: Generate Summary
        run: |
          echo "## E2E Test Results" >> $GITHUB_STEP_SUMMARY
          echo "" >> $GITHUB_STEP_SUMMARY
          echo "| Platform | Status |" >> $GITHUB_STEP_SUMMARY
          echo "|----------|--------|" >> $GITHUB_STEP_SUMMARY
          for dir in all-results/e2e-results-*/; do
            platform=$(basename "$dir" | sed 's/e2e-results-//')
            if [ -f "$dir/results.json" ]; then
              status="✅ Passed"
            else
              status="❌ Failed"
            fi
            echo "| $platform | $status |" >> $GITHUB_STEP_SUMMARY
          done

      - name: Post PR Comment
        if: github.event_name == 'pull_request'
        uses: actions/github-script@v7
        with:
          script: |
            const fs = require('fs');
            const summary = fs.readFileSync('${{ github.step_summary }}', 'utf8');
            github.rest.issues.createComment({
              owner: context.repo.owner,
              repo: context.repo.repo,
              issue_number: context.issue.number,
              body: summary
            });
```

## Cost Analysis

### Monthly Cost Estimate (Hybrid Architecture)

| Component | Usage | Unit Cost | Monthly Cost |
|-----------|-------|-----------|--------------|
| **GitHub-Hosted Runners** | | | |
| - macOS (Intel + ARM) | Included in Actions | Free | **$0** |
| - Ubuntu (22.04 + 24.04) | Included in Actions | Free | **$0** |
| - Windows | Included in Actions | Free | **$0** |
| **AWS EC2 Spot Instances** | | | |
| - Fedora 40 (t3.medium) | 0.33hr × 100 runs | $0.008/hr | ~$0.26 |
| - Arch Linux (t3.medium) | 0.33hr × 100 runs | $0.008/hr | ~$0.26 |
| - Alpine (t3.small) | 0.25hr × 100 runs | $0.004/hr | ~$0.10 |
| - FreeBSD 14 (t3.medium) | 0.33hr × 100 runs | $0.008/hr | ~$0.26 |
| **Storage** | | | |
| - AMIs (4 custom images) | ~40GB total | $0.05/GB | ~$2 |
| **Data Transfer** | | | |
| - EC2 outbound | ~5GB | $0.09/GB | ~$0.45 |
| | | | |
| **Total Monthly Cost** | | | **~$3-5** |

### Cost Comparison

| Approach | Monthly Cost | Notes |
|----------|-------------|-------|
| Original PRD (AWS dedicated hosts) | ~$925 | macOS hosts dominate cost |
| **Hybrid (this PRD)** | **~$3-5** | 99.5% cost reduction |
| Future with MacinCloud | ~$25-55 | If GitHub macOS proves insufficient |

### Why This Is So Cheap

1. **GitHub-hosted runners are free** for public repos (and included in paid plans)
2. **EC2 Spot instances** cost ~$0.008/hr (vs $0.0416 on-demand = 80% savings)
3. **Ephemeral runners** only run for ~20 minutes per test, then terminate
4. **No always-on infrastructure** - no Lambda, Step Functions, or S3 buckets needed
5. **GitHub Actions handles orchestration** - no custom coordination code required

## Implementation Phases

### Phase 1: GitHub-Native E2E (Week 1)

Create the E2E test workflow using only GitHub-hosted runners.

1. Create `tests/e2e_base_tools.rs` integration test
2. Create `.github/workflows/e2e-cross-platform.yml` with matrix strategy
3. Configure macOS (Intel + ARM), Ubuntu (22.04 + 24.04), Windows matrix
4. Implement result aggregation job with PR comments
5. Add nightly schedule trigger

**Deliverables:**
- Working E2E tests on 5 platforms (all GitHub-hosted)
- PR comments with result matrix
- Nightly scheduled runs

### Phase 2: AWS EC2 for Additional Distros (Week 2)

Add EC2 Spot runners for Fedora, Arch, Alpine, and FreeBSD.

1. Create Packer templates for 4 custom AMIs
2. Create minimal Terraform for EC2 runner provisioning
3. Configure self-hosted runner registration in user-data
4. Add EC2-hosted jobs to GitHub workflow
5. Test Spot interruption handling

**Deliverables:**
- 4 custom AMIs with Rust toolchain
- Terraform module for ephemeral runners
- Full 9-platform E2E coverage

### Phase 3: Production Polish (Week 3)

Harden the system and document everything.

1. Add retry logic for flaky tests
2. Implement better error messages and debugging info
3. Create runbook for common issues
4. Document architecture and maintenance procedures
5. Set up AWS cost alerts ($10/month threshold)

**Deliverables:**
- Reliable E2E suite with <5% flake rate
- Operational documentation
- Cost monitoring

### Phase 4: MacinCloud Integration (Future - Optional)

Only implement if GitHub macOS runners prove insufficient.

1. Set up MacinCloud account and SSH access
2. Configure self-hosted runner on MacinCloud
3. Add to workflow matrix with conditional logic
4. Document cost trade-offs

**Deliverables:**
- MacinCloud runner integration (if needed)
- Cost comparison documentation

## Success Metrics

| Metric | Current | Target |
|--------|---------|--------|
| Platform coverage | 3 (ubuntu, macos-github, windows-github) | 9 platforms |
| E2E test availability | 0 | 100% (on every PR) |
| P95 test duration | N/A | < 20 minutes |
| Flake rate | Unknown | < 5% |
| Monthly cost | ~$0 (GitHub free tier) | **< $10** |
| Test reliability | Unknown | > 95% pass rate for known-good commits |

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| GitHub runner environment pollution | Medium | Medium | Document known pre-installed tools; test "install over existing" |
| Spot instance interruptions | Low | Low | Retry logic; jobs only run on merge/nightly, not every PR |
| AMI drift causes failures | Medium | Medium | Monthly AMI rebuilds; pin versions |
| Package manager rate limiting | Low | Medium | Spread tests; use mirrors if needed |
| Self-hosted runner security | Low | High | Ephemeral runners; isolated VPC; no secrets on instance |

## Security Considerations

1. **Ephemeral runners**: EC2 instances terminate after single job, no persistent state
2. **IAM least privilege**: Runner IAM role only allows self-registration and termination
3. **VPC isolation**: EC2 instances in private subnet with NAT gateway for egress only
4. **No secrets on runners**: GitHub Actions secrets never reach EC2; only used for runner token
5. **GitHub token scoping**: Runner registration token has minimal permissions

## Dependencies

### AWS Services (Minimal)
- EC2 (Spot instances only, t3.medium/small)
- IAM (runner role)
- VPC (optional, can use default)

### External Dependencies
- GitHub Actions (orchestration, artifacts, PR comments)
- Package manager mirrors (brew, apt, dnf, pacman, apk, winget, pkg)

### Internal Dependencies
- PRD-006: Testing Infrastructure (base test patterns)
- PRD-014: Real-World Testing (smoke test approach)

## Files to Create

```
# Minimal infrastructure (only for EC2 runners)
infra/
├── README.md
├── modules/
│   └── ec2-runner/
│       ├── main.tf           # EC2 Spot + security group + IAM
│       ├── variables.tf
│       ├── outputs.tf
│       └── user-data.sh      # Register as ephemeral GH runner
├── packer/
│   ├── fedora-40.pkr.hcl
│   ├── arch-linux.pkr.hcl
│   ├── alpine.pkr.hcl
│   └── freebsd-14.pkr.hcl
└── environments/
    └── prod/
        ├── main.tf
        └── terraform.tfvars

# Test files
tests/
└── e2e_base_tools.rs         # E2E integration test

# GitHub workflow
.github/workflows/
└── e2e-cross-platform.yml    # Main E2E workflow

# Configuration
tests/fixtures/
└── e2e-base-tools.toml       # Base tool set for testing
```

## Effort Estimate

| Phase | Description | Effort |
|-------|-------------|--------|
| 1 | GitHub-native E2E (5 platforms) | 3 days |
| 2 | AWS EC2 for additional distros | 4 days |
| 3 | Production polish & docs | 2 days |
| 4 | MacinCloud integration (future, optional) | 2 days |
| **Total (Phases 1-3)** | | **~9 days (2 weeks)** |

## Open Questions

1. **EC2 distro priority**: Should we start with all 4 (Fedora, Arch, Alpine, FreeBSD) or prioritize?
2. **PR vs main**: Should EC2 tests run on PRs, or only on merge to main / nightly?
3. **Failure policy**: Should E2E failures block merges, or just be advisory?

## Appendix A: Custom AMI Specifications (EC2 Only)

Only 4 custom AMIs needed for distros not available on GitHub:

### Fedora 40
- Base: Official Fedora Cloud AMI (`ami-fedora-40-*`)
- Additions: `@development-tools`, `git`, `curl`, Rust toolchain (via rustup)
- User: `fedora` (sudo enabled)
- GitHub runner: Pre-installed in `/opt/actions-runner`

### Arch Linux
- Base: Community Arch Linux AMI
- Additions: `base-devel`, `git`, `curl`, Rust toolchain (via rustup)
- User: `arch` (sudo enabled)
- GitHub runner: Pre-installed in `/opt/actions-runner`

### Alpine Linux
- Base: Community Alpine AMI
- Additions: `build-base`, `git`, `curl`, Rust toolchain (via rustup)
- User: `alpine` (doas enabled)
- GitHub runner: Pre-installed in `/opt/actions-runner`

### FreeBSD 14
- Base: Community FreeBSD AMI
- Additions: `devel/git`, `ftp/curl`, Rust toolchain (via rustup)
- User: `freebsd` (sudo via doas)
- GitHub runner: Pre-installed in `/opt/actions-runner`

## Appendix B: Test Result Schema

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "required": ["platform", "commit_sha", "timestamp", "results"],
  "properties": {
    "platform": { "type": "string" },
    "commit_sha": { "type": "string" },
    "timestamp": { "type": "string", "format": "date-time" },
    "runner_type": { "enum": ["github-hosted", "self-hosted-ec2"] },
    "duration_seconds": { "type": "integer" },
    "results": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["tool", "status"],
        "properties": {
          "tool": { "type": "string" },
          "status": { "enum": ["success", "failed", "skipped", "timeout"] },
          "duration_seconds": { "type": "integer" },
          "error_message": { "type": "string" },
          "installed_version": { "type": "string" }
        }
      }
    }
  }
}
```

## Appendix C: MacinCloud Integration (Future Reference)

If GitHub macOS runners prove insufficient, MacinCloud can be integrated:

### Setup Steps
1. Create MacinCloud account (pay-as-you-go or dedicated server)
2. SSH into Mac instance and install GitHub Actions runner
3. Configure as self-hosted runner with labels `self-hosted-macincloud`
4. Add to workflow matrix with conditional logic

### Cost Comparison
| Option | Cost | Environment Quality |
|--------|------|---------------------|
| GitHub `macos-13` | Free | Good (some pre-installed tools) |
| GitHub `macos-14` | Free | Good (Apple Silicon M1) |
| MacinCloud PAYG | ~$0.03-0.05/min | Excellent (clean environment) |
| MacinCloud Dedicated | ~$30-50/month | Excellent + always available |

### When to Consider MacinCloud
- GitHub macOS runners have environment pollution causing false passes
- Need specific macOS version not available on GitHub
- Need longer test runs (GitHub limit: 6 hours)
- Need to test GUI applications (Docker Desktop, etc.)
