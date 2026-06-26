# PRD-027: Observability & Debugging

## Overview

Add comprehensive observability and debugging features that help users understand what Jarvy is doing, diagnose failures, and optimize performance, including structured logging, performance profiling, and diagnostic tools.

## Problem Statement

When things go wrong, users struggle to understand why:
- No detailed logging beyond basic output
- Difficult to debug "why did this fail?"
- No visibility into what's taking time
- Network requests are opaque
- Support requests hard to handle without context
- CI failures are difficult to reproduce and diagnose

Debugging developer tool issues requires visibility into operations that Jarvy currently doesn't provide.

## Evidence

- Support questions: "Why isn't X working?"
- CI/CD failures with no actionable error context
- Users can't tell if slowness is network or package manager
- No way to capture diagnostic info for bug reports
- Operations teams need visibility for troubleshooting

## Requirements

### Functional Requirements

1. **Verbose debug mode**: Structured, detailed logging
2. **Performance profiling**: Timeline of operations
3. **Network tracing**: Track all network requests
4. **Diagnostic command**: `jarvy diagnose <tool>`
5. **Health dashboard**: Enhanced `jarvy doctor` with metrics
6. **Log export**: Generate shareable diagnostic bundles

### Non-Functional Requirements

1. Debug mode adds < 10% overhead
2. Logs are machine-parseable (JSON option)
3. Sensitive data (tokens, passwords) redacted
4. Works in CI environments
5. Integrates with existing logging infrastructure

## Non-Goals

- Real-time monitoring dashboard
- Metrics aggregation service
- APM integration (future PRD)
- Distributed tracing
- Alerting system

## Feature Specifications

### 1. Verbose Debug Mode

Structured, detailed logging for all operations.

```bash
# Enable debug mode
jarvy setup --debug

# Output:
# [2024-01-15T10:30:00.123Z] DEBUG jarvy::config Reading config from ./jarvy.toml
# [2024-01-15T10:30:00.125Z] DEBUG jarvy::config Parsed 8 tools from config
# [2024-01-15T10:30:00.126Z] DEBUG jarvy::tools Checking installed tools
# [2024-01-15T10:30:00.127Z] DEBUG jarvy::tools::git Running: git --version
# [2024-01-15T10:30:00.145Z] DEBUG jarvy::tools::git Output: git version 2.43.0
# [2024-01-15T10:30:00.145Z] DEBUG jarvy::tools::git Parsed version: 2.43.0
# [2024-01-15T10:30:00.146Z] INFO  jarvy::tools git 2.43.0 - already installed
# [2024-01-15T10:30:00.147Z] DEBUG jarvy::tools::node Running: node --version
# [2024-01-15T10:30:00.189Z] DEBUG jarvy::tools::node Output: v20.10.0
# [2024-01-15T10:30:00.189Z] DEBUG jarvy::tools::node Need: 20.11.0, Have: 20.10.0
# [2024-01-15T10:30:00.190Z] INFO  jarvy::tools node needs update: 20.10.0 -> 20.11.0
# ...

# Even more verbose (trace level)
jarvy setup --trace

# Debug with JSON output (for parsing)
jarvy setup --debug --log-format json

# Output:
# {"timestamp":"2024-01-15T10:30:00.123Z","level":"DEBUG","target":"jarvy::config","message":"Reading config from ./jarvy.toml"}
# {"timestamp":"2024-01-15T10:30:00.125Z","level":"DEBUG","target":"jarvy::config","message":"Parsed 8 tools","tools":["git","node","docker","jq","ripgrep","fd","bat","starship"]}

# Debug specific subsystem
jarvy setup --debug-filter "jarvy::tools::docker"

# Write logs to file
jarvy setup --debug --log-file jarvy.log

# Combine with normal output
jarvy setup --debug 2> debug.log
```

**Debug levels:**
- `--quiet`: Errors only
- (default): Info and above
- `--verbose` / `-v`: Includes warnings
- `--debug` / `-vv`: Full debug logs
- `--trace` / `-vvv`: Trace-level detail

### 2. Performance Profiling

Track timing of all operations.

```bash
# Run with performance profiling
jarvy setup --profile

# Output at end:
# ══════════════════════════════════════════════════════════
# Performance Profile
# ══════════════════════════════════════════════════════════
#
# Total duration: 45.23s
#
# Phase breakdown:
#   Config parsing:      0.02s  (0.0%)
#   Tool detection:      1.24s  (2.7%)
#   Package downloads:  28.67s (63.4%)
#   Installation:       14.89s (32.9%)
#   Hooks:               0.41s  (0.9%)
#
# Tool installation times:
#   docker    25.34s  ████████████████████████████████████
#   node       8.12s  ███████████
#   rust       5.67s  ████████
#   git        2.34s  ███
#   jq         0.89s  █
#   ripgrep    0.78s  █
#   fd         0.56s  █
#   starship   1.53s  ██
#
# Network requests:
#   Total requests: 12
#   Total downloaded: 892.4 MB
#   Slowest: https://desktop.docker.com/mac/main/arm64/Docker.dmg (520.1 MB, 23.4s)
#
# Recommendations:
#   ⚠ Docker download took 52% of total time
#     Consider using local mirror or pre-cached bundle

# Export profile as JSON
jarvy setup --profile --profile-output profile.json

# View profile from previous run
jarvy profile show profile.json

# Compare two profiles
jarvy profile compare before.json after.json
```

**Profile data includes:**
- Phase timing (parse, detect, download, install, hooks)
- Per-tool timing breakdown
- Network request details
- Memory usage (peak)
- Recommendations for optimization

### 3. Network Tracing

Track all network requests made by Jarvy.

```bash
# Enable network tracing
jarvy setup --trace-network

# Output:
# Network Trace
# =============
#
# [10:30:01.234] GET https://formulae.brew.sh/api/formula/git.json
#   Status: 200 OK
#   Duration: 145ms
#   Size: 4.2 KB
#
# [10:30:01.456] GET https://formulae.brew.sh/api/cask/docker.json
#   Status: 200 OK
#   Duration: 132ms
#   Size: 3.8 KB
#
# [10:30:02.012] GET https://desktop.docker.com/mac/main/arm64/Docker.dmg
#   Status: 200 OK
#   Duration: 23,456ms
#   Size: 520.1 MB
#   Speed: 22.2 MB/s
#
# Summary:
#   Total requests: 12
#   Successful: 12
#   Failed: 0
#   Total data: 892.4 MB
#   Total time: 28.67s

# Save network trace
jarvy setup --trace-network --network-log network.json

# Replay/analyze network log
jarvy network analyze network.json

# Output:
# Network Analysis
# ================
#
# Domains contacted:
#   formulae.brew.sh (5 requests, 23.4 KB)
#   github.com (3 requests, 12.1 KB)
#   desktop.docker.com (1 request, 520.1 MB)
#   nodejs.org (1 request, 45.8 MB)
#   static.rust-lang.org (2 requests, 312.4 MB)
#
# Potential issues:
#   None detected
#
# Optimization suggestions:
#   • docker.dmg: Consider local mirror (520.1 MB)
#   • rust: Consider rustup cache warming
```

**Network trace includes:**
- All HTTP(S) requests
- Request/response headers (sensitive redacted)
- Timing breakdown (DNS, connect, TLS, transfer)
- Response status and size
- Redirect chains

### 4. Diagnostic Command

Deep diagnosis for specific tools.

```bash
# Diagnose a specific tool
jarvy diagnose docker

# Output:
# Diagnosing: docker
# ==================
#
# Installation Status
# -------------------
# Installed: Yes
# Version: 24.0.7
# Location: /usr/local/bin/docker
# Install method: homebrew-cask
#
# Binary Analysis
# ---------------
# File type: Mach-O 64-bit executable arm64
# Permissions: -rwxr-xr-x
# Owner: root:wheel
# Symlink: /usr/local/bin/docker -> /Applications/Docker.app/Contents/Resources/bin/docker
#
# Dependencies
# ------------
# Docker.app: /Applications/Docker.app (installed)
# Docker daemon: Running (pid 1234)
# Docker socket: /var/run/docker.sock (accessible)
#
# Configuration
# -------------
# Config file: ~/.docker/config.json
# Data directory: ~/Library/Containers/com.docker.docker/Data
# Disk usage: 45.2 GB
#
# Connectivity
# ------------
# Registry: https://registry-1.docker.io (accessible)
# Hub login: Authenticated as user@example.com
#
# Health Checks
# -------------
# ✓ docker version - responds correctly
# ✓ docker info - daemon accessible
# ✓ docker ps - no permission errors
# ✓ docker pull hello-world - registry accessible
#
# Recent Issues
# -------------
# No issues detected
#
# Recommendation
# --------------
# Docker is healthy and fully functional.

# Diagnose with suggested fixes
jarvy diagnose node --fix

# Output:
# Diagnosing: node
# ================
#
# Installation Status
# -------------------
# Installed: Yes
# Version: 20.10.0
# Location: ~/.nvm/versions/node/v20.10.0/bin/node
# Install method: nvm
#
# Issues Found
# ------------
# ⚠ PATH issue: nvm node not in current PATH
#   Expected: ~/.nvm/versions/node/v20.10.0/bin
#   Current PATH does not include this directory
#
# ✗ Shell integration missing
#   nvm.sh not sourced in current shell
#
# Suggested Fixes
# ---------------
# 1. Add to ~/.zshrc:
#    export NVM_DIR="$HOME/.nvm"
#    [ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"
#
# 2. Restart shell or run:
#    source ~/.zshrc
#
# ? Apply fix automatically? (Y/n)
```

**Diagnose checks:**
- Installation status and location
- Binary analysis (type, permissions)
- Dependencies and runtime requirements
- Configuration files
- Network connectivity
- Recent logs/errors
- Suggested fixes

### 5. Enhanced Health Dashboard

Extended `jarvy doctor` with metrics and trends.

```bash
# Enhanced health check
jarvy doctor --extended

# Output:
# ╔═══════════════════════════════════════════════════════════════╗
# ║                    Jarvy Health Dashboard                      ║
# ╚═══════════════════════════════════════════════════════════════╝
#
# System Overview
# ───────────────
# OS: macOS 14.2 (darwin-arm64)
# Shell: zsh 5.9
# Uptime: 5 days, 12 hours
# Load: 1.23, 1.45, 1.67
# Memory: 12.4 GB / 16.0 GB (77.5%)
# Disk: 234.5 GB / 500.0 GB (46.9%)
#
# Package Managers
# ────────────────
# homebrew    4.2.0   ✓ Healthy   (234 packages)
# nvm         0.39.7  ✓ Healthy   (3 node versions)
# rustup      1.26.0  ✓ Healthy   (2 toolchains)
# pip         24.0    ✓ Healthy
#
# Tool Status (12 tools)
# ──────────────────────
# ✓ Healthy:  10
# ⚠ Outdated:  1 (rust 1.70 -> 1.75 available)
# ✗ Missing:   1 (kubectl)
#
# Tool Versions
# ─────────────
# git       2.43.0   ✓ Latest
# node      20.11.0  ✓ Latest
# rust      1.70.0   ⚠ Update: 1.75.0
# docker    24.0.7   ✓ Latest
# jq        1.7.1    ✓ Latest
# kubectl   -        ✗ Not installed
# ...
#
# Performance Metrics
# ───────────────────
# Last setup:     45.2s (2024-01-14)
# Avg setup time: 52.3s (last 5 runs)
# Cache size:     156.3 MB
# Cache hit rate: 78.4%
#
# Recommendations
# ───────────────
# 1. Update rust: jarvy upgrade rust
# 2. Install kubectl: jarvy setup --only kubectl
# 3. Clear old cache: jarvy cache clean (save 45.2 MB)
#
# Detailed report: jarvy doctor --report > health-report.md

# Generate health report
jarvy doctor --report

# Output saved to: jarvy-health-report-20240115.md

# Check specific categories
jarvy doctor --check tools
jarvy doctor --check network
jarvy doctor --check performance
```

**Health dashboard features:**
- System overview
- Package manager status
- Tool status summary
- Version comparison
- Performance metrics history
- Actionable recommendations

### 6. Diagnostic Bundle Export

Generate shareable diagnostic information for support.

```bash
# Create diagnostic bundle
jarvy diagnose --export

# Output:
# Creating diagnostic bundle...
#
# Collecting:
#   ✓ System information
#   ✓ Jarvy configuration
#   ✓ Tool status
#   ✓ Recent logs (last 1000 lines)
#   ✓ Environment variables (sanitized)
#   ✓ Network connectivity
#   ✓ Performance metrics
#
# Sanitizing sensitive data...
#   ✓ Tokens/passwords redacted
#   ✓ Personal paths anonymized
#   ✓ Email addresses masked
#
# ✓ Created: jarvy-diagnostic-20240115-103045.zip
#   Size: 245 KB
#
# Contents:
#   system-info.json
#   jarvy-config.json (sanitized)
#   tool-status.json
#   recent-logs.txt
#   environment.txt (sanitized)
#   network-test.json
#   performance.json
#
# Share this file when reporting issues:
#   https://github.com/Cliftonz/jarvy/issues
#
# ⚠ Review contents before sharing:
#   unzip -l jarvy-diagnostic-20240115-103045.zip

# Export with specific scope
jarvy diagnose --export --scope tools,network

# Include last N operations
jarvy diagnose --export --include-history 10

# Export for specific tool issue
jarvy diagnose docker --export
```

**Bundle contents:**
- System information (OS, arch, shell)
- Jarvy configuration (sanitized)
- Tool installation status
- Recent operation logs
- Environment variables (sanitized)
- Network connectivity tests
- Performance metrics
- Error history

## Acceptance Criteria

### Verbose Debug Mode
- [ ] `--debug` flag enables debug logging
- [ ] `--trace` flag enables trace logging
- [ ] `--log-format json` outputs JSON logs
- [ ] `--debug-filter` limits to specific modules
- [ ] `--log-file` writes to specified file
- [ ] Debug mode works with all commands
- [ ] Sensitive data is redacted

### Performance Profiling
- [ ] `--profile` flag enables profiling
- [ ] Phase breakdown shown at end
- [ ] Per-tool timing displayed
- [ ] Network timing included
- [ ] `--profile-output` saves JSON
- [ ] Profile comparison supported
- [ ] Recommendations generated

### Network Tracing
- [ ] `--trace-network` shows all requests
- [ ] Request/response details captured
- [ ] Timing breakdown included
- [ ] `--network-log` saves trace
- [ ] `jarvy network analyze` works
- [ ] Sensitive headers redacted
- [ ] Summary statistics displayed

### Diagnostic Command
- [ ] `jarvy diagnose <tool>` works for all tools
- [ ] Installation status checked
- [ ] Binary analysis performed
- [ ] Dependencies verified
- [ ] Configuration examined
- [ ] Health checks run
- [ ] `--fix` offers remediation
- [ ] Useful for troubleshooting

### Enhanced Health Dashboard
- [ ] `jarvy doctor --extended` shows full dashboard
- [ ] System overview included
- [ ] Package manager status shown
- [ ] Performance metrics tracked
- [ ] History trends available
- [ ] `--report` generates markdown
- [ ] Category filtering works

### Diagnostic Bundle Export
- [ ] `jarvy diagnose --export` creates bundle
- [ ] Sensitive data sanitized
- [ ] Bundle is complete but minimal
- [ ] Contents documented
- [ ] Scope filtering works
- [ ] Tool-specific export works
- [ ] Instructions for sharing included

## Technical Approach

### Module Structure

```
src/
  observability/
    mod.rs              # Observability coordination
    logging.rs          # Debug logging setup
    profiler.rs         # Performance profiling
    network_trace.rs    # Network tracing
    sanitizer.rs        # Sensitive data redaction
  commands/
    diagnose.rs         # jarvy diagnose command
  doctor/
    dashboard.rs        # Enhanced health dashboard
    metrics.rs          # Performance metrics
```

### Structured Logging

```rust
// src/observability/logging.rs
use tracing::{Level, span};
use tracing_subscriber::{fmt, EnvFilter};

pub fn init_logging(config: &LogConfig) -> Result<(), Error> {
    let filter = match config.level {
        LogLevel::Quiet => EnvFilter::new("error"),
        LogLevel::Normal => EnvFilter::new("info"),
        LogLevel::Verbose => EnvFilter::new("warn,jarvy=info"),
        LogLevel::Debug => EnvFilter::new("debug"),
        LogLevel::Trace => EnvFilter::new("trace"),
    };

    let subscriber = fmt::Subscriber::builder()
        .with_env_filter(filter)
        .with_timer(fmt::time::UtcTime::rfc_3339());

    let subscriber = if config.json {
        subscriber.json().finish()
    } else {
        subscriber.finish()
    };

    tracing::subscriber::set_global_default(subscriber)?;
    Ok(())
}

// Usage in code
pub fn install_tool(tool: &Tool) -> Result<(), Error> {
    let span = span!(Level::INFO, "install_tool", name = %tool.name);
    let _guard = span.enter();

    tracing::debug!("Starting installation");
    tracing::debug!(method = %tool.install_method, "Using install method");

    // ... installation code

    tracing::info!(version = %installed_version, "Installation complete");
    Ok(())
}
```

### Performance Profiler

```rust
// src/observability/profiler.rs
use std::time::{Duration, Instant};
use std::collections::HashMap;

pub struct Profiler {
    start: Instant,
    phases: HashMap<String, PhaseTiming>,
    current_phase: Option<String>,
    network_requests: Vec<NetworkTiming>,
}

impl Profiler {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
            phases: HashMap::new(),
            current_phase: None,
            network_requests: Vec::new(),
        }
    }

    pub fn start_phase(&mut self, name: &str) {
        if let Some(current) = &self.current_phase {
            self.end_phase();
        }
        self.current_phase = Some(name.to_string());
        self.phases.insert(name.to_string(), PhaseTiming {
            start: Instant::now(),
            duration: Duration::ZERO,
        });
    }

    pub fn end_phase(&mut self) {
        if let Some(name) = self.current_phase.take() {
            if let Some(phase) = self.phases.get_mut(&name) {
                phase.duration = phase.start.elapsed();
            }
        }
    }

    pub fn record_network(&mut self, timing: NetworkTiming) {
        self.network_requests.push(timing);
    }

    pub fn report(&self) -> ProfileReport {
        let total_duration = self.start.elapsed();

        ProfileReport {
            total_duration,
            phases: self.phases.clone(),
            network: NetworkSummary {
                total_requests: self.network_requests.len(),
                total_bytes: self.network_requests.iter().map(|r| r.bytes).sum(),
                total_time: self.network_requests.iter().map(|r| r.duration).sum(),
            },
            recommendations: self.generate_recommendations(),
        }
    }
}
```

### Data Sanitizer

```rust
// src/observability/sanitizer.rs
use regex::Regex;

pub struct Sanitizer {
    patterns: Vec<(Regex, &'static str)>,
}

impl Sanitizer {
    pub fn new() -> Self {
        Self {
            patterns: vec![
                // API keys and tokens
                (Regex::new(r"(?i)(api[_-]?key|token|secret|password|auth)[=:]\s*['\"]?[\w-]+['\"]?").unwrap(), "$1=[REDACTED]"),
                // Bearer tokens
                (Regex::new(r"Bearer\s+[\w-]+").unwrap(), "Bearer [REDACTED]"),
                // Email addresses
                (Regex::new(r"[\w.+-]+@[\w.-]+\.\w+").unwrap(), "[EMAIL]"),
                // Home directory paths
                (Regex::new(&format!(r"{}", dirs::home_dir().unwrap().display())).unwrap(), "~"),
            ],
        }
    }

    pub fn sanitize(&self, input: &str) -> String {
        let mut result = input.to_string();
        for (pattern, replacement) in &self.patterns {
            result = pattern.replace_all(&result, *replacement).to_string();
        }
        result
    }
}
```

## Implementation Steps

1. Create observability module structure
2. Implement structured logging with tracing
3. Add log level configuration
4. Implement performance profiler
5. Add network tracing middleware
6. Implement data sanitizer
7. Build `jarvy diagnose` command
8. Enhance `jarvy doctor` dashboard
9. Add metrics collection
10. Implement bundle export
11. Add profile comparison
12. Write unit tests
13. Write integration tests
14. Update documentation

## Dependencies

- `tracing` - Structured logging (existing)
- `tracing-subscriber` - Log formatting (existing)
- `zip` - Bundle creation
- No new major dependencies

## Effort Estimate

| Task | Effort |
|------|--------|
| Observability module structure | 0.5 days |
| Structured logging setup | 1.5 days |
| Log level configuration | 0.5 days |
| Performance profiler | 2 days |
| Network tracing | 2 days |
| Data sanitizer | 1 day |
| `jarvy diagnose` command | 2.5 days |
| Enhanced `jarvy doctor` | 2 days |
| Metrics collection | 1 day |
| Bundle export | 1.5 days |
| Profile comparison | 1 day |
| Testing | 2.5 days |
| Documentation | 1 day |
| **Total** | **19 days** |

## Files to Create/Modify

### New Files
- `src/observability/mod.rs`
- `src/observability/logging.rs`
- `src/observability/profiler.rs`
- `src/observability/network_trace.rs`
- `src/observability/sanitizer.rs`
- `src/commands/diagnose.rs`
- `src/doctor/dashboard.rs`
- `src/doctor/metrics.rs`
- `tests/observability_integration.rs`
- `tests/diagnose_integration.rs`

### Modified Files
- `src/main.rs` - Add debug flags, diagnose command
- `src/commands/doctor.rs` - Enhanced dashboard
- `Cargo.toml` - Add zip
- `CLAUDE.md` - Document debugging features

## Success Metrics

| Metric | Current | Target |
|--------|---------|--------|
| Debug logging | Basic | Structured |
| Performance visibility | None | Full profile |
| Network transparency | Opaque | Traced |
| Tool diagnosis | Manual | Automated |
| Support efficiency | Low | High |
| Bug report quality | Poor | Complete |

## Risks

1. **Log volume**: Debug mode could generate excessive logs
   - Mitigation: Level filtering, log rotation

2. **Performance overhead**: Profiling could slow operations
   - Mitigation: Optional, minimal overhead design

3. **Sensitive data exposure**: Logs might leak secrets
   - Mitigation: Comprehensive sanitization

4. **Storage requirements**: Logs and metrics use disk space
   - Mitigation: Rotation, size limits, cleanup

5. **Complexity**: Too much information can be confusing
   - Mitigation: Clear formatting, summaries first
