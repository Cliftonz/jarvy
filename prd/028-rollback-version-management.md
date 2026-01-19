# PRD-028: Rollback & Version Management

WILL NOT IMPLEMENT

## Overview

Add comprehensive version management features including rollback to previous versions, lock files for reproducibility, environment snapshots, version history tracking, and migration path detection.

## Problem Statement

Users have no safety net when tool versions cause problems:
- "It worked yesterday" with no way to go back
- No safe way to test new versions before committing
- CI builds aren't reproducible without exact version locking
- No history of what versions were installed when
- Breaking changes in tools surprise users
- Different machines have version drift

Version management is critical for stability, especially in team and CI environments.

## Evidence

- Common complaint: "Update broke my setup, how do I undo?"
- CI reproducibility requires exact version pinning
- Tools like npm, cargo, poetry all have lock files
- Version conflicts are a top support issue
- Teams experience "works on my machine" problems

## Requirements

### Functional Requirements

1. **Rollback command**: `jarvy rollback <tool>` to previous version
2. **Lock files**: `jarvy.lock` for exact version pinning
3. **Environment snapshots**: Save/restore the entire state
4. **Version history**: Track installed versions over time
5. **Migration detection**: Warn about breaking changes
6. **Version diffing**: Compare environments

### Non-Functional Requirements

1. Rollback completes in < 2 minutes for any tool
2. Lock files are human-readable
3. Snapshots are efficient (incremental where possible)
4. History doesn't grow unbounded
5. Works across all platforms

## Non-Goals

- Per-project tool installations (see PRD-029)
- Automatic version conflict resolution
- Semantic versioning enforcement
- Tool deprecation management
- Multi-version parallel installation

## Feature Specifications

### 1. Rollback Command

Restore a tool to its previous version.

```bash
# Rollback a tool to previous version
jarvy rollback node

# Output:
# Rolling back node...
#
# Current version: 21.0.0
# Previous version: 20.11.0 (installed 3 days ago)
#
# Changes:
#   node: 21.0.0 -> 20.11.0
#   npm: 10.3.0 -> 10.2.4 (bundled)
#
# ? Proceed with rollback? (Y/n)
#
# Rolling back...
#   Uninstalling node 21.0.0...
#   Installing node 20.11.0...
#   Verifying installation...
#
# ✓ Rolled back node to 20.11.0
#
# Note: Run hooks may need to be re-executed
#       jarvy hooks run node

# Rollback to specific version
jarvy rollback node --to 18.19.0

# Output:
# Rolling back node to 18.19.0...
#
# Version history for node:
#   21.0.0  - current (installed 2024-01-15)
#   20.11.0 - previous (installed 2024-01-12)
#   20.10.0 - (installed 2024-01-05)
#   18.19.0 - (installed 2023-12-20)
#   18.18.0 - (installed 2023-11-15)
#
# ? Roll back to 18.19.0? (Y/n)

# Show rollback history
jarvy rollback --history node

# Output:
# Version History: node
# =====================
#
# Version    Installed          Method      Duration
# ───────────────────────────────────────────────────
# 21.0.0     2024-01-15 10:30   nvm         current
# 20.11.0    2024-01-12 14:22   nvm         3 days
# 20.10.0    2024-01-05 09:15   nvm         7 days
# 18.19.0    2023-12-20 16:45   nvm         16 days
# 18.18.0    2023-11-15 11:00   nvm         35 days
#
# Total versions tracked: 5
# Rollback available: Yes

# Rollback all tools to previous state
jarvy rollback --all

# Dry run (show what would happen)
jarvy rollback node --dry-run
```

**Rollback features:**
- Rollback to immediate previous version
- Rollback to specific historical version
- Version history display
- Bundled dependency handling
- Dry run mode
- Batch rollback

### 2. Lock Files

Exact version pinning for reproducibility.

```bash
# Generate lock file from current state
jarvy lock

# Output:
# Generating jarvy.lock...
#
# Analyzing installed tools...
#   git: 2.43.0
#   node: 20.11.0
#   docker: 24.0.7
#   jq: 1.7.1
#   ripgrep: 14.0.0
#   fd: 9.0.0
#   bat: 0.24.0
#   starship: 1.17.0
#
# ✓ Generated jarvy.lock
#
# This file ensures reproducible installations.
# Commit it to version control.

# Install using lock file
jarvy setup --locked

# Output:
# Installing from jarvy.lock...
#
# Lock file version: 1
# Generated: 2024-01-15T10:30:00Z
# Platform: darwin-arm64
#
# Installing locked versions:
#   [1/8] git 2.43.0... ✓
#   [2/8] node 20.11.0... ✓
#   [3/8] docker 24.0.7... ✓
#   ...
#
# ✓ All tools installed at locked versions

# Update lock file (all tools)
jarvy lock update

# Output:
# Updating jarvy.lock...
#
# Changes:
#   node: 20.11.0 -> 21.0.0 (major update)
#   docker: 24.0.7 -> 25.0.0 (major update)
#   jq: 1.7.1 (no change)
#   ripgrep: 14.0.0 -> 14.1.0 (minor update)
#   ...
#
# ? Update lock file with these changes? (Y/n)

# Update specific tool in lock
jarvy lock update node

# Check lock status
jarvy lock status

# Output:
# Lock File Status
# ================
#
# Lock file: jarvy.lock
# Generated: 2024-01-15T10:30:00Z
# Platform: darwin-arm64
#
# Tool         Locked     Installed   Available   Status
# ─────────────────────────────────────────────────────────
# git          2.43.0     2.43.0      2.44.0      ⚠ Update
# node         20.11.0    20.11.0     21.0.0      ⚠ Major
# docker       24.0.7     24.0.7      25.0.0      ⚠ Major
# jq           1.7.1      1.7.1       1.7.1       ✓ Current
# ripgrep      14.0.0     14.1.0      14.1.0      ✗ Drift
# ...
#
# Summary:
#   ✓ Matching: 6 tools
#   ✗ Drifted: 1 tool (installed differs from locked)
#   ⚠ Updates: 3 tools available

# Verify lock integrity
jarvy lock verify

# Output:
# Verifying jarvy.lock integrity...
#
# Checksum verification:
#   git: ✓ sha256 matches
#   node: ✓ sha256 matches
#   ...
#
# Installation verification:
#   git: ✓ 2.43.0 installed
#   node: ✓ 20.11.0 installed
#   ripgrep: ✗ 14.1.0 installed (locked: 14.0.0)
#   ...
#
# Result: MISMATCH
#   1 tool has drifted from lock file
#
# To restore locked versions:
#   jarvy setup --locked --force
```

**Lock file format:**

```toml
# jarvy.lock
# Generated by Jarvy - DO NOT EDIT MANUALLY
# https://jarvy.dev/docs/lock-files

[metadata]
version = 1
generated = "2024-01-15T10:30:00Z"
jarvy_version = "0.1.0"
config_hash = "sha256:abc123..."

[platforms.darwin-arm64]

[platforms.darwin-arm64.tools.git]
version = "2.43.0"
source = "homebrew"
formula = "git"
checksum = "sha256:def456..."
installed = "2024-01-15T10:30:00Z"

[platforms.darwin-arm64.tools.node]
version = "20.11.0"
source = "nvm"
checksum = "sha256:ghi789..."
installed = "2024-01-12T14:22:00Z"
dependencies = ["npm@10.2.4"]

[platforms.darwin-arm64.tools.docker]
version = "24.0.7"
source = "homebrew-cask"
cask = "docker"
checksum = "sha256:jkl012..."
installed = "2024-01-10T09:00:00Z"

# ... more tools

[platforms.linux-x64]
# Linux-specific locked versions
# ...
```

**Lock features:**
- Platform-specific versions
- Checksum verification
- Source/method tracking
- Dependency locking
- Config file hash

### 3. Environment Snapshots

Save and restore entire environment state.

```bash
# Create snapshot
jarvy snapshot create

# Output:
# Creating environment snapshot...
#
# Capturing:
#   ✓ Tool versions (8 tools)
#   ✓ Configuration (jarvy.toml)
#   ✓ Hook state
#   ✓ Environment variables (jarvy-related)
#
# ✓ Snapshot created: snapshot-20240115-103045
#
# Metadata:
#   ID: snapshot-20240115-103045
#   Size: 12.4 KB
#   Tools: 8
#   Platform: darwin-arm64
#
# Restore with: jarvy snapshot restore snapshot-20240115-103045

# Create named snapshot
jarvy snapshot create --name "before-upgrade"

# List snapshots
jarvy snapshot list

# Output:
# Environment Snapshots
# =====================
#
# Name                      Created              Tools  Size
# ────────────────────────────────────────────────────────────
# before-upgrade            2024-01-15 10:30     8      12.4 KB
# working-state             2024-01-12 14:22     7      11.2 KB
# snapshot-20240105-091500  2024-01-05 09:15     6      10.1 KB
#
# Total: 3 snapshots (33.7 KB)

# Show snapshot details
jarvy snapshot show before-upgrade

# Output:
# Snapshot: before-upgrade
# ========================
#
# Created: 2024-01-15 10:30:45
# Platform: darwin-arm64
# Config hash: sha256:abc123...
#
# Tools:
#   git        2.43.0   (homebrew)
#   node       20.11.0  (nvm)
#   docker     24.0.7   (homebrew-cask)
#   jq         1.7.1    (homebrew)
#   ripgrep    14.0.0   (homebrew)
#   fd         9.0.0    (homebrew)
#   bat        0.24.0   (homebrew)
#   starship   1.17.0   (homebrew)
#
# Hooks configured: 3
# Environment vars: 2

# Restore snapshot
jarvy snapshot restore before-upgrade

# Output:
# Restoring snapshot: before-upgrade
#
# Current state vs snapshot:
#   node: 21.0.0 -> 20.11.0 (downgrade)
#   docker: 25.0.0 -> 24.0.7 (downgrade)
#   ripgrep: 14.0.0 (no change)
#   ...
#
# ? Proceed with restore? (Y/n)
#
# Restoring...
#   [1/2] Downgrading node... ✓
#   [2/2] Downgrading docker... ✓
#
# ✓ Snapshot restored
#
# Note: A backup snapshot was created: pre-restore-20240115-110000

# Delete snapshot
jarvy snapshot delete snapshot-20240105-091500

# Compare snapshots
jarvy snapshot diff before-upgrade working-state
```

**Snapshot features:**
- Complete environment capture
- Named snapshots
- Automatic pre-restore backup
- Snapshot comparison
- Space-efficient storage

### 4. Version History Tracking

Maintain history of installed versions.

```bash
# View version history for a tool
jarvy history node

# Output:
# Version History: node
# =====================
#
# Current: 21.0.0
#
# Date                Version    Method    Duration    Notes
# ───────────────────────────────────────────────────────────────
# 2024-01-15 10:30    21.0.0     upgrade   current     jarvy upgrade
# 2024-01-12 14:22    20.11.0    install   3 days      jarvy setup
# 2024-01-05 09:15    20.10.0    upgrade   7 days      jarvy upgrade
# 2023-12-20 16:45    18.19.0    install   16 days     jarvy setup
# 2023-11-15 11:00    18.18.0    install   35 days     initial
#
# Total changes: 5
# Average duration: 15 days

# View all tools history
jarvy history --all

# Output:
# Version History Summary
# =======================
#
# Tool        Changes  Current     First Installed
# ───────────────────────────────────────────────────
# git         3        2.43.0      2023-10-01
# node        5        21.0.0      2023-11-15
# docker      4        25.0.0      2023-10-15
# jq          2        1.7.1       2023-12-01
# ...
#
# Total tracked: 8 tools
# Total changes: 24

# View history with timeline
jarvy history --timeline

# Output:
# Version Timeline
# ================
#
# 2024-01
#   15: node 20.11.0 -> 21.0.0
#   15: docker 24.0.7 -> 25.0.0
#   12: node 20.10.0 -> 20.11.0
#   05: ripgrep 13.0.0 -> 14.0.0
#
# 2023-12
#   20: node 18.18.0 -> 18.19.0
#   15: git 2.42.0 -> 2.43.0
#   01: jq 1.6 -> 1.7.1
#   ...

# Export history
jarvy history --export history.json

# Prune old history
jarvy history prune --older-than 90d
```

**History features:**
- Per-tool version history
- Change timestamps
- Installation method tracking
- Timeline view
- Export capability
- Automatic pruning

### 5. Migration Path Detection

Warn about breaking changes between versions.

```bash
# Check for breaking changes before upgrade
jarvy upgrade --check

# Output:
# Checking for breaking changes...
#
# node 20.11.0 -> 21.0.0
#   ⚠ BREAKING: Node.js 21 is current release, not LTS
#   ⚠ API: http.Agent has new default behavior
#   ⚠ API: fs.promises.readdir() changed
#   📖 Migration guide: https://nodejs.org/en/blog/release/v21.0.0
#
# docker 24.0.7 -> 25.0.0
#   ⚠ BREAKING: Deprecated --link flag removed
#   ⚠ CONFIG: daemon.json format changes
#   📖 Migration guide: https://docs.docker.com/engine/release-notes/25.0/
#
# ripgrep 14.0.0 -> 14.1.0
#   ✓ No breaking changes detected
#
# Summary:
#   2 tools have breaking changes
#   1 tool has safe update
#
# ? Proceed with upgrades? (Y/n)

# Show migration info for specific upgrade
jarvy migrate node --from 18 --to 21

# Output:
# Migration Path: node 18.x -> 21.x
# =================================
#
# Major version jumps: 18 -> 19 -> 20 -> 21
#
# node 18 -> 19:
#   ⚠ Experimental: native fetch API
#   ⚠ Deprecated: url.parse() (use URL constructor)
#
# node 19 -> 20:
#   ⚠ BREAKING: Permission model changes
#   ⚠ Minimum: macOS 10.15+ required
#
# node 20 -> 21:
#   ⚠ BREAKING: Not LTS (support ends 2024-04)
#   ⚠ V8: Updated to 11.8
#
# Recommendations:
#   • Consider staying on node 20 LTS for stability
#   • Test thoroughly before production deployment
#   • Review deprecation warnings in your code
#
# Resources:
#   • https://nodejs.org/en/about/releases/
#   • https://github.com/nodejs/node/blob/main/CHANGELOG.md
```

**Migration features:**
- Breaking change detection
- Multi-version migration paths
- Links to official docs
- Recommendations
- Risk assessment

### 6. Version Comparison

Compare versions across environments.

```bash
# Compare with lock file
jarvy diff --lock

# Output:
# Comparing installed vs jarvy.lock
# =================================
#
# Tool        Installed   Locked      Status
# ───────────────────────────────────────────
# git         2.43.0      2.43.0      ✓ Match
# node        21.0.0      20.11.0     ✗ Newer
# docker      25.0.0      24.0.7      ✗ Newer
# ripgrep     14.1.0      14.0.0      ✗ Newer
# ...
#
# Summary:
#   Matching: 4
#   Drifted: 4

# Compare with remote environment
jarvy diff --remote https://company.com/team-versions.json

# Compare two lock files
jarvy diff --locks jarvy.lock jarvy.lock.backup

# Compare with teammate
jarvy diff --export > my-versions.json
# Share my-versions.json with teammate
jarvy diff --compare their-versions.json

# Output:
# Environment Comparison
# ======================
#
# You vs them:
#
# Tool        You         Them        Difference
# ─────────────────────────────────────────────────
# node        21.0.0      20.11.0     You newer
# docker      24.0.7      25.0.0      They newer
# rust        1.75.0      1.75.0      ✓ Same
# python      -           3.12.0      They have
# go          1.21.0      -           You have
```

**Comparison features:**
- Lock file comparison
- Remote environment comparison
- Export/share versions
- Clear difference display
- Recommendations

## Acceptance Criteria

### Rollback Command
- [ ] `jarvy rollback <tool>` reverts to previous version
- [ ] `--to <version>` allows specific version rollback
- [ ] `--history` shows version history
- [ ] `--all` rolls back all tools
- [ ] `--dry-run` previews changes
- [ ] Bundled dependencies handled
- [ ] Hooks re-run if needed

### Lock Files
- [ ] `jarvy lock` generates jarvy.lock
- [ ] `jarvy setup --locked` uses lock file
- [ ] `jarvy lock update` refreshes lock
- [ ] `jarvy lock status` shows drift
- [ ] `jarvy lock verify` checks integrity
- [ ] Platform-specific sections
- [ ] Checksum verification

### Environment Snapshots
- [ ] `jarvy snapshot create` captures state
- [ ] `jarvy snapshot list` shows snapshots
- [ ] `jarvy snapshot show` displays details
- [ ] `jarvy snapshot restore` restores state
- [ ] `jarvy snapshot delete` removes snapshots
- [ ] `jarvy snapshot diff` compares snapshots
- [ ] Automatic backup before restore

### Version History
- [ ] `jarvy history <tool>` shows history
- [ ] `jarvy history --all` shows all tools
- [ ] `jarvy history --timeline` shows timeline
- [ ] `jarvy history --export` exports JSON
- [ ] `jarvy history prune` cleans old entries
- [ ] History persists across sessions

### Migration Detection
- [ ] `jarvy upgrade --check` shows breaking changes
- [ ] `jarvy migrate` shows migration paths
- [ ] Links to official documentation
- [ ] Risk assessment included
- [ ] Recommendations provided

### Version Comparison
- [ ] `jarvy diff --lock` compares to lock file
- [ ] `jarvy diff --export` creates shareable file
- [ ] `jarvy diff --compare` compares files
- [ ] Clear difference display
- [ ] Actionable output

## Technical Approach

### Module Structure

```
src/
  version/
    mod.rs              # Version management
    rollback.rs         # Rollback implementation
    lock.rs             # Lock file management
    snapshot.rs         # Snapshot management
    history.rs          # Version history
    migration.rs        # Migration detection
    diff.rs             # Version comparison
  data/
    migrations/         # Migration data files
      node.toml
      docker.toml
      ...
```

### Version History Storage

```rust
// src/version/history.rs
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct VersionHistory {
    pub tool: String,
    pub entries: Vec<HistoryEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub version: String,
    pub timestamp: DateTime<Utc>,
    pub method: InstallMethod,
    pub previous: Option<String>,
    pub source: String,
    pub notes: Option<String>,
}

pub struct HistoryManager {
    storage_path: PathBuf,
}

impl HistoryManager {
    pub fn record_install(&self, tool: &str, version: &str, method: InstallMethod) -> Result<(), Error> {
        let mut history = self.load_history(tool)?;

        let previous = history.entries.last().map(|e| e.version.clone());

        history.entries.push(HistoryEntry {
            version: version.to_string(),
            timestamp: Utc::now(),
            method,
            previous,
            source: "jarvy".to_string(),
            notes: None,
        });

        self.save_history(tool, &history)
    }

    pub fn get_previous_version(&self, tool: &str) -> Result<Option<String>, Error> {
        let history = self.load_history(tool)?;
        Ok(history.entries
            .iter()
            .rev()
            .nth(1)
            .map(|e| e.version.clone()))
    }
}
```

### Lock File Management

```rust
// src/version/lock.rs
use sha2::{Sha256, Digest};

#[derive(Debug, Serialize, Deserialize)]
pub struct LockFile {
    pub metadata: LockMetadata,
    pub platforms: HashMap<String, PlatformLock>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LockMetadata {
    pub version: u32,
    pub generated: DateTime<Utc>,
    pub jarvy_version: String,
    pub config_hash: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PlatformLock {
    pub tools: HashMap<String, ToolLock>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolLock {
    pub version: String,
    pub source: String,
    pub checksum: String,
    pub installed: DateTime<Utc>,
    pub dependencies: Option<Vec<String>>,
}

pub fn generate_lock(config: &Config) -> Result<LockFile, Error> {
    let platform = get_current_platform();
    let mut tools = HashMap::new();

    for (name, spec) in &config.tools {
        let installed = get_installed_version(name)?;
        let checksum = compute_checksum(name, &installed)?;

        tools.insert(name.clone(), ToolLock {
            version: installed.version,
            source: installed.source,
            checksum,
            installed: Utc::now(),
            dependencies: installed.dependencies,
        });
    }

    Ok(LockFile {
        metadata: LockMetadata {
            version: 1,
            generated: Utc::now(),
            jarvy_version: env!("CARGO_PKG_VERSION").to_string(),
            config_hash: hash_config(config)?,
        },
        platforms: {
            let mut p = HashMap::new();
            p.insert(platform, PlatformLock { tools });
            p
        },
    })
}
```

### Snapshot Management

```rust
// src/version/snapshot.rs
#[derive(Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub id: String,
    pub name: Option<String>,
    pub created: DateTime<Utc>,
    pub platform: String,
    pub tools: HashMap<String, ToolSnapshot>,
    pub config: Config,
    pub hooks: Vec<HookState>,
    pub env_vars: HashMap<String, String>,
}

pub struct SnapshotManager {
    storage_dir: PathBuf,
}

impl SnapshotManager {
    pub fn create(&self, name: Option<String>) -> Result<Snapshot, Error> {
        let id = format!("snapshot-{}", Utc::now().format("%Y%m%d-%H%M%S"));

        let snapshot = Snapshot {
            id: id.clone(),
            name,
            created: Utc::now(),
            platform: get_current_platform(),
            tools: self.capture_tools()?,
            config: self.capture_config()?,
            hooks: self.capture_hooks()?,
            env_vars: self.capture_env_vars()?,
        };

        self.save_snapshot(&snapshot)?;
        Ok(snapshot)
    }

    pub fn restore(&self, id: &str) -> Result<(), Error> {
        // Create backup first
        self.create(Some("pre-restore".to_string()))?;

        let snapshot = self.load_snapshot(id)?;

        for (name, tool_snapshot) in &snapshot.tools {
            let current = get_installed_version(name)?;
            if current.version != tool_snapshot.version {
                rollback_tool(name, &tool_snapshot.version)?;
            }
        }

        Ok(())
    }
}
```

## Implementation Steps

1. Create version module structure
2. Implement version history tracking
3. Add history persistence
4. Implement rollback command
5. Build lock file generation
6. Implement lock verification
7. Create snapshot system
8. Add snapshot comparison
9. Implement migration detection
10. Build migration data files
11. Implement version comparison
12. Write unit tests
13. Write integration tests
14. Update documentation

## Dependencies

- `sha2` - Checksum computation
- `chrono` - Timestamp handling (existing via other deps)
- No new major dependencies

## Effort Estimate

| Task | Effort |
|------|--------|
| Version module structure | 0.5 days |
| Version history tracking | 2 days |
| History persistence | 1 day |
| Rollback command | 2.5 days |
| Lock file generation | 2 days |
| Lock verification | 1 day |
| Snapshot system | 2.5 days |
| Snapshot comparison | 1 day |
| Migration detection | 2 days |
| Migration data files | 1.5 days |
| Version comparison | 1.5 days |
| Testing | 3 days |
| Documentation | 1 day |
| **Total** | **21.5 days** |

## Files to Create/Modify

### New Files
- `src/version/mod.rs`
- `src/version/rollback.rs`
- `src/version/lock.rs`
- `src/version/snapshot.rs`
- `src/version/history.rs`
- `src/version/migration.rs`
- `src/version/diff.rs`
- `data/migrations/*.toml`
- `tests/rollback_integration.rs`
- `tests/lock_integration.rs`
- `tests/snapshot_integration.rs`

### Modified Files
- `src/main.rs` - Add version management commands
- `src/commands/mod.rs` - Export new commands
- `src/commands/setup.rs` - Add `--locked` flag
- `Cargo.toml` - Add sha2
- `CLAUDE.md` - Document version features

## Success Metrics

| Metric | Current | Target |
|--------|---------|--------|
| Rollback capability | None | Full |
| Version reproducibility | None | Lock files |
| Environment snapshots | None | Full capture |
| Version history | None | Tracked |
| Breaking change warnings | None | Automated |
| Environment comparison | None | Full diff |

## Risks

1. **Storage growth**: History and snapshots use disk space
   - Mitigation: Pruning, size limits

2. **Rollback failures**: Rollback might fail mid-operation
   - Mitigation: Atomic operations, recovery mode

3. **Lock file conflicts**: Merging lock files in version control
   - Mitigation: Clear format, tooling support

4. **Migration data maintenance**: Breaking changes data needs updates
   - Mitigation: Community contributions, automated detection

5. **Cross-platform snapshots**: Snapshots not portable across platforms
   - Mitigation: Clear labeling, platform sections
