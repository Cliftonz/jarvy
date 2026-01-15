# PRD-001: Optimized Tool Installation

## Overview

Optimize tool installation performance through strategic parallelization where safe, batch operations, and parallel pre-flight checks.

## Problem Statement

Currently, Jarvy installs tools sequentially in `src/main.rs`. When provisioning multiple tools, each installation blocks the next. A setup with 10 tools taking 30 seconds each requires 5 minutes total.

## Critical Constraint: Package Manager Locking

**Original assumption was flawed.** Most system package managers use exclusive locking and **cannot run concurrently**:

### Platform Analysis

#### macOS (Homebrew)
**❌ Concurrent installs NOT supported**

Homebrew uses lock files at `$(brew --prefix)/var/homebrew/locks/`:
- Running multiple `brew install` commands concurrently causes lock contention
- Second process waits for the first to release the lock
- Error: `Another active Homebrew process is already in progress`

#### Linux (apt/dnf/pacman/apk)
**❌ Concurrent installs NOT supported**

| Package Manager | Lock File | Behavior |
|----------------|-----------|----------|
| **apt/apt-get** | `/var/lib/dpkg/lock-frontend`, `/var/lib/dpkg/lock` | "Could not get lock" error |
| **dnf/yum** | `/var/lib/rpm/.rpm.lock`, `/var/cache/dnf/*/lock` | "Waiting for process lock" |
| **pacman** | `/var/lib/pacman/db.lck` | "database is locked" error |
| **apk** | `/lib/apk/db/lock` | Lock error |

All Linux package managers use **exclusive locking** - only one can run at a time.

#### Windows (winget/Chocolatey)
**❌ Concurrent installs NOT supported**

- **winget**: Windows Installer (MSI) has a single-instance limitation - only one MSI can run at a time system-wide
- **Chocolatey**: Uses internal locking; concurrent runs cause unpredictable behavior
- **EXE installers**: May conflict when modifying shared resources (PATH, registry)

### What CAN vs. CANNOT Be Parallelized

| ✅ Safe to Parallelize | ❌ Must Be Sequential |
|------------------------|----------------------|
| Version checking (`--version` calls) | System package manager installs (brew, apt, dnf, pacman, winget) |
| Binary downloads (curl/wget) | MSI/pkg/deb installations |
| Git clones | Shared resource modifications (PATH, registry) |
| User-space package managers (npm, pip, cargo, go) | Any operation using exclusive locks |
| Configuration file generation | |
| Post-install hooks (if independent) | |

## Revised Requirements

### Functional Requirements

1. **Batch system PM operations**: Combine multiple tools into single PM command
2. **Parallel version checking**: Check all tool versions concurrently before installation
3. **Parallel user-space installs**: npm, pip, cargo, go can run concurrently
4. **Parallel downloads**: Download binaries/archives concurrently, install sequentially
5. **Progress reporting**: Show real-time status of operations
6. **Dependency ordering**: Respect tool dependencies (e.g., nvm before node)

### Non-Functional Requirements

1. Detect and group tools by package manager
2. Use batch install where supported (`brew install a b c`)
3. Maintain deterministic output ordering
4. Support `--sequential` flag for debugging

## Revised Technical Approach

### Phase 1: Batch Operations (High Impact, Low Risk)

Group tools by package manager and install in batches:

```rust
// Instead of:
brew install git
brew install jq
brew install ripgrep

// Do:
brew install git jq ripgrep
```

**Benefits**:
- Single dependency resolution pass
- Single lock acquisition
- Homebrew/apt can optimize internally
- ~3-5x faster for multi-tool installs

**Implementation**:
```rust
fn batch_install(pm: PackageManager, packages: Vec<&str>) -> Result<(), InstallError> {
    match pm {
        PackageManager::Brew => run("brew", &["install"].chain(packages)),
        PackageManager::Apt => run_maybe_sudo(true, "apt", &["install", "-y"].chain(packages)),
        PackageManager::Dnf => run_maybe_sudo(true, "dnf", &["install", "-y"].chain(packages)),
        // ...
    }
}
```

### Phase 2: Parallel Version Checking (Medium Impact, No Risk)

Check all tool versions concurrently before installation:

```rust
use rayon::prelude::*;

let version_results: Vec<_> = tools
    .par_iter()
    .map(|tool| (tool.name, check_version(&tool.name)))
    .collect();

// Build installation plan from results
let needs_install: Vec<_> = version_results
    .iter()
    .filter(|(_, result)| result.needs_update())
    .collect();
```

**Benefits**:
- Version checks are read-only (no locking issues)
- Can check 10+ tools in ~1 second vs ~10 seconds sequential
- Improves perceived responsiveness

### Phase 3: Parallel User-Space Package Managers (Medium Impact, Medium Risk)

npm, pip, cargo, and go install to user directories and don't conflict:

```rust
// These CAN run in parallel:
let user_space_installs = tools.iter()
    .filter(|t| matches!(t.installer, Installer::Npm | Installer::Pip | Installer::Cargo | Installer::Go));

user_space_installs.par_iter().for_each(|tool| {
    install_user_space_tool(tool);
});
```

**Caveats**:
- npm/pip with `--global` may still conflict
- Cargo builds may compete for CPU/memory
- Limit concurrency to 2-4 for resource management

### Phase 4: Parallel Downloads, Sequential Installs (Low Impact, Low Risk)

For tools installed via direct binary download:

```rust
// Download all binaries in parallel
let downloads: Vec<_> = binary_tools
    .par_iter()
    .map(|tool| download_binary(tool))
    .collect();

// Install sequentially (may modify PATH, etc.)
for (tool, binary_path) in downloads {
    install_binary(tool, binary_path)?;
}
```

## Implementation Steps

### Phase 1: Batch Operations (Recommended First)
1. Add `PackageManager` enum if not exists
2. Group tools by package manager in setup flow
3. Implement `batch_install()` for each PM
4. Handle mixed success/failure in batch operations
5. Add tests for batch installation

### Phase 2: Parallel Version Checking
1. Add `rayon` dependency
2. Refactor version checking to use `par_iter()`
3. Collect results before installation planning
4. Update progress reporting

### Phase 3: User-Space Parallelization
1. Identify user-space package managers
2. Implement parallel installation with concurrency limit
3. Add `--jobs` flag for user control
4. Handle error aggregation

### Phase 4: Parallel Downloads
1. Identify tools using binary downloads
2. Implement async/parallel download
3. Queue sequential installation

## Success Metrics

| Metric | Current | Phase 1 | Phase 2 | Full |
|--------|---------|---------|---------|------|
| 10 brew tools | ~5 min | ~1 min | ~1 min | ~1 min |
| Version check (10 tools) | ~10 sec | ~10 sec | ~2 sec | ~2 sec |
| Mixed install (brew + npm + pip) | ~3 min | ~2 min | ~2 min | ~1 min |

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Batch install partial failure | Medium | Medium | Parse output to identify failed packages, retry individually |
| Version check race conditions | Low | Low | Version checks are read-only |
| User-space PM conflicts | Low | Medium | Limit concurrency, detect conflicts |
| Resource exhaustion | Medium | Low | Default to conservative concurrency (2-4) |

## Dependencies

- `rayon` crate for parallel iteration (Phase 2+)
- No new dependencies for Phase 1

## Effort Estimate

| Phase | Effort | Impact | Priority |
|-------|--------|--------|----------|
| Phase 1: Batch Operations | 1-2 days | High | **P0** |
| Phase 2: Parallel Version Check | 1 day | Medium | P1 |
| Phase 3: User-Space Parallel | 1-2 days | Medium | P2 |
| Phase 4: Parallel Downloads | 1 day | Low | P3 |

## Files to Modify

- `src/tools/common.rs` - Add batch install functions
- `src/main.rs` - Refactor setup loop for batching
- `src/tools/spec.rs` - Add package manager grouping
- `Cargo.toml` - Add rayon (Phase 2+)
- `tests/` - Add batch and parallel tests

## Appendix: Package Manager Batch Support

| Package Manager | Batch Command | Notes |
|-----------------|---------------|-------|
| Homebrew | `brew install a b c` | ✅ Full support |
| apt | `apt install -y a b c` | ✅ Full support |
| dnf | `dnf install -y a b c` | ✅ Full support |
| pacman | `pacman -S --noconfirm a b c` | ✅ Full support |
| apk | `apk add a b c` | ✅ Full support |
| winget | `winget install a b c` | ⚠️ Limited (sequential internally) |
| Chocolatey | `choco install a b c -y` | ✅ Full support |
| npm | `npm install -g a b c` | ✅ Full support |
| pip | `pip install a b c` | ✅ Full support |
| cargo | N/A (one at a time) | ❌ No batch support |

## Revision History

| Date | Change | Reason |
|------|--------|--------|
| 2026-01-15 | Major revision: Removed naive parallelization approach | Package managers use exclusive locking; concurrent installs fail |
| 2026-01-15 | Added batch operations as primary optimization | Safe, high-impact, works with PM locking |
| 2026-01-15 | Scoped parallelization to safe operations only | Version checks, user-space PMs, downloads |
