# PRD-050: Debug Ticket & Enhanced Logging System

## Overview

Add a persistent file-based logging system with automatic rotation in the `.jarvy` folder, along with a debug ticket command that collects comprehensive diagnostic data for troubleshooting support issues.

## Problem Statement

When users encounter issues with Jarvy, troubleshooting is difficult because:

- Logs are only displayed to stdout/stderr and not persisted
- Previous command outputs are lost when the terminal is cleared
- Users can't easily share diagnostic information with support
- No historical record of what commands were run and their outcomes
- Manual collection of system info is tedious and often incomplete
- Different users provide inconsistent diagnostic information

Effective support requires:
1. Persistent logs that survive terminal sessions
2. A standardized way to collect and share diagnostic data
3. Automatic cleanup to prevent disk space issues

## Evidence

- Support interactions require back-and-forth to gather context
- Users often say "it worked yesterday" with no way to see what changed
- GitHub issues lack sufficient diagnostic information
- Reproducing user issues is difficult without environment details
- No audit trail for compliance or debugging

## Requirements

### Functional Requirements

1. **File-based logging**: Write all Jarvy operations to log files in `~/.jarvy/logs/`
2. **Log rotation**: Automatically rotate logs by size and age
3. **Log levels**: Configurable verbosity levels (error, warn, info, debug, trace)
4. **Debug ticket**: Command to generate a comprehensive diagnostic bundle
5. **Privacy**: Automatic redaction of sensitive information
6. **Cleanup**: Automatic purging of old logs

### Non-Functional Requirements

1. **Performance**: Logging adds <5% overhead to operations
2. **Storage**: Respect configurable disk space limits (default 50MB)
3. **Portability**: Cross-platform log paths and formats
4. **Parseable**: Structured JSON logs for tooling integration
5. **Offline**: All operations work without network access

## Non-Goals

- Real-time log streaming to external services
- Log aggregation across multiple machines
- Automatic ticket submission to support systems
- Alert/notification based on log patterns
- Log encryption at rest

## Feature Specifications

### 1. Logging Configuration

```toml
# ~/.jarvy/config.toml (global)
[logging]
# Enable file-based logging
enabled = true

# Log level: error, warn, info, debug, trace
level = "info"

# Log directory (default: ~/.jarvy/logs/)
directory = "~/.jarvy/logs"

# Log format: text, json
format = "json"

# Maximum size per log file before rotation (default: 10MB)
max_file_size = "10MB"

# Maximum number of rotated files to keep
max_files = 5

# Maximum total log storage (default: 50MB)
max_total_size = "50MB"

# Maximum age of log files (default: 30 days)
max_age_days = 30
```

### 2. Log File Format

```
~/.jarvy/
├── config.toml
├── logs/
│   ├── jarvy.log           # Current log file
│   ├── jarvy.1.log.gz      # Rotated (compressed)
│   ├── jarvy.2.log.gz
│   ├── jarvy.3.log.gz
│   └── jarvy.4.log.gz
└── state.json
```

**Log entry format (JSON):**
```json
{
  "timestamp": "2026-01-26T10:30:00.123Z",
  "level": "INFO",
  "target": "jarvy::commands::setup",
  "message": "Installing tool",
  "fields": {
    "tool": "git",
    "version": "2.43.0",
    "method": "brew"
  },
  "span": {
    "name": "setup",
    "id": "abc123"
  }
}
```

**Log entry format (text):**
```
2026-01-26T10:30:00.123Z INFO  [jarvy::commands::setup] Installing tool tool=git version=2.43.0 method=brew
```

### 3. CLI Commands

```bash
# View recent logs
jarvy logs
# Output shows last 50 log entries

# View more entries
jarvy logs --lines 200

# Follow logs in real-time
jarvy logs --follow

# Filter by level
jarvy logs --level error

# Filter by time range
jarvy logs --since "1 hour ago"
jarvy logs --since "2026-01-25" --until "2026-01-26"

# Search logs
jarvy logs --grep "docker"
jarvy logs --grep "error" --level warn

# View log stats
jarvy logs stats
# Output:
# Log Statistics
# ==============
# Total files: 5
# Total size: 23.4 MB
# Oldest entry: 2026-01-10
# Newest entry: 2026-01-26
# Entries by level:
#   ERROR: 12
#   WARN:  45
#   INFO:  1,234
#   DEBUG: 0

# Clean up old logs
jarvy logs clean
# Output:
# Cleaning logs older than 30 days...
# Removed 2 log files (15.2 MB)

# Clean all logs
jarvy logs clean --all

# Configure logging
jarvy logs config
# Output shows current configuration

jarvy logs config --level debug
jarvy logs config --max-size 100MB
jarvy logs config --disable
jarvy logs config --enable
```

### 4. Debug Ticket Command

```bash
# Generate debug ticket
jarvy ticket

# Output:
# Generating debug ticket...
#
# Collecting:
#   ✓ System information
#   ✓ Jarvy version and configuration
#   ✓ Tool installation status
#   ✓ Recent logs (last 500 entries)
#   ✓ Environment state (.jarvy/state.json)
#   ✓ Drift detection status
#   ✓ Environment variables (sanitized)
#   ✓ Network configuration
#   ✓ Recent command history
#
# Sanitizing sensitive data...
#   ✓ API keys/tokens redacted
#   ✓ Passwords redacted
#   ✓ Personal paths anonymized
#   ✓ Email addresses masked
#
# ✓ Created: ~/.jarvy/tickets/jarvy-ticket-20260126-103045.zip
#   Size: 156 KB
#   Ticket ID: JRV-20260126-A3F2
#
# Contents:
#   - manifest.json (ticket metadata)
#   - system.json (OS, arch, resources)
#   - jarvy-config.json (sanitized)
#   - jarvy-version.json
#   - tools.json (installed tools status)
#   - state.json (environment state)
#   - drift-report.json
#   - logs.txt (recent logs, sanitized)
#   - environment.txt (env vars, sanitized)
#   - network.json (proxy config, sanitized)
#
# To share with support:
#   1. Review contents: unzip -l ~/.jarvy/tickets/jarvy-ticket-20260126-103045.zip
#   2. Attach to GitHub issue: https://github.com/bearbinary/jarvy/issues
#   3. Or email: support@jarvy.dev
#
# Ticket expires: 2026-02-25 (30 days)

# Generate ticket for specific tool issue
jarvy ticket --tool docker

# Include more log history
jarvy ticket --logs 1000

# Include full logs (for severe issues)
jarvy ticket --logs all

# Output to specific location
jarvy ticket --output /path/to/ticket.zip

# Preview what will be collected (no file created)
jarvy ticket --dry-run

# View ticket contents
jarvy ticket show JRV-20260126-A3F2

# List generated tickets
jarvy ticket list
# Output:
# Tickets
# =======
# JRV-20260126-A3F2  156 KB  2026-01-26  (expires: 2026-02-25)
# JRV-20260120-B7C1   89 KB  2026-01-20  (expired)
#
# Total: 2 tickets, 245 KB

# Clean expired tickets
jarvy ticket clean
```

### 5. Ticket Contents

**manifest.json:**
```json
{
  "ticket_id": "JRV-20260126-A3F2",
  "created_at": "2026-01-26T10:30:45Z",
  "expires_at": "2026-02-25T10:30:45Z",
  "jarvy_version": "0.5.0",
  "collector_version": "1",
  "scope": "full",
  "sections": [
    "system",
    "config",
    "tools",
    "logs",
    "environment",
    "network"
  ],
  "sanitization": {
    "applied": true,
    "patterns_matched": 12
  }
}
```

**system.json:**
```json
{
  "os": {
    "name": "macOS",
    "version": "14.3",
    "arch": "arm64",
    "kernel": "Darwin 23.3.0"
  },
  "hardware": {
    "cpu_cores": 10,
    "memory_total_gb": 16,
    "memory_available_gb": 8.2
  },
  "shell": {
    "name": "zsh",
    "version": "5.9",
    "path": "/bin/zsh"
  },
  "locale": {
    "lang": "en_US.UTF-8",
    "timezone": "America/Los_Angeles"
  },
  "disk": {
    "home_available_gb": 234.5,
    "jarvy_dir_size_mb": 45.2
  }
}
```

**tools.json:**
```json
{
  "package_managers": {
    "homebrew": { "installed": true, "version": "4.2.0" },
    "apt": { "installed": false },
    "winget": { "installed": false }
  },
  "configured_tools": {
    "git": {
      "configured_version": "latest",
      "installed_version": "2.43.0",
      "status": "ok",
      "path": "/opt/homebrew/bin/git"
    },
    "node": {
      "configured_version": "20",
      "installed_version": "20.11.0",
      "status": "ok",
      "path": "~/.nvm/versions/node/v20.11.0/bin/node"
    },
    "docker": {
      "configured_version": "latest",
      "installed_version": null,
      "status": "missing",
      "error": "Command not found"
    }
  }
}
```

### 6. Logging Integration Points

```rust
// Logging is automatic for all Jarvy operations

// Example: setup command automatically logs
jarvy setup
// Generates log entries:
// INFO  [setup] Starting environment setup
// INFO  [setup] Loading config from ./jarvy.toml
// DEBUG [setup] Found 8 tools to install
// INFO  [tools::git] Checking installation
// INFO  [tools::git] Already installed: 2.43.0
// INFO  [tools::docker] Installing via brew cask
// DEBUG [tools::docker] Running: brew install --cask docker
// INFO  [tools::docker] Installed: 24.0.7
// INFO  [hooks] Running post-install hooks
// INFO  [setup] Setup complete: 8 tools, 0 failed

// On error, logs capture full context
// ERROR [tools::node] Installation failed
//   error: "brew install node failed with exit code 1"
//   stdout: ""
//   stderr: "Error: node 20.11.0 is already installed"
//   suggestion: "Run 'brew upgrade node' to update"
```

## Technical Approach

### Module Structure

```
src/
  logging/
    mod.rs              # Public API
    config.rs           # LoggingConfig type
    writer.rs           # File writer with rotation
    rotator.rs          # Log rotation logic
    formatter.rs        # JSON and text formatters
    sanitizer.rs        # Sensitive data redaction
  commands/
    logs_cmd.rs         # jarvy logs command
    ticket_cmd.rs       # jarvy ticket command
  ticket/
    mod.rs              # Ticket generation
    collector.rs        # Data collection
    bundler.rs          # ZIP creation
```

### Log Writer Implementation

```rust
// src/logging/writer.rs
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::Mutex;

pub struct RotatingFileWriter {
    config: LoggingConfig,
    current_file: Mutex<Option<BufWriter<File>>>,
    current_size: Mutex<u64>,
    log_dir: PathBuf,
}

impl RotatingFileWriter {
    pub fn new(config: LoggingConfig) -> Result<Self, LogError> {
        let log_dir = config.directory.clone();
        std::fs::create_dir_all(&log_dir)?;

        let writer = Self {
            config,
            current_file: Mutex::new(None),
            current_size: Mutex::new(0),
            log_dir,
        };

        writer.open_current_log()?;
        Ok(writer)
    }

    pub fn write(&self, entry: &LogEntry) -> Result<(), LogError> {
        let mut file_guard = self.current_file.lock().unwrap();
        let mut size_guard = self.current_size.lock().unwrap();

        // Rotate if needed
        if *size_guard >= self.config.max_file_size {
            drop(file_guard);
            drop(size_guard);
            self.rotate()?;
            file_guard = self.current_file.lock().unwrap();
            size_guard = self.current_size.lock().unwrap();
        }

        let formatted = self.format_entry(entry)?;
        let bytes = formatted.as_bytes();

        if let Some(ref mut writer) = *file_guard {
            writer.write_all(bytes)?;
            writer.write_all(b"\n")?;
            writer.flush()?;
            *size_guard += bytes.len() as u64 + 1;
        }

        Ok(())
    }

    fn rotate(&self) -> Result<(), LogError> {
        let mut file_guard = self.current_file.lock().unwrap();

        // Close current file
        *file_guard = None;

        // Rotate existing files (jarvy.4.log.gz -> deleted, 3->4, 2->3, 1->2, current->1)
        for i in (1..self.config.max_files).rev() {
            let from = self.log_dir.join(format!("jarvy.{}.log.gz", i));
            let to = self.log_dir.join(format!("jarvy.{}.log.gz", i + 1));
            if from.exists() {
                std::fs::rename(&from, &to)?;
            }
        }

        // Compress current log to .1.log.gz
        let current_path = self.log_dir.join("jarvy.log");
        if current_path.exists() {
            let compressed_path = self.log_dir.join("jarvy.1.log.gz");
            self.compress_file(&current_path, &compressed_path)?;
            std::fs::remove_file(&current_path)?;
        }

        // Enforce max_total_size
        self.enforce_size_limit()?;

        // Open new current log
        self.open_current_log_inner(&mut file_guard)?;

        Ok(())
    }

    fn compress_file(&self, src: &PathBuf, dst: &PathBuf) -> Result<(), LogError> {
        use flate2::write::GzEncoder;
        use flate2::Compression;

        let input = std::fs::read(src)?;
        let file = File::create(dst)?;
        let mut encoder = GzEncoder::new(file, Compression::default());
        encoder.write_all(&input)?;
        encoder.finish()?;
        Ok(())
    }
}
```

### Tracing Integration

```rust
// src/logging/mod.rs
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

pub fn init_logging(config: &LoggingConfig) -> Result<LoggingGuard, LogError> {
    // Create file writer layer
    let file_layer = if config.enabled {
        let writer = RotatingFileWriter::new(config.clone())?;
        Some(tracing_subscriber::fmt::layer()
            .with_writer(move || writer.make_writer())
            .with_ansi(false)
            .json())
    } else {
        None
    };

    // Console layer (existing behavior)
    let console_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr);

    // Combine layers
    tracing_subscriber::registry()
        .with(file_layer)
        .with(console_layer)
        .init();

    Ok(LoggingGuard { /* cleanup on drop */ })
}
```

### Sanitizer Implementation

```rust
// src/logging/sanitizer.rs
use regex::Regex;
use once_cell::sync::Lazy;

static SANITIZE_PATTERNS: Lazy<Vec<(Regex, &'static str)>> = Lazy::new(|| {
    vec![
        // API keys and tokens
        (Regex::new(r"(?i)(api[_-]?key|token|secret|password|auth)[=:]\s*['\"]?[\w\-\.]+['\"]?").unwrap(),
         "$1=[REDACTED]"),
        // Bearer tokens
        (Regex::new(r"Bearer\s+[\w\-\.]+").unwrap(),
         "Bearer [REDACTED]"),
        // SSH keys
        (Regex::new(r"-----BEGIN [A-Z ]+ KEY-----[\s\S]*?-----END [A-Z ]+ KEY-----").unwrap(),
         "[SSH KEY REDACTED]"),
        // AWS credentials
        (Regex::new(r"AKIA[0-9A-Z]{16}").unwrap(),
         "[AWS KEY REDACTED]"),
        // Email addresses
        (Regex::new(r"[\w.+-]+@[\w.-]+\.\w{2,}").unwrap(),
         "[EMAIL]"),
        // Home directory paths (replaced at runtime)
        // GitHub tokens
        (Regex::new(r"gh[ps]_[A-Za-z0-9_]{36,}").unwrap(),
         "[GITHUB TOKEN REDACTED]"),
        // npm tokens
        (Regex::new(r"npm_[A-Za-z0-9]{36,}").unwrap(),
         "[NPM TOKEN REDACTED]"),
    ]
});

pub fn sanitize(input: &str) -> String {
    let mut result = input.to_string();

    // Replace home directory
    if let Some(home) = dirs::home_dir() {
        result = result.replace(&home.to_string_lossy().to_string(), "~");
    }

    // Apply regex patterns
    for (pattern, replacement) in SANITIZE_PATTERNS.iter() {
        result = pattern.replace_all(&result, *replacement).to_string();
    }

    result
}
```

### Ticket Collector

```rust
// src/ticket/collector.rs
pub struct TicketCollector {
    scope: TicketScope,
    sanitizer: Sanitizer,
}

impl TicketCollector {
    pub fn collect(&self) -> Result<TicketData, TicketError> {
        let mut data = TicketData::new();

        // System info
        data.system = self.collect_system_info()?;

        // Jarvy info
        data.jarvy_version = env!("CARGO_PKG_VERSION").to_string();
        data.jarvy_config = self.collect_config()?;

        // Tools
        data.tools = self.collect_tools_status()?;

        // State
        data.state = self.collect_state()?;

        // Drift
        data.drift_report = self.collect_drift_report()?;

        // Logs
        data.logs = self.collect_logs()?;

        // Environment
        data.environment = self.collect_environment()?;

        // Network
        data.network = self.collect_network_config()?;

        // Sanitize all collected data
        self.sanitizer.sanitize_ticket(&mut data);

        Ok(data)
    }

    fn collect_system_info(&self) -> Result<SystemInfo, TicketError> {
        Ok(SystemInfo {
            os_name: std::env::consts::OS.to_string(),
            os_version: self.get_os_version()?,
            arch: std::env::consts::ARCH.to_string(),
            cpu_cores: num_cpus::get(),
            memory_total: sys_info::mem_info()?.total,
            shell: std::env::var("SHELL").unwrap_or_default(),
            locale: std::env::var("LANG").unwrap_or_default(),
        })
    }

    fn collect_logs(&self) -> Result<Vec<String>, TicketError> {
        let log_dir = dirs::home_dir()
            .ok_or(TicketError::NoHomeDir)?
            .join(".jarvy/logs");

        let current_log = log_dir.join("jarvy.log");
        if !current_log.exists() {
            return Ok(Vec::new());
        }

        // Read last N lines
        let content = std::fs::read_to_string(&current_log)?;
        let lines: Vec<String> = content
            .lines()
            .rev()
            .take(self.scope.log_lines)
            .map(|s| self.sanitizer.sanitize(s))
            .collect();

        Ok(lines.into_iter().rev().collect())
    }
}
```

## Implementation Steps

1. Create logging module structure
2. Implement LoggingConfig parsing
3. Implement RotatingFileWriter
4. Implement log rotation with compression
5. Integrate with tracing-subscriber
6. Implement sanitizer for sensitive data
7. Add `jarvy logs` CLI command
8. Create ticket module structure
9. Implement system info collector
10. Implement tools status collector
11. Implement log collector
12. Implement environment collector
13. Implement ZIP bundler
14. Add `jarvy ticket` CLI command
15. Implement ticket listing and cleanup
16. Write unit tests
17. Write integration tests
18. Update documentation

## Success Metrics

| Metric | Current | Target |
|--------|---------|--------|
| Log persistence | None | 30 days |
| Support ticket creation | Manual | <30 seconds |
| Diagnostic completeness | ~20% | 95% |
| Sensitive data exposure | Risk | Zero |
| Time to first response | Days | Hours |

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Disk space exhaustion | Medium | Medium | Size limits, rotation, cleanup |
| Sensitive data leakage | Medium | High | Comprehensive sanitization |
| Performance degradation | Low | Medium | Async writes, buffering |
| Log corruption | Low | Low | Atomic writes, checksums |
| User privacy concerns | Medium | Medium | Clear documentation, preview mode |

## Dependencies

### New Dependencies
- `flate2` - Gzip compression for log rotation
- `zip` - ZIP archive creation for tickets
- `sys-info` - System information collection
- `num_cpus` - CPU count detection

### Existing Dependencies
- `tracing` - Structured logging
- `tracing-subscriber` - Log formatting
- `serde_json` - JSON serialization
- `chrono` - Timestamps
- `dirs` - Standard directories

## Effort Estimate

| Task | Effort |
|------|--------|
| Logging module structure | 0.5 days |
| LoggingConfig and parsing | 0.5 days |
| RotatingFileWriter | 1.5 days |
| Log rotation with compression | 1 day |
| Tracing integration | 1 day |
| Sanitizer implementation | 1 day |
| `jarvy logs` command | 1 day |
| Ticket module structure | 0.5 days |
| System info collector | 0.5 days |
| Tools status collector | 0.5 days |
| All collectors | 1 day |
| ZIP bundler | 0.5 days |
| `jarvy ticket` command | 1 day |
| Ticket management (list, clean) | 0.5 days |
| Testing | 2 days |
| Documentation | 0.5 days |
| **Total** | **13 days** |

## Files to Create/Modify

### New Files
- `src/logging/mod.rs`
- `src/logging/config.rs`
- `src/logging/writer.rs`
- `src/logging/rotator.rs`
- `src/logging/formatter.rs`
- `src/logging/sanitizer.rs`
- `src/ticket/mod.rs`
- `src/ticket/collector.rs`
- `src/ticket/bundler.rs`
- `src/commands/logs_cmd.rs`
- `src/commands/ticket_cmd.rs`
- `tests/logging_integration.rs`
- `tests/ticket_integration.rs`

### Modified Files
- `src/config.rs` - Add LoggingConfig
- `src/lib.rs` - Export logging and ticket modules
- `src/main.rs` - Initialize logging, add commands
- `src/cli/args.rs` - Add Logs and Ticket commands
- `src/cli/subcommands.rs` - Add LogsAction and TicketAction
- `Cargo.toml` - Add dependencies
- `CLAUDE.md` - Document logging and ticket features

---

*PRD-050 v1.0 | Debug Ticket & Enhanced Logging System | Priority: Medium*
