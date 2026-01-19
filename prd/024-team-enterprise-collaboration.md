# PRD-024: Team & Enterprise Collaboration

## Overview

Add features that enable teams to standardize developer environments across organizations, including shared configuration templates, configuration inheritance, and automatic tool updates.

## Problem Statement

Jarvy currently works well for individual developers, but lacks features for team-wide adoption:
- No way to share team-standard configurations
- No configuration inheritance (DRY principle violations)
- Teams manually copy configs, leading to drift and inconsistency
- No easy way to keep tools updated across teams

Teams need standardization without creating rigidity that blocks individual productivity.

## Evidence

- Enterprise customers ask: "How do I ensure all devs have the same tools?"
- Teams duplicate configs manually → config drift
- Different projects need consistent base configurations
- Developers need simple way to keep tools current

## Requirements

### Functional Requirements

1. **Config inheritance**: `extends` field to compose configurations
2. **Team config registry**: Centralized config discovery and sharing
3. **Automatic tool updates**: Check and update tools to latest versions
4. **Remote config support**: Fetch configs from URLs

### Non-Functional Requirements

1. Works with existing jarvy.toml format (backward compatible)
2. Inheritance resolution is fast (< 100ms)
3. Graceful degradation when remote configs unavailable
4. Clear error messages for inheritance issues

## Non-Goals

- Multi-tenancy or SaaS platform
- User management UI
- Real-time config synchronization
- Authentication/SSO for config servers
- Policy enforcement or compliance checking

## Feature Specifications

### 1. Configuration Inheritance

Allow configs to extend other configs.

```toml
# team-base.toml (shared by all team members)
[tools]
git = "latest"
docker = "latest"
jq = "latest"

[hooks.git]
script = "git config --global core.autocrlf input"

---

# jarvy.toml (individual developer)
extends = "https://raw.githubusercontent.com/company/configs/main/team-base.toml"

[tools]
# Add project-specific tools
node = "20"
rust = "1.75"

# Override team default
docker = "24.0"  # Need specific version for project

[hooks.custom]
script = "echo 'Project-specific setup'"
```

**Inheritance behavior:**
- Deep merge of tool sections (child overrides parent)
- Hooks are merged (both parent and child run)
- Multiple inheritance: `extends = ["base.toml", "frontend.toml"]`
- Circular dependency detection
- Maximum inheritance depth: 10 levels

### Recursive Extending Resolution

When configs extend other configs that also use `extends`, Jarvy resolves them using **depth-first, left-to-right** traversal with **last-write-wins** merging.

**Resolution Algorithm:**

```
Given: A extends [B, C], B extends [D], C extends [D, E]

Resolution order (depth-first):
1. Load D (B's parent)
2. Load B, merge with D → B'
3. Load D (C's parent, cached from step 1)
4. Load E (C's parent)
5. Merge D + E → DE
6. Load C, merge with DE → C'
7. Merge B' + C' → BC
8. Load A, merge with BC → Final

Traversal tree:
        A
       / \
      B   C
      |  / \
      D D   E

Visit order: D → B → D(cached) → E → C → A
```

**Merge Rules:**

| Section | Behavior | Example |
|---------|----------|---------|
| `[tools]` | Deep merge, child overrides parent | Parent: `git = "latest"`, Child: `git = "2.40"` → `git = "2.40"` |
| `[hooks.{tool}]` | Append (both run) | Parent hook + Child hook = Both execute in order |
| `[hooks.pre_setup]` | Append (both run) | All pre_setup hooks run, parent first |
| `[hooks.post_setup]` | Append (both run) | All post_setup hooks run, parent first |
| `[env.vars]` | Deep merge, child overrides | Child values override parent on conflict |
| `[env.secrets]` | Deep merge, child overrides | Child values override parent on conflict |
| `[services]` | Deep merge, child overrides | Child service config overrides parent |

**Diamond Dependency Handling:**

When multiple paths lead to the same config (diamond pattern), the config is:
1. Loaded only once (cached after first load)
2. Applied only once in the merge chain (at its first occurrence)

```
# Diamond: A extends [B, C], both B and C extend D
# D is processed once, not twice

A
├── B ── D    ← D processed here
└── C ── D    ← D skipped (already in visited set)

Final merge: D → B → C → A
```

**Hook Execution Order:**

For recursive extends, hooks execute in **ancestor-first, left-to-right** order:

```toml
# Given: project.toml extends [team.toml, frontend.toml]
#        team.toml extends [company-base.toml]

# Hook execution order for pre_setup:
# 1. company-base.toml pre_setup
# 2. team.toml pre_setup
# 3. frontend.toml pre_setup
# 4. project.toml pre_setup

# Tool-specific hook order for [hooks.node]:
# 1. company-base.toml hooks.node (if exists)
# 2. team.toml hooks.node (if exists)
# 3. frontend.toml hooks.node (if exists)
# 4. project.toml hooks.node (if exists)
```

**Error Handling:**

| Error | Detection | Message |
|-------|-----------|---------|
| Circular dependency | `visited` set check | `Circular dependency detected: A → B → C → A` |
| Max depth exceeded | Depth counter > 10 | `Maximum inheritance depth (10) exceeded at: {path}` |
| Missing parent config | HTTP 404 / file not found | `Could not load parent config: {url}. Use --offline to use cached version.` |
| Invalid TOML in parent | Parse error | `Invalid TOML in parent config {url}: {error}` |
| Conflicting extends types | Type check | `Cannot mix URL and local extends in same array` |

**Caching Behavior for Recursive Extends:**

```
Cache key: SHA256(normalized_url)
Cache TTL: 1 hour (configurable)
Cache location: ~/.jarvy/cache/configs/

On resolve:
1. Check cache for each URL in extends chain
2. If cache hit and fresh → use cached
3. If cache miss or stale → fetch, validate TOML, cache
4. If fetch fails and cache exists → use stale cache with warning
5. If fetch fails and no cache → error (unless --offline)
```

**Debug/Inspect Commands:**

```bash
# Show full inheritance chain
jarvy config show --extends-chain

# Output:
# Inheritance Chain
# =================
# 1. https://company.com/configs/base.toml
#    └── (no parents)
# 2. https://company.com/configs/team.toml
#    └── extends: https://company.com/configs/base.toml
# 3. ./jarvy.toml
#    └── extends: https://company.com/configs/team.toml
#
# Resolution order: base.toml → team.toml → jarvy.toml
# Total depth: 3 levels

# Validate extends chain without running setup
jarvy config validate --check-extends

# Force refresh all cached parent configs
jarvy config refresh --recursive
```

```bash
# View resolved configuration
jarvy config show --resolved

# Output:
# Resolved Configuration (3 sources)
# ==================================
#
# Source chain:
#   1. https://raw.githubusercontent.com/company/configs/main/team-base.toml
#   2. https://raw.githubusercontent.com/company/configs/main/frontend.toml
#   3. ./jarvy.toml (local)
#
# [tools]
# git = "latest"        # from: team-base.toml
# docker = "24.0"       # from: jarvy.toml (overrides team-base)
# jq = "latest"         # from: team-base.toml
# node = "20"           # from: frontend.toml
# rust = "1.75"         # from: jarvy.toml
#
# [hooks]
# git: team-base.toml
# custom: jarvy.toml

# Validate inheritance chain
jarvy config validate --check-extends
```

**Local file inheritance:**

```toml
# jarvy.toml
extends = "./configs/base.toml"

# Or relative to project root
extends = "configs/team-base.toml"
```

### 2. Team Config Registry

Discover and use shared configurations.

```bash
# Register a config source
jarvy team add company https://raw.githubusercontent.com/company/jarvy-configs/main/

# Output:
# Added config source: company
#   URL: https://raw.githubusercontent.com/company/jarvy-configs/main/
#
# Discovered configs:
#   company/base          - Base tools for all developers
#   company/frontend      - Frontend development stack
#   company/backend       - Backend development stack
#   company/ml            - Machine learning stack

# List registered sources
jarvy team list

# Output:
# Registered Config Sources
# =========================
#
# company
#   URL: https://raw.githubusercontent.com/company/jarvy-configs/main/
#   Configs: 4 available
#   Last sync: 2 hours ago
#
# community
#   URL: https://jarvy.dev/community/
#   Configs: 25 available
#   Last sync: 1 day ago

# Browse configs from a source
jarvy team browse company

# Use a config to create jarvy.toml
jarvy init --from company/frontend

# Sync/update cached configs
jarvy team sync

# Remove a source
jarvy team remove company
```

**Registry index format:**

```toml
# index.toml (at registry root)
[registry]
name = "Company Configs"
description = "Standard configurations for Company developers"
version = "1.0"

[[configs]]
name = "base"
path = "base.toml"
description = "Base tools for all developers"
tags = ["essential"]

[[configs]]
name = "frontend"
path = "frontend.toml"
description = "Frontend development stack"
tags = ["web", "javascript"]

[[configs]]
name = "backend"
path = "backend.toml"
description = "Backend development stack"
tags = ["api", "server"]
```

**Registry features:**
- Multiple sources
- Config discovery via index.toml
- Caching with TTL
- Offline fallback to cached configs

### 3. Automatic Tool Updates

Keep tools up to date with automatic update checking and one-command updates.

```bash
# Check for available updates
jarvy update

# Output:
# Checking for updates...
#
# Updates available:
#   git      2.43.0 → 2.44.0
#   node     20.11.0 → 20.12.0
#   docker   24.0.7 → 25.0.0
#
# Run `jarvy update --all` to update all tools
# Or `jarvy update <tool>` to update a specific tool

# Update all outdated tools
jarvy update --all

# Update specific tool
jarvy update node

# Preview what would be updated
jarvy update --dry-run

# Output:
# Dry run - no changes will be made
#
# Would update:
#   git      2.43.0 → 2.44.0 (via homebrew)
#   node     20.11.0 → 20.12.0 (via nvm)
#   docker   24.0.7 → 25.0.0 (via homebrew-cask)
```

**Configuration:**

```toml
# jarvy.toml or ~/.jarvy/config.toml
[update]
auto_check = true       # Check for updates after setup (default: true)
auto_update = false     # Automatically update tools (default: false)
check_interval = "24h"  # How often to check (default: 24h)
exclude = ["java"]      # Tools to skip when updating
```

**Update features:**
- Check for updates across all package managers
- Update all or specific tools
- Dry-run mode to preview changes
- Configurable auto-check on setup
- Exclude specific tools from updates
- Cached update checks to avoid rate limits
- CI-aware (skips notifications in CI)

## Acceptance Criteria

### Configuration Inheritance
- [ ] `extends` field accepts URL or local path
- [ ] Multiple inheritance with array syntax
- [ ] Deep merge of tool sections
- [ ] Hook merging (both parent and child run)
- [ ] Circular dependency detection with clear error
- [ ] Maximum depth enforcement (10 levels)
- [ ] `jarvy config show --resolved` displays merged config
- [ ] Cache for remote configs with TTL
- [ ] Graceful fallback when remote unavailable

### Team Config Registry
- [ ] `jarvy team add` registers config source
- [ ] `jarvy team list` shows registered sources
- [ ] `jarvy team browse` discovers available configs
- [ ] `jarvy team sync` updates cached configs
- [ ] `jarvy team remove` unregisters source
- [ ] Index.toml format for config discovery
- [ ] Offline fallback to cached configs

### Automatic Tool Updates
- [ ] `jarvy update` checks for available updates
- [ ] `jarvy update --all` updates all outdated tools
- [ ] `jarvy update <tool>` updates specific tool
- [ ] `jarvy update --dry-run` previews changes
- [ ] Auto-check on setup with configurable interval
- [ ] Exclude list for tools to skip
- [ ] Update notifications shown after setup

## Technical Approach

### Module Structure

```
src/
  team/
    mod.rs            # Team features
    inheritance.rs    # Config inheritance resolution
    registry.rs       # Team config registry
    cache.rs          # Config caching
  update/
    mod.rs            # Update management
    check.rs          # Update checking logic
    config.rs         # Auto-update configuration
  commands/
    team.rs           # jarvy team command
    update.rs         # jarvy update command
```

### Inheritance Resolution

```rust
// src/team/inheritance.rs
use std::collections::{HashSet, HashMap};

const MAX_DEPTH: usize = 10;

pub struct InheritanceResolver {
    cache: ConfigCache,
    /// Tracks configs currently in the resolution stack (for cycle detection)
    in_progress: HashSet<String>,
    /// Caches already-resolved configs (for diamond dependency handling)
    resolved_cache: HashMap<String, Config>,
    depth: usize,
}

#[derive(Debug, Clone)]
pub struct ResolutionTrace {
    pub chain: Vec<String>,
    pub total_depth: usize,
}

impl InheritanceResolver {
    pub fn new(cache: ConfigCache) -> Self {
        Self {
            cache,
            in_progress: HashSet::new(),
            resolved_cache: HashMap::new(),
            depth: 0,
        }
    }

    /// Resolve a config and all its ancestors recursively.
    /// Uses depth-first, left-to-right traversal with last-write-wins merging.
    pub fn resolve(&mut self, config_path: &str) -> Result<Config, Error> {
        self.resolve_with_trace(config_path).map(|(config, _)| config)
    }

    /// Resolve with trace for debugging/display
    pub fn resolve_with_trace(&mut self, config_path: &str) -> Result<(Config, ResolutionTrace), Error> {
        let mut trace = ResolutionTrace {
            chain: Vec::new(),
            total_depth: 0,
        };
        let config = self.resolve_internal(config_path, &mut trace)?;
        Ok((config, trace))
    }

    fn resolve_internal(&mut self, config_path: &str, trace: &mut ResolutionTrace) -> Result<Config, Error> {
        // Check max depth
        if self.depth > MAX_DEPTH {
            return Err(Error::MaxDepthExceeded {
                path: config_path.to_string(),
                depth: self.depth,
            });
        }

        // Check for circular dependency (config is currently being resolved)
        if self.in_progress.contains(config_path) {
            let cycle = self.build_cycle_path(config_path);
            return Err(Error::CircularDependency(cycle));
        }

        // Check if already fully resolved (diamond dependency optimization)
        if let Some(cached) = self.resolved_cache.get(config_path) {
            return Ok(cached.clone());
        }

        // Mark as in-progress for cycle detection
        self.in_progress.insert(config_path.to_string());
        self.depth += 1;
        trace.chain.push(config_path.to_string());
        trace.total_depth = trace.total_depth.max(self.depth);

        // Load the config (from cache or fetch)
        let config = self.load_config(config_path)?;

        // Resolve parents first (depth-first)
        let resolved = if let Some(extends) = &config.extends {
            let parents = match extends {
                Extends::Single(path) => vec![path.clone()],
                Extends::Multiple(paths) => paths.clone(),
            };

            // Resolve all parents left-to-right, merge incrementally
            let mut merged = Config::default();
            for parent_path in parents {
                let parent = self.resolve_internal(&parent_path, trace)?;
                merged = self.merge_configs(merged, parent);
            }

            // Child overrides merged parents
            self.merge_configs(merged, config)
        } else {
            config
        };

        // Mark as fully resolved (for diamond dependency reuse)
        self.in_progress.remove(config_path);
        self.resolved_cache.insert(config_path.to_string(), resolved.clone());
        self.depth -= 1;

        Ok(resolved)
    }

    /// Merge two configs: base + overlay (overlay wins on conflicts)
    fn merge_configs(&self, base: Config, overlay: Config) -> Config {
        Config {
            extends: overlay.extends.or(base.extends),
            tools: self.merge_tools(base.tools, overlay.tools),
            hooks: self.merge_hooks(base.hooks, overlay.hooks),
            env: self.merge_env(base.env, overlay.env),
            services: overlay.services.or(base.services),
        }
    }

    /// Deep merge tools: overlay values override base on key conflict
    fn merge_tools(&self, base: HashMap<String, ToolConfig>, overlay: HashMap<String, ToolConfig>) -> HashMap<String, ToolConfig> {
        let mut merged = base;
        for (key, value) in overlay {
            merged.insert(key, value); // Overlay wins
        }
        merged
    }

    /// Merge hooks: append (both run), overlay hooks come after base hooks
    fn merge_hooks(&self, base: HooksConfig, overlay: HooksConfig) -> HooksConfig {
        HooksConfig {
            pre_setup: self.append_hooks(base.pre_setup, overlay.pre_setup),
            post_setup: self.append_hooks(base.post_setup, overlay.post_setup),
            tool_hooks: self.merge_tool_hooks(base.tool_hooks, overlay.tool_hooks),
        }
    }

    /// Append hook lists (base first, then overlay)
    fn append_hooks(&self, base: Option<Vec<Hook>>, overlay: Option<Vec<Hook>>) -> Option<Vec<Hook>> {
        match (base, overlay) {
            (None, None) => None,
            (Some(b), None) => Some(b),
            (None, Some(o)) => Some(o),
            (Some(mut b), Some(o)) => {
                b.extend(o);
                Some(b)
            }
        }
    }

    /// Merge per-tool hooks (append for same tool)
    fn merge_tool_hooks(&self, base: HashMap<String, Vec<Hook>>, overlay: HashMap<String, Vec<Hook>>) -> HashMap<String, Vec<Hook>> {
        let mut merged = base;
        for (tool, hooks) in overlay {
            merged.entry(tool).or_default().extend(hooks);
        }
        merged
    }

    fn build_cycle_path(&self, config_path: &str) -> String {
        // Build readable cycle path: A → B → C → A
        let mut path_parts: Vec<&str> = self.in_progress.iter().map(|s| s.as_str()).collect();
        path_parts.push(config_path);
        path_parts.join(" → ")
    }

    fn load_config(&self, path: &str) -> Result<Config, Error> {
        if path.starts_with("http://") || path.starts_with("https://") {
            self.load_remote_config(path)
        } else {
            self.load_local_config(path)
        }
    }

    fn load_remote_config(&self, url: &str) -> Result<Config, Error> {
        // Try cache first
        if let Some(cached) = self.cache.get(url) {
            return toml::from_str(&cached.content)
                .map_err(|e| Error::InvalidToml { url: url.to_string(), error: e.to_string() });
        }

        // Fetch and cache
        let content = self.fetch_url(url)?;
        let config: Config = toml::from_str(&content)
            .map_err(|e| Error::InvalidToml { url: url.to_string(), error: e.to_string() })?;

        self.cache.set(url, &content)?;
        Ok(config)
    }

    fn load_local_config(&self, path: &str) -> Result<Config, Error> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| Error::FileNotFound { path: path.to_string(), error: e.to_string() })?;

        toml::from_str(&content)
            .map_err(|e| Error::InvalidToml { url: path.to_string(), error: e.to_string() })
    }

    fn fetch_url(&self, url: &str) -> Result<String, Error> {
        let response = ureq::Agent::new_with_defaults()
            .get(url)
            .call()
            .map_err(|e| Error::FetchFailed { url: url.to_string(), error: e.to_string() })?;

        response.into_body().read_to_string()
            .map_err(|e| Error::FetchFailed { url: url.to_string(), error: e.to_string() })
    }
}

#[derive(Debug)]
pub enum Error {
    MaxDepthExceeded { path: String, depth: usize },
    CircularDependency(String),
    FileNotFound { path: String, error: String },
    FetchFailed { url: String, error: String },
    InvalidToml { url: String, error: String },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::MaxDepthExceeded { path, depth } =>
                write!(f, "Maximum inheritance depth ({}) exceeded at: {}", depth, path),
            Error::CircularDependency(cycle) =>
                write!(f, "Circular dependency detected: {}", cycle),
            Error::FileNotFound { path, error } =>
                write!(f, "Could not load config '{}': {}", path, error),
            Error::FetchFailed { url, error } =>
                write!(f, "Could not fetch config '{}': {}. Use --offline to use cached version.", url, error),
            Error::InvalidToml { url, error } =>
                write!(f, "Invalid TOML in config '{}': {}", url, error),
        }
    }
}
```

### Config Cache

```rust
// src/team/cache.rs
use std::time::{Duration, SystemTime};

const DEFAULT_TTL: Duration = Duration::from_secs(3600); // 1 hour

pub struct ConfigCache {
    cache_dir: PathBuf,
    ttl: Duration,
}

impl ConfigCache {
    pub fn get(&self, url: &str) -> Option<CachedConfig> {
        let cache_path = self.cache_path(url);
        if !cache_path.exists() {
            return None;
        }

        let metadata = fs::metadata(&cache_path).ok()?;
        let modified = metadata.modified().ok()?;

        if SystemTime::now().duration_since(modified).ok()? > self.ttl {
            return None; // Expired
        }

        let content = fs::read_to_string(&cache_path).ok()?;
        Some(CachedConfig { content, path: cache_path })
    }

    pub fn set(&self, url: &str, content: &str) -> Result<(), Error> {
        let cache_path = self.cache_path(url);
        fs::create_dir_all(cache_path.parent().unwrap())?;
        fs::write(cache_path, content)?;
        Ok(())
    }

    fn cache_path(&self, url: &str) -> PathBuf {
        let hash = sha256::digest(url);
        self.cache_dir.join(&hash[..16]).with_extension("toml")
    }
}
```

### Update Checking

```rust
// src/update/check.rs
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

const UPDATE_CACHE_TTL: Duration = Duration::from_secs(3600); // 1 hour

pub struct UpdateChecker {
    cache: UpdateCache,
}

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub tool: String,
    pub current: String,
    pub available: String,
    pub source: String,
}

impl UpdateChecker {
    /// Check for updates for all tools in config
    pub fn check_updates(&self, config: &Config) -> Result<Vec<UpdateInfo>, Error> {
        let mut updates = Vec::new();

        for (name, _spec) in &config.tools {
            if let Some(update) = self.check_tool_update(name)? {
                updates.push(update);
            }
        }

        Ok(updates)
    }

    /// Check if a specific tool has an update available
    pub fn check_tool_update(&self, tool: &str) -> Result<Option<UpdateInfo>, Error> {
        // Check cache first
        if let Some(cached) = self.cache.get(tool) {
            return Ok(cached);
        }

        let current = get_installed_version(tool)?;
        let available = get_latest_version(tool)?;

        let result = if semver_newer(&available, &current) {
            Some(UpdateInfo {
                tool: tool.to_string(),
                current,
                available,
                source: get_install_source(tool)?,
            })
        } else {
            None
        };

        self.cache.set(tool, &result)?;
        Ok(result)
    }
}
```

## Implementation Steps

1. Create team module structure
2. Implement config inheritance resolver
3. Add remote config fetching with caching
4. Implement config cache with TTL
5. Build team config registry with index.toml
6. Implement `jarvy team` commands
7. Create update module structure
8. Implement update checking logic
9. Implement `jarvy update` commands
10. Add auto-update configuration
11. Add update notifications to setup
12. Write unit tests for inheritance and updates
13. Write integration tests
14. Update documentation

## Dependencies

- `ureq` - HTTP client for remote configs (already present)
- `semver` - Version comparison (already present)

## Effort Estimate

| Task | Effort |
|------|--------|
| Team module structure | 0.5 days |
| Config inheritance | 2 days |
| Remote config caching | 1 day |
| Team config registry | 2 days |
| Team commands | 1 day |
| Update module | 0.5 days |
| Update checking | 1.5 days |
| Update commands | 1 day |
| Auto-update config | 0.5 days |
| Testing | 2 days |
| Documentation | 1 day |
| **Total** | **13 days** |

## Files to Create/Modify

### New Files
- `src/team/mod.rs`
- `src/team/inheritance.rs`
- `src/team/registry.rs`
- `src/team/cache.rs`
- `src/update/mod.rs`
- `src/update/check.rs`
- `src/update/config.rs`
- `src/commands/team.rs`
- `src/commands/update.rs`
- `tests/inheritance_integration.rs`
- `tests/update_integration.rs`

### Modified Files
- `src/main.rs` - Add team, update commands
- `src/config.rs` - Add extends and update fields
- `CLAUDE.md` - Document team and update features

## Success Metrics

| Metric | Current | Target |
|--------|---------|--------|
| Config sharing | Manual copy | Inheritance |
| Team config discovery | None | Registry |
| Tool update visibility | None | Update checks |
| Remote config support | None | URL extends |

## Risks

1. **Inheritance conflicts**: Deep merge can have unexpected results
   - Mitigation: Clear precedence rules, `--resolved` flag to inspect

2. **Offline remote configs**: Network issues block setup
   - Mitigation: Aggressive caching, offline fallback

3. **Update check performance**: Querying package managers can be slow
   - Mitigation: Cache update check results, background checking

4. **Cache invalidation**: Stale configs cause issues
   - Mitigation: Configurable TTL, manual sync command

5. **Auto-update breaking changes**: Tool updates may break workflows
   - Mitigation: Auto-update disabled by default, notifications instead

---

*PRD-024 v1.2 | Team & Enterprise Collaboration | Priority: High*
