//! AI agent skill installation (PRD-049 v1, riding on PRD-054 library
//! registry).
//!
//! Skills are markdown files (`SKILL.md` + optional companions) that
//! live under each AI coding agent's config directory
//! (`~/.claude/skills/`, `~/.cursor/skills/`, etc.). Jarvy installs them
//! by fetching the `skill_md_url` published in a library manifest,
//! sha256-verifying against the manifest entry, and writing to every
//! detected agent's skill directory.
//!
//! # Shipped (v1 + PRD-049 phase 2)
//!
//! - `[skills]` config block with `library_sources` + `install` list
//! - `jarvy skills {install, update, remove, list, status, agents}`
//!   subcommands (update/remove/ad-hoc install landed in phase 2)
//! - Ad-hoc `jarvy skills install <name>` — resolves a skill from
//!   library_sources at `latest` without a `[skills.install]` entry
//! - Auto-install during `jarvy setup` (gated on
//!   `[skills] auto_install = true`)
//! - Agent detection (claude-code, cursor, codex, windsurf, cline,
//!   continue — same set as ai_hooks)
//! - sha256 verification of fetched `SKILL.md`
//!
//! # Still open (PRD-049 follow-up)
//!
//! - skills.sh API integration (search / popular / info commands)
//! - Companion file fetching (only `SKILL.md` lands today)
//! - Project-scope skills (only `~/.agent/skills/` user scope)
//! - Version-range pinning (only exact or `"latest"`)

pub mod agents;
pub mod config;
pub mod installer;

pub use agents::{SkillAgent, detect_agents};
pub use config::{SkillEntry, SkillsConfig};
#[allow(unused_imports)] // Public lib API; bin only uses install_skill + SkillStatus directly
pub use installer::{InstallResult, RemoveResult, SkillError, UpdateResult};
pub use installer::{SkillStatus, install_skill, remove_skill, update_skill};
