# PRD-049: Skills Registry Integration

## Overview

Enable Jarvy to install, manage, and configure AI agent skills from [skills.sh](https://skills.sh) and other skill registries. This extends Jarvy's environment provisioning to include AI agent capabilities, ensuring consistent agentic workflows across team members.

## Problem Statement

AI coding agents (Claude Code, Cursor, Copilot, Codex) are becoming essential development tools, but skills management is fragmented:

- Developers manually clone skill repositories and configure paths
- Skill configurations differ between team members
- No centralized way to define required skills for a project
- Skills are often project-specific but not versioned with the project
- Onboarding requires manual skill installation instructions
- No visibility into what skills are available or installed

Jarvy already provisions development environments. Extending this to AI agent skills creates a unified setup experience.

## Evidence

- skills.sh tracks 500+ skills with growing adoption
- Major tools adopting SKILL.md format (Claude Code, Codex, Cursor, Cline)
- Teams creating private skill registries for proprietary workflows
- Repeated "install these skills" instructions in onboarding docs
- Skills drift between developers causing inconsistent agent behavior

## Requirements

### Functional Requirements

1. **Skill installation**: Install skills from skills.sh and GitHub repositories
2. **Skill listing**: List available and installed skills
3. **Skill search**: Search skills.sh registry by keyword
4. **Skill removal**: Uninstall skills cleanly
5. **Project skills**: Define required skills in jarvy.toml
6. **Agent detection**: Auto-detect installed AI agents
7. **Multi-agent**: Install skills for multiple agents
8. **Versioning**: Pin skills to specific versions/commits

### Non-Functional Requirements

1. **Idempotent**: Safe to run multiple times
2. **Fast**: Skill installation <5 seconds for cached skills
3. **Offline capable**: Use cached skills when offline
4. **Non-destructive**: Don't overwrite user-modified skills
5. **Privacy-respecting**: No telemetry on skill usage by default

## Non-Goals

- Creating or authoring skills (use existing tooling)
- Skill execution or invocation (agent responsibility)
- Agent configuration beyond skill installation
- Skill marketplace/monetization features
- Building a competing skills.sh registry

## Feature Specifications

### 1. Configuration Syntax

```toml
# jarvy.toml

[skills]
# Auto-install skills during setup
auto_install = true

# Target agents (auto-detected if not specified)
agents = ["claude-code", "cursor"]

# Cache directory for skill repos
cache_dir = "~/.jarvy/skills-cache"

# Skills to install (from skills.sh)
install = [
    "anthropics/claude-code-skills/prompt-engineering",
    "vercel/ai-sdk-skills/nextjs",
    "myorg/internal-skills/code-review",
]

# Skills with version pinning
[skills.pinned]
"anthropics/claude-code-skills/prompt-engineering" = "v1.2.0"
"myorg/internal-skills" = "abc1234"  # commit hash

# Private registry support
[skills.registries]
internal = "https://skills.internal.corp.com"

# Skill from private registry
[skills.sources]
"corp-standards" = { registry = "internal", path = "standards/rust" }
```

### 2. CLI Commands

```bash
# Install a skill (from skills.sh registry)
jarvy skills install anthropics/claude-code-skills/prompt-engineering

# Output:
# Installing skill: anthropics/claude-code-skills/prompt-engineering
#   Fetching from skills.sh registry...
#   ✓ Skill downloaded
#   Detected agents: claude-code, cursor
#   Installing to:
#     ~/.claude/skills/prompt-engineering
#     ~/.cursor/skills/prompt-engineering
#   ✓ Skill installed for 2 agents

# Install skill with version
jarvy skills install anthropics/claude-code-skills@v1.2.0

# Install from GitHub directly
jarvy skills install github:myorg/my-skills/custom-skill

# Install from local path
jarvy skills install ./local-skills/my-skill

# Install for specific agent only
jarvy skills install anthropics/claude-code-skills --agent cursor

# List installed skills
jarvy skills list

# Output:
# Installed Skills
# ================
# Agent: claude-code (~/.claude/skills/)
#   prompt-engineering    anthropics/claude-code-skills  v1.2.0
#   nextjs               vercel/ai-sdk-skills           latest
#   code-review          myorg/internal-skills          abc1234
#
# Agent: cursor (~/.cursor/skills/)
#   prompt-engineering    anthropics/claude-code-skills  v1.2.0
#   nextjs               vercel/ai-sdk-skills           latest

# List available skills from registry
jarvy skills available

# Output:
# Popular Skills (skills.sh)
# ==========================
# anthropics/claude-code-skills     ⬇ 12.5k   Official Claude Code skills
#   /prompt-engineering             ⬇ 8.2k    Prompt engineering best practices
#   /debugging                      ⬇ 6.1k    Advanced debugging workflows
#   /testing                        ⬇ 4.8k    Test-first development
# vercel/ai-sdk-skills             ⬇ 9.8k    Vercel AI SDK skills
#   /nextjs                        ⬇ 7.2k    Next.js development
#   /react                         ⬇ 5.1k    React best practices

# Search skills
jarvy skills search "rust testing"

# Output:
# Search Results: "rust testing"
# ==============================
# rust-lang/skills/testing        ⬇ 3.2k   Rust testing patterns
# myorg/rust-skills/integration   ⬇ 892    Integration testing for Rust
# community/rust-tdd              ⬇ 567    TDD workflow for Rust projects

# Show skill details
jarvy skills info anthropics/claude-code-skills/prompt-engineering

# Output:
# Skill: prompt-engineering
# =========================
# Repository: anthropics/claude-code-skills
# Path: /prompt-engineering
# Description: Best practices for prompt engineering with Claude
# Version: v1.2.0 (latest)
# Downloads: 8,234
#
# Supported Agents:
#   ✓ Claude Code
#   ✓ Cursor
#   ✓ Codex CLI
#   ✓ Cline
#
# SKILL.md Preview:
# ---
# name: prompt-engineering
# description: Prompt engineering best practices for Claude
# globs: ["**/*.md", "**/*.txt"]
# ---
#
# # Prompt Engineering Guidelines
# ...

# Remove a skill
jarvy skills remove prompt-engineering

# Output:
# Removing skill: prompt-engineering
#   Removing from claude-code...
#   Removing from cursor...
#   ✓ Skill removed from 2 agents

# Update skills to latest versions
jarvy skills update

# Output:
# Updating skills...
#   prompt-engineering: v1.1.0 → v1.2.0
#   nextjs: up to date
#   ✓ 1 skill updated

# Check skill status
jarvy skills status

# Output:
# Skills Status
# =============
# Defined in jarvy.toml: 3
# Installed: 3
# Up to date: 2
# Updates available: 1
#   prompt-engineering: v1.1.0 → v1.2.0 available

# Detect installed agents
jarvy skills agents

# Output:
# Detected AI Agents
# ==================
# ✓ Claude Code    ~/.claude/          v1.0.0
# ✓ Cursor         ~/.cursor/          v0.45.0
# ✗ Codex CLI      Not installed
# ✗ Cline          Not installed
# ✗ GitHub Copilot Not installed
```

### 3. Setup Integration

```bash
# During jarvy setup
jarvy setup

# Output:
# ...tool installation...
# ...hook installation...
#
# AI Agent Skills
# ===============
# Detected agents: claude-code, cursor
# Installing 3 skills from jarvy.toml...
#   ✓ prompt-engineering (anthropics/claude-code-skills)
#   ✓ nextjs (vercel/ai-sdk-skills)
#   ✓ code-review (myorg/internal-skills)
#
# Skills installed to:
#   Claude Code: ~/.claude/skills/
#   Cursor: ~/.cursor/skills/
#
# Setup complete!
```

### 4. Agent Detection

```rust
// Detect installed AI coding agents
enum AIAgent {
    ClaudeCode,     // ~/.claude/ or .claude/
    Cursor,         // ~/.cursor/
    CodexCLI,       // ~/.codex/
    Cline,          // ~/.cline/
    GitHubCopilot,  // ~/.copilot/
    Windsurf,       // ~/.windsurf/
    Zed,            // ~/.zed/
}

// Agent-specific skill paths
fn skill_path(agent: AIAgent, scope: SkillScope) -> PathBuf {
    match (agent, scope) {
        (AIAgent::ClaudeCode, SkillScope::Global) => home_dir().join(".claude/skills/"),
        (AIAgent::ClaudeCode, SkillScope::Project) => PathBuf::from(".claude/skills/"),
        (AIAgent::Cursor, SkillScope::Global) => home_dir().join(".cursor/skills/"),
        (AIAgent::Cursor, SkillScope::Project) => PathBuf::from(".cursor/skills/"),
        // ... etc
    }
}
```

### 5. Skills.sh API Integration

```rust
// skills.sh API client
pub struct SkillsRegistry {
    base_url: String,  // https://skills.sh
    cache_dir: PathBuf,
}

impl SkillsRegistry {
    /// Search skills by keyword
    pub async fn search(&self, query: &str) -> Result<Vec<SkillSummary>, RegistryError> {
        // GET https://skills.sh/api/search?q={query}
    }

    /// Get skill details
    pub async fn info(&self, skill_id: &str) -> Result<SkillDetails, RegistryError> {
        // GET https://skills.sh/api/skills/{owner}/{repo}/{path}
    }

    /// Download skill files
    pub async fn download(&self, skill_id: &str, version: Option<&str>) -> Result<PathBuf, RegistryError> {
        // Clone or fetch from GitHub
        // Cache locally
    }

    /// List popular/trending skills
    pub async fn popular(&self) -> Result<Vec<SkillSummary>, RegistryError> {
        // GET https://skills.sh/api/popular
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SkillSummary {
    pub id: String,           // "anthropics/claude-code-skills/prompt-engineering"
    pub name: String,         // "prompt-engineering"
    pub owner: String,        // "anthropics"
    pub repo: String,         // "claude-code-skills"
    pub description: String,
    pub downloads: u64,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SkillDetails {
    #[serde(flatten)]
    pub summary: SkillSummary,
    pub readme: String,
    pub skill_md: String,
    pub supported_agents: Vec<String>,
    pub versions: Vec<VersionInfo>,
}
```

## Technical Approach

### Module Structure

```
src/
  skills/
    mod.rs           # Public API
    config.rs        # Skills configuration parsing
    registry.rs      # skills.sh API client
    installer.rs     # Skill installation logic
    agents.rs        # Agent detection
    cache.rs         # Local skill caching
    commands.rs      # CLI command handlers
    skill.rs         # Skill types and SKILL.md parsing
```

### Configuration Types

```rust
// src/skills/config.rs
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct SkillsConfig {
    /// Auto-install skills during setup
    #[serde(default = "default_true")]
    pub auto_install: bool,

    /// Target agents (auto-detected if empty)
    #[serde(default)]
    pub agents: Vec<String>,

    /// Cache directory
    #[serde(default = "default_cache_dir")]
    pub cache_dir: String,

    /// Skills to install (shorthand format)
    #[serde(default)]
    pub install: Vec<String>,

    /// Skills with version pins
    #[serde(default)]
    pub pinned: HashMap<String, String>,

    /// Custom registries
    #[serde(default)]
    pub registries: HashMap<String, String>,

    /// Skills from custom sources
    #[serde(default)]
    pub sources: HashMap<String, SkillSource>,
}

fn default_true() -> bool {
    true
}

fn default_cache_dir() -> String {
    "~/.jarvy/skills-cache".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SkillSource {
    /// Registry name (from registries section)
    pub registry: Option<String>,
    /// Path within the registry
    pub path: Option<String>,
    /// Direct GitHub URL
    pub github: Option<String>,
    /// Local path
    pub local: Option<String>,
    /// Version/commit
    pub version: Option<String>,
}
```

### Skill Installation Flow

```rust
// src/skills/installer.rs
use std::path::PathBuf;

pub struct SkillInstaller {
    registry: SkillsRegistry,
    cache: SkillCache,
}

impl SkillInstaller {
    /// Install a skill to all detected agents
    pub async fn install(
        &self,
        skill_id: &str,
        version: Option<&str>,
        agents: &[AIAgent],
    ) -> Result<InstallResult, SkillError> {
        // 1. Parse skill ID (owner/repo/path or github:url or ./local)
        let skill_ref = SkillRef::parse(skill_id)?;

        // 2. Download/fetch skill files
        let skill_path = match &skill_ref {
            SkillRef::Registry { owner, repo, path } => {
                self.registry.download(skill_id, version).await?
            }
            SkillRef::GitHub(url) => {
                self.clone_github(url, version).await?
            }
            SkillRef::Local(path) => {
                path.clone()
            }
        };

        // 3. Validate SKILL.md exists
        let skill_md = skill_path.join("SKILL.md");
        if !skill_md.exists() {
            return Err(SkillError::InvalidSkill("No SKILL.md found".to_string()));
        }

        // 4. Parse SKILL.md for metadata
        let skill = Skill::parse(&skill_md)?;

        // 5. Install to each agent
        let mut results = Vec::new();
        for agent in agents {
            let dest = agent.skill_path(SkillScope::Global).join(&skill.name);

            // Check if already installed
            if dest.exists() {
                // Check version
                if let Some(installed_version) = self.get_installed_version(&dest)? {
                    if version.map_or(true, |v| installed_version == v) {
                        results.push((*agent, InstallStatus::AlreadyInstalled));
                        continue;
                    }
                }
            }

            // Copy skill files
            self.copy_skill(&skill_path, &dest)?;

            // Record installation metadata
            self.record_install(&dest, skill_id, version)?;

            results.push((*agent, InstallStatus::Installed));
        }

        Ok(InstallResult { skill, results })
    }

    /// Copy skill directory to destination
    fn copy_skill(&self, src: &Path, dest: &Path) -> Result<(), SkillError> {
        if dest.exists() {
            std::fs::remove_dir_all(dest)?;
        }

        copy_dir_all(src, dest)?;
        Ok(())
    }

    /// Record installation metadata for updates
    fn record_install(
        &self,
        dest: &Path,
        skill_id: &str,
        version: Option<&str>,
    ) -> Result<(), SkillError> {
        let meta = InstallMeta {
            skill_id: skill_id.to_string(),
            version: version.map(String::from),
            installed_at: Utc::now(),
        };

        let meta_path = dest.join(".jarvy-skill.json");
        let content = serde_json::to_string_pretty(&meta)?;
        std::fs::write(meta_path, content)?;
        Ok(())
    }
}
```

### Agent Detection

```rust
// src/skills/agents.rs
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AIAgent {
    ClaudeCode,
    Cursor,
    CodexCLI,
    Cline,
    GitHubCopilot,
    Windsurf,
    Zed,
    AiderChat,
}

impl AIAgent {
    pub fn detect_all() -> Vec<AIAgent> {
        let mut agents = Vec::new();

        for agent in [
            AIAgent::ClaudeCode,
            AIAgent::Cursor,
            AIAgent::CodexCLI,
            AIAgent::Cline,
            AIAgent::GitHubCopilot,
            AIAgent::Windsurf,
            AIAgent::Zed,
            AIAgent::AiderChat,
        ] {
            if agent.is_installed() {
                agents.push(agent);
            }
        }

        agents
    }

    pub fn is_installed(&self) -> bool {
        self.config_dir().map(|p| p.exists()).unwrap_or(false)
    }

    pub fn config_dir(&self) -> Option<PathBuf> {
        let home = dirs::home_dir()?;
        Some(match self {
            AIAgent::ClaudeCode => home.join(".claude"),
            AIAgent::Cursor => home.join(".cursor"),
            AIAgent::CodexCLI => home.join(".codex"),
            AIAgent::Cline => home.join(".cline"),
            AIAgent::GitHubCopilot => home.join(".copilot"),
            AIAgent::Windsurf => home.join(".windsurf"),
            AIAgent::Zed => home.join(".zed"),
            AIAgent::AiderChat => home.join(".aider"),
        })
    }

    pub fn skill_path(&self, scope: SkillScope) -> PathBuf {
        match scope {
            SkillScope::Global => {
                self.config_dir()
                    .map(|p| p.join("skills"))
                    .unwrap_or_default()
            }
            SkillScope::Project => {
                let prefix = match self {
                    AIAgent::ClaudeCode => ".claude",
                    AIAgent::Cursor => ".cursor",
                    AIAgent::CodexCLI => ".codex",
                    AIAgent::Cline => ".cline",
                    AIAgent::GitHubCopilot => ".copilot",
                    AIAgent::Windsurf => ".windsurf",
                    AIAgent::Zed => ".zed",
                    AIAgent::AiderChat => ".aider",
                };
                PathBuf::from(prefix).join("skills")
            }
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            AIAgent::ClaudeCode => "claude-code",
            AIAgent::Cursor => "cursor",
            AIAgent::CodexCLI => "codex",
            AIAgent::Cline => "cline",
            AIAgent::GitHubCopilot => "copilot",
            AIAgent::Windsurf => "windsurf",
            AIAgent::Zed => "zed",
            AIAgent::AiderChat => "aider",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            AIAgent::ClaudeCode => "Claude Code",
            AIAgent::Cursor => "Cursor",
            AIAgent::CodexCLI => "Codex CLI",
            AIAgent::Cline => "Cline",
            AIAgent::GitHubCopilot => "GitHub Copilot",
            AIAgent::Windsurf => "Windsurf",
            AIAgent::Zed => "Zed",
            AIAgent::AiderChat => "Aider",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillScope {
    Global,   // ~/.agent/skills/
    Project,  // .agent/skills/
}
```

### SKILL.md Parsing

```rust
// src/skills/skill.rs
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Skill {
    /// Skill identifier (from SKILL.md frontmatter)
    pub name: String,

    /// Description
    pub description: String,

    /// File patterns this skill applies to
    #[serde(default)]
    pub globs: Vec<String>,

    /// Trigger keywords/phrases
    #[serde(default)]
    pub triggers: Vec<String>,

    /// Skill content (markdown instructions)
    #[serde(skip)]
    pub content: String,
}

impl Skill {
    pub fn parse(path: &Path) -> Result<Self, SkillError> {
        let content = std::fs::read_to_string(path)?;

        // Parse YAML frontmatter between --- markers
        let parts: Vec<&str> = content.splitn(3, "---").collect();
        if parts.len() < 3 {
            return Err(SkillError::InvalidSkill("No YAML frontmatter".to_string()));
        }

        let frontmatter = parts[1].trim();
        let mut skill: Skill = serde_yaml::from_str(frontmatter)?;
        skill.content = parts[2].trim().to_string();

        Ok(skill)
    }
}
```

## Implementation Steps

1. Create skills module structure
2. Implement SkillsConfig parsing
3. Implement AI agent detection
4. Implement skills.sh API client
5. Implement skill caching
6. Implement SKILL.md parsing
7. Implement skill installation
8. Integrate with setup command
9. Implement `skills install` command
10. Implement `skills list` command
11. Implement `skills search` command
12. Implement `skills info` command
13. Implement `skills remove` command
14. Implement `skills update` command
15. Implement `skills status` command
16. Implement `skills agents` command
17. Add version pinning support
18. Add private registry support
19. Write tests
20. Update documentation

## Success Metrics

| Metric | Current | Target |
|--------|---------|--------|
| Manual skill installation | 100% | <5% |
| Skill consistency across team | Low | High |
| Time to install skills | 5-10 min | <30 sec |
| Discoverability of skills | Poor | Excellent |
| Skill version tracking | None | Full |

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| skills.sh API changes | Medium | Medium | Abstract behind interface, handle gracefully |
| Agent paths change | Low | Medium | Make paths configurable, detect at runtime |
| Network unavailable | Medium | Low | Use cached skills, clear error messages |
| Private repos auth | Medium | Low | Support SSH keys, tokens |
| Large skill repos | Low | Low | Clone sparse checkout for skill path only |
| SKILL.md format evolution | Medium | Low | Version-aware parsing |

## Dependencies

### New Dependencies
- `reqwest` - HTTP client for skills.sh API (already in project)
- `serde_yaml` - Parse SKILL.md frontmatter
- `git2` - Git operations for cloning (or use git CLI)

### Existing Dependencies
- `serde` - Configuration parsing
- `tokio` - Async runtime
- `dirs` - Home directory detection

## Effort Estimate

| Task | Effort |
|------|--------|
| Module structure and config | 0.5 days |
| Agent detection | 0.5 days |
| skills.sh API client | 1 day |
| Skill caching | 0.5 days |
| SKILL.md parsing | 0.5 days |
| Skill installation logic | 1 day |
| Setup integration | 0.5 days |
| skills install command | 0.5 days |
| skills list command | 0.25 days |
| skills search command | 0.5 days |
| skills info command | 0.25 days |
| skills remove command | 0.25 days |
| skills update command | 0.5 days |
| skills status command | 0.25 days |
| skills agents command | 0.25 days |
| Version pinning | 0.5 days |
| Private registry support | 0.5 days |
| Testing | 1.5 days |
| Documentation | 0.5 days |
| **Total** | **10 days** |

## Files to Create/Modify

### New Files
- `src/skills/mod.rs`
- `src/skills/config.rs`
- `src/skills/registry.rs`
- `src/skills/installer.rs`
- `src/skills/agents.rs`
- `src/skills/cache.rs`
- `src/skills/commands.rs`
- `src/skills/skill.rs`
- `src/commands/skills_cmd.rs`
- `tests/skills_integration.rs`

### Modified Files
- `src/config.rs` - Add skills config parsing
- `src/lib.rs` - Export skills module
- `src/main.rs` - Add skills subcommand
- `src/cli/subcommands.rs` - Add SkillsAction enum
- `src/commands/setup_cmd.rs` - Integrate skill installation
- `Cargo.toml` - Add serde_yaml if not present
- `CLAUDE.md` - Document [skills] section

## Related PRDs

- PRD-048: Pre-Commit Hook Installation (similar integration pattern)
- PRD-025: Docker/MCP Catalog (registry integration pattern)

---

*PRD-049 v1.0 | Skills Registry Integration | Priority: Medium*

Sources:
- [skills.sh](https://skills.sh/) - The Open Agent Skills Ecosystem
- [Vercel add-skill](https://github.com/vercel-labs/add-skill) - Open agent skills tool
- [Claude Code Skills Docs](https://code.claude.com/docs/en/skills) - Official skill documentation
- [Anthropic Skills Repository](https://github.com/anthropics/skills) - Public agent skills
