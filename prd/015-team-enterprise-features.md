# PRD-015: Team and Enterprise Features

## Overview

Enable teams to share and manage development environment configurations through remote config loading, configuration inheritance, profiles, lock files, and audit logging.

## Problem Statement

Individual developers benefit from Jarvy's local configuration, but teams face challenges:
1. No standard way to share configurations across team members
2. Each developer maintains their own jarvy.toml, leading to environment drift
3. No mechanism to ensure reproducible environments across machines
4. Enterprises need audit trails for compliance requirements
5. Different projects/roles need different tool subsets (minimal vs full)

Currently, teams manually copy configurations or maintain them in shared documentation, leading to inconsistency and onboarding friction.

## Evidence

- Common request: "How do I share my jarvy.toml with my team?"
- Enterprise tools (nix, devcontainers) all support remote/shared configs
- Reproducibility is a top concern for CI/CD pipelines
- SOC2/compliance requirements mandate audit logging for environment changes

## Requirements

### Functional Requirements

1. **Remote config loading**: Load jarvy.toml from HTTP(S) URLs
2. **Config inheritance**: Extend base configurations with overrides
3. **Profiles**: Define tool subsets for different use cases
4. **Lock files**: Pin exact versions for reproducibility
5. **Audit logging**: Log all environment changes for compliance
6. **Config validation**: Lint and validate configurations before use

### Non-Functional Requirements

1. Remote configs must be fetched securely (HTTPS only)
2. Config validation must be fast (<100ms for local files)
3. Audit logs must be tamper-evident
4. Profiles should not increase config complexity for simple cases
5. Lock files must be deterministic across platforms

## User Stories

### US-1: Remote Config Loading

**As a** team lead
**I want to** host our team's jarvy.toml on our company server
**So that** all team members use the same environment configuration

```bash
# Load config from URL
jarvy setup --from https://company.com/configs/jarvy.toml

# Load config from GitHub raw URL
jarvy setup --from https://raw.githubusercontent.com/org/repo/main/jarvy.toml

# Load config from private URL with auth
jarvy setup --from https://company.com/configs/jarvy.toml --header "Authorization: Bearer $TOKEN"
```

### US-2: Config Inheritance/Extends

**As a** developer
**I want to** extend our team's base configuration with my personal additions
**So that** I get team tools plus my preferred extras

```toml
# jarvy.toml

# Inherit from remote or local base config
extends = "https://company.com/configs/base-jarvy.toml"

# Or extend from local file
# extends = "../shared/base-jarvy.toml"

# Override or add to inherited tools
[tools]
# This overrides the version from base config
node = "22"

# This adds a tool not in base config
helix = "latest"

# Inherited tools from base config are still installed
```

**Multiple inheritance:**
```toml
# Extend multiple configs (later configs override earlier)
extends = [
    "https://company.com/configs/base.toml",
    "https://company.com/configs/frontend-team.toml"
]

[tools]
# Local overrides take highest precedence
node = "22"
```

### US-3: Profiles

**As a** developer
**I want to** choose between minimal and full tool installations
**So that** I can save time/space when I don't need all tools

```toml
# jarvy.toml

[tools]
# Base tools installed for all profiles
git = "latest"

[profiles.minimal]
# Fast setup for quick tasks
description = "Minimal tools for quick fixes"
tools = ["git", "node"]

[profiles.full]
# Complete development environment
description = "Full development environment"
tools = ["git", "node", "docker", "kubectl", "helm", "terraform"]

[profiles.backend]
# Backend-specific tools
description = "Backend development tools"
extends = "minimal"
tools = ["go", "rust", "postgresql"]

[profiles.frontend]
# Frontend-specific tools
description = "Frontend development tools"
extends = "minimal"
tools = ["node", "bun", "chromium"]
```

**CLI usage:**
```bash
# Use a specific profile
jarvy setup --profile minimal

# List available profiles
jarvy profiles

# Show profile details
jarvy profiles show backend
```

### US-4: Lock Files

**As a** DevOps engineer
**I want to** pin exact tool versions in a lock file
**So that** all environments are identical and reproducible

```bash
# Generate lock file from current config
jarvy lock

# This creates jarvy.lock with exact versions:
```

```toml
# jarvy.lock (auto-generated, do not edit)
# Generated at: 2024-01-15T10:30:00Z
# Platform: darwin-arm64

[tools]
node = { version = "20.11.0", checksum = "sha256:abc123..." }
rust = { version = "1.75.0", checksum = "sha256:def456..." }
go = { version = "1.21.6", checksum = "sha256:ghi789..." }

[metadata]
jarvy_version = "0.5.0"
generated_at = "2024-01-15T10:30:00Z"
platform = "darwin-arm64"
```

```bash
# Install from lock file (exact versions)
jarvy setup --locked

# Update lock file to latest compatible versions
jarvy lock --update

# Verify current environment matches lock file
jarvy lock --verify
```

### US-5: Audit Logging

**As a** security officer
**I want to** see a log of all environment changes
**So that** I can audit compliance and troubleshoot issues

```toml
# ~/.jarvy/config.toml

[audit]
enabled = true
log_file = "~/.jarvy/audit.log"
log_format = "json"  # or "text"
include_user = true
include_machine = true
```

**Audit log entries:**
```json
{"timestamp":"2024-01-15T10:30:00Z","event":"tool_installed","tool":"node","version":"20.11.0","user":"john","machine":"laptop-1","config_source":"https://company.com/jarvy.toml"}
{"timestamp":"2024-01-15T10:30:05Z","event":"tool_installed","tool":"rust","version":"1.75.0","user":"john","machine":"laptop-1","config_source":"https://company.com/jarvy.toml"}
{"timestamp":"2024-01-15T10:31:00Z","event":"setup_completed","tools_installed":2,"duration_seconds":60,"user":"john","machine":"laptop-1"}
```

```bash
# View recent audit log entries
jarvy audit

# View audit log for specific tool
jarvy audit --tool node

# Export audit log for compliance
jarvy audit --export audit-report.json --since 2024-01-01
```

### US-6: Config Validation and Linting

**As a** config author
**I want to** validate my jarvy.toml before sharing
**So that** team members don't encounter errors

```bash
# Validate local config
jarvy validate

# Validate remote config
jarvy validate --from https://company.com/jarvy.toml

# Validate with strict mode (warnings become errors)
jarvy validate --strict

# Output:
# Validating jarvy.toml...
# [WARN] Line 5: Tool 'node' version '20' will match 20.x.x - consider pinning exact version
# [ERROR] Line 12: Unknown tool 'nodejs' - did you mean 'node'?
# [WARN] Line 18: Profile 'backend' extends 'minimal' but 'minimal' is not defined
#
# Validation failed: 1 error, 2 warnings
```

**Validation checks:**
- Syntax errors in TOML
- Unknown tool names
- Invalid version strings
- Missing profile references
- Circular extends chains
- Conflicting tool versions
- Deprecated configuration options

## Technical Approach

### Remote Config Fetching

```rust
// src/remote/fetch.rs
pub struct RemoteConfig {
    pub url: Url,
    pub headers: HashMap<String, String>,
    pub timeout: Duration,
    pub cache_duration: Duration,
}

impl RemoteConfig {
    pub async fn fetch(&self) -> Result<String, RemoteError> {
        // Validate URL scheme (HTTPS only in production)
        if !self.url.scheme().starts_with("https") && !cfg!(debug_assertions) {
            return Err(RemoteError::InsecureUrl(self.url.clone()));
        }

        // Check cache first
        if let Some(cached) = self.check_cache()? {
            return Ok(cached);
        }

        // Fetch with timeout
        let client = reqwest::Client::builder()
            .timeout(self.timeout)
            .build()?;

        let mut request = client.get(self.url.clone());
        for (key, value) in &self.headers {
            request = request.header(key, value);
        }

        let response = request.send().await?;

        if !response.status().is_success() {
            return Err(RemoteError::HttpError(response.status()));
        }

        let content = response.text().await?;

        // Validate it's valid TOML before caching
        toml::from_str::<JarvyConfig>(&content)?;

        self.cache_content(&content)?;

        Ok(content)
    }
}
```

### Config Inheritance Resolution

```rust
// src/config/extends.rs
pub fn resolve_extends(config: &JarvyConfig, depth: usize) -> Result<JarvyConfig, ConfigError> {
    const MAX_DEPTH: usize = 10;

    if depth > MAX_DEPTH {
        return Err(ConfigError::CircularExtends);
    }

    let extends = match &config.extends {
        Some(Extends::Single(url)) => vec![url.clone()],
        Some(Extends::Multiple(urls)) => urls.clone(),
        None => return Ok(config.clone()),
    };

    let mut base = JarvyConfig::default();

    for url in extends {
        let parent = fetch_config(&url)?;
        let resolved_parent = resolve_extends(&parent, depth + 1)?;
        base = merge_configs(base, resolved_parent);
    }

    // Child config overrides parent
    Ok(merge_configs(base, config.clone()))
}

fn merge_configs(base: JarvyConfig, overlay: JarvyConfig) -> JarvyConfig {
    JarvyConfig {
        tools: base.tools.into_iter()
            .chain(overlay.tools)
            .collect(),
        profiles: base.profiles.into_iter()
            .chain(overlay.profiles)
            .collect(),
        // Other fields...
    }
}
```

### Lock File Generation

```rust
// src/lock/mod.rs
#[derive(Serialize, Deserialize)]
pub struct LockFile {
    pub tools: HashMap<String, LockedTool>,
    pub metadata: LockMetadata,
}

#[derive(Serialize, Deserialize)]
pub struct LockedTool {
    pub version: String,
    pub checksum: Option<String>,
    pub source: String,
}

impl LockFile {
    pub fn generate(config: &JarvyConfig) -> Result<Self, LockError> {
        let mut tools = HashMap::new();

        for (name, spec) in &config.tools {
            let resolved = resolve_version(name, spec)?;
            tools.insert(name.clone(), LockedTool {
                version: resolved.version,
                checksum: resolved.checksum,
                source: resolved.source,
            });
        }

        Ok(LockFile {
            tools,
            metadata: LockMetadata {
                jarvy_version: env!("CARGO_PKG_VERSION").to_string(),
                generated_at: chrono::Utc::now(),
                platform: current_platform(),
            },
        })
    }

    pub fn verify(&self) -> Result<VerifyResult, LockError> {
        let mut mismatches = Vec::new();

        for (name, locked) in &self.tools {
            if let Some(installed) = get_installed_version(name)? {
                if installed != locked.version {
                    mismatches.push(Mismatch {
                        tool: name.clone(),
                        expected: locked.version.clone(),
                        actual: installed,
                    });
                }
            }
        }

        Ok(VerifyResult { mismatches })
    }
}
```

## Security Considerations

### Remote Config Security

1. **HTTPS Only**: Remote configs must be loaded over HTTPS (HTTP allowed only in debug mode for local testing)
2. **Content Validation**: Validate TOML syntax and schema before executing any commands
3. **No Arbitrary Code**: Config files cannot execute arbitrary code; only declarative tool specifications
4. **Certificate Validation**: Use system certificate store, require valid TLS certificates
5. **Timeout Protection**: Enforce timeouts to prevent hanging on slow/malicious servers
6. **Size Limits**: Limit config file size (e.g., 1MB) to prevent memory exhaustion

### Config Caching

1. **Cache Location**: Store in `~/.jarvy/cache/configs/` with appropriate permissions (0600)
2. **Cache Invalidation**: TTL-based expiry (default 1 hour) or manual refresh
3. **Cache Integrity**: Store content hash to detect corruption

### Audit Log Security

1. **Append-Only**: Audit log is append-only, no deletion API
2. **Tamper Detection**: Include hash chain linking entries
3. **File Permissions**: Audit log created with 0600 permissions
4. **Rotation**: Support log rotation to prevent disk exhaustion

## Proposed Config Syntax Summary

```toml
# jarvy.toml

# Extend from remote or local config
extends = "https://company.com/configs/base-jarvy.toml"

[tools]
git = "latest"
node = "20"
docker = "latest"
kubectl = "latest"

[profiles.minimal]
description = "Quick setup for hotfixes"
tools = ["git", "node"]

[profiles.full]
description = "Complete development environment"
tools = ["git", "node", "docker", "kubectl"]

[profiles.backend]
description = "Backend development"
extends = "minimal"
tools = ["go", "rust", "postgresql"]
```

## CLI Commands

```bash
# Remote config
jarvy setup --from https://company.com/jarvy.toml
jarvy setup --from https://company.com/jarvy.toml --header "Authorization: Bearer $TOKEN"

# Profiles
jarvy setup --profile minimal
jarvy profiles
jarvy profiles show backend

# Lock files
jarvy lock                    # Generate lock file
jarvy setup --locked          # Install from lock file
jarvy lock --update           # Update lock file
jarvy lock --verify           # Verify environment matches lock

# Audit
jarvy audit                   # View recent entries
jarvy audit --tool node       # Filter by tool
jarvy audit --export report.json

# Validation
jarvy validate                # Validate local config
jarvy validate --from URL     # Validate remote config
jarvy validate --strict       # Warnings become errors
```

## Implementation Steps

1. Create `src/remote/mod.rs` module for remote config fetching
2. Add HTTPS client with reqwest (minimal features)
3. Implement config caching in `~/.jarvy/cache/`
4. Add `extends` field parsing to config.rs
5. Implement inheritance resolution with depth limit
6. Add `profiles` section to config schema
7. Implement profile selection logic
8. Create `src/lock/mod.rs` for lock file management
9. Implement version resolution and checksum calculation
10. Add `jarvy lock` CLI commands
11. Create `src/audit/mod.rs` for audit logging
12. Implement append-only log with hash chain
13. Add `jarvy audit` CLI commands
14. Create `src/validate/mod.rs` for config validation
15. Implement validation rules and linting
16. Add `jarvy validate` CLI command
17. Write comprehensive tests for all features
18. Update documentation

## Acceptance Criteria

1. **Remote Config Loading**
   - `jarvy setup --from <URL>` fetches and uses remote config
   - HTTPS URLs work with valid certificates
   - HTTP URLs are rejected in release builds
   - Custom headers can be passed for authentication
   - Cached configs are used when within TTL
   - Clear error messages for network failures

2. **Config Inheritance**
   - `extends` field resolves single URL/path
   - `extends` field resolves array of URLs/paths
   - Child config overrides parent values
   - Circular extends detected and rejected
   - Maximum depth (10) enforced

3. **Profiles**
   - `[profiles.name]` sections define tool subsets
   - `jarvy setup --profile <name>` uses profile
   - `jarvy profiles` lists available profiles
   - Profiles can extend other profiles
   - Missing profile references are validation errors

4. **Lock Files**
   - `jarvy lock` generates jarvy.lock with exact versions
   - `jarvy setup --locked` installs exact versions from lock
   - `jarvy lock --verify` compares environment to lock
   - Lock file includes checksums where available
   - Lock file includes generation metadata

5. **Audit Logging**
   - When enabled, all tool installations are logged
   - Log entries include timestamp, user, machine, tool, version
   - `jarvy audit` displays recent entries
   - Log export works in JSON format
   - Log file has secure permissions

6. **Config Validation**
   - `jarvy validate` checks local jarvy.toml
   - Unknown tool names are errors
   - Invalid version strings are errors
   - Missing profile references are errors
   - Circular extends are detected
   - Helpful suggestions for typos

## Success Metrics

| Metric | Current | Target |
|--------|---------|--------|
| Config sharing | Manual copy | URL-based |
| Environment drift | Common | Rare (with locks) |
| Onboarding time | ~30 min config | < 5 min |
| Audit compliance | None | Full audit trail |
| Config errors | Runtime | Caught at validation |

## Non-Goals

1. **User Authentication**: Jarvy does not implement its own auth; use HTTP headers or pre-signed URLs
2. **Paid Features**: All features are open source; no tiered feature gates
3. **Config Hosting**: Jarvy does not host configs; use existing infrastructure (GitHub, S3, etc.)
4. **Secret Management**: Jarvy does not store secrets; use environment variables or secret managers
5. **Role-Based Access**: Access control is handled by config hosting infrastructure

## Risks

1. **Remote config availability**: Network issues could block setup
   - Mitigation: Cache configs, allow offline mode with cached version
2. **Security of remote configs**: Malicious configs could be served
   - Mitigation: HTTPS only, validate before use, no arbitrary code execution
3. **Lock file staleness**: Lock files may become outdated
   - Mitigation: `jarvy lock --update` command, CI checks for lock freshness
4. **Audit log growth**: Logs could consume disk space
   - Mitigation: Log rotation, configurable retention period
5. **Inheritance complexity**: Deep extends chains hard to debug
   - Mitigation: Depth limit, `jarvy config show --resolved` to display final config

## Dependencies

- `reqwest` - HTTP client (already used or minimal features)
- `sha2` - Checksum calculation for lock files
- `chrono` - Timestamps for audit logging

## Effort Estimate

- Remote config fetching: 1 day
- Config caching: 0.5 days
- Inheritance resolution: 1 day
- Profiles implementation: 1 day
- Lock file generation: 1.5 days
- Lock file verification: 0.5 days
- Audit logging: 1 day
- Config validation: 1 day
- CLI commands: 1 day
- Testing: 2 days
- Documentation: 1 day

**Total: ~11.5 days**

## Files to Create/Modify

### New Files
- `src/remote/mod.rs` - Remote config fetching
- `src/remote/cache.rs` - Config caching
- `src/extends/mod.rs` - Inheritance resolution
- `src/profiles/mod.rs` - Profile management
- `src/lock/mod.rs` - Lock file generation/verification
- `src/audit/mod.rs` - Audit logging
- `src/validate/mod.rs` - Config validation
- `tests/remote_config.rs` - Integration tests
- `tests/profiles.rs` - Profile tests
- `tests/lock_file.rs` - Lock file tests

### Modified Files
- `src/config.rs` - Add extends, profiles parsing
- `src/main.rs` - Add new CLI commands
- `Cargo.toml` - Add dependencies if needed
