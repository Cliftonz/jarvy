//! `[ai_hooks]` schema for `jarvy.toml`.
//!
//! ```toml
//! [ai_hooks]
//! agents = ["claude-code", "cursor", "codex", "windsurf"]
//! scope = "user"                       # user | project
//! allow_custom_commands = false        # gate raw `command` entries
//!
//! [[ai_hooks.hook]]
//! use = "block-rm-rf"
//!
//! [[ai_hooks.hook]]
//! name = "block-force-push-main"
//! event = "pre_tool_use"
//! matcher = "Bash"
//! command = "..."                      # custom — refused unless allow_custom_commands
//! command_windows = "..."              # optional PowerShell variant
//! timeout_ms = 5000
//! ```

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::event::HookEvent;

/// Where a config came from. Drives the trust boundary: remote-fetched
/// configs can NARROW but not BROADEN policy.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum ConfigOrigin {
    /// Loaded from a local file on disk under the user's control.
    #[default]
    Local,
    /// Fetched from a remote URL (e.g. `jarvy setup --from <url>`). The
    /// runtime refuses to honor `allow_custom_commands = true` or raw
    /// `command` entries from this origin.
    Remote,
}

/// Every top-level `Config` sub-block that carries a `ConfigOrigin`
/// implements this so `Config::mark_remote` can iterate uniformly. Was
/// six copy-paste `if let Some(ref mut cfg) = self.X { cfg.origin =
/// ConfigOrigin::Remote; }` blocks that silently drifted twice pre-
/// review (skills and git_hooks were both missed).
///
/// Adding a new origin-bearing sub-config now requires one impl block
/// (2 lines) and one `tag(&mut self.X, o)` line in `mark_remote`. The
/// regression test `mark_remote_propagates_to_all_origin_bearing_subconfigs`
/// remains load-bearing — missing the tag call still compiles.
pub trait HasOrigin {
    fn set_origin(&mut self, origin: ConfigOrigin);
}

impl HasOrigin for AiHooksConfig {
    fn set_origin(&mut self, origin: ConfigOrigin) {
        self.origin = origin;
    }
}

/// Top-level `[ai_hooks]` block.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct AiHooksConfig {
    /// Which agents to provision. Empty = no-op.
    pub agents: Vec<AgentTarget>,

    /// Where to write hook configs. Default: `user`.
    pub scope: HookScope,

    /// If `false` (default), entries with a raw `command` field are refused
    /// unless they match a built-in library hook. Team configs SHOULD leave
    /// this false; individual developers can opt in locally.
    pub allow_custom_commands: bool,

    /// Hook entries — either `use = "library-name"` or fully inline.
    #[serde(rename = "hook", default)]
    pub hooks: Vec<HookEntry>,

    /// Third-party library sources (PRD-054). Each entry is fetched on
    /// `jarvy setup` / `jarvy ai-hooks apply` and its `ai_hook` items
    /// become resolvable via `use = "..."`. Local-origin only — the
    /// runtime refuses `library_sources` from remote-fetched configs
    /// (see `library_registry::check_origin`).
    #[serde(default)]
    pub library_sources: Vec<crate::library_registry::LibrarySource>,

    /// Where this config came from. Not serialized — populated by the
    /// loader (`Local` for `Config::new`, `Remote` for `--from <url>`).
    #[serde(skip)]
    pub origin: ConfigOrigin,
}

impl AiHooksConfig {
    /// Is there any work to do?
    pub fn is_empty(&self) -> bool {
        self.agents.is_empty() || self.hooks.is_empty()
    }

    /// Targets sorted + deduped for stable telemetry. Allocates — call
    /// once per `apply`, hoist before per-entry loops.
    pub fn unique_agents(&self) -> Vec<AgentTarget> {
        let set: BTreeSet<_> = self.agents.iter().copied().collect();
        set.into_iter().collect()
    }

    /// Bitset of `agents` for O(1) membership tests inside per-entry
    /// loops. Each variant fits in a single u8.
    pub fn agents_bitset(&self) -> u8 {
        self.agents
            .iter()
            .fold(0u8, |acc, a| acc | (1 << (*a as u8)))
    }
}

/// Each provisioned AI agent. Re-export of the canonical
/// [`crate::agents::Agent`] enum (review item 19) — historically a
/// per-subsystem enum lived here, but the three copies (here,
/// `mcp_register::McpAgentTarget`, `skills::SkillAgent`) carried the
/// same six variants and the same slug mapping with only per-subsystem
/// method bolt-ons differing. Consolidating to one type makes
/// cross-subsystem drift impossible (a Cursor variant added in one but
/// not another) while preserving every prior API (`ALL`, `COUNT`,
/// `slug`, `from_slug`, `Display`, kebab-case serde shape).
pub use crate::agents::Agent as AgentTarget;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HookScope {
    /// Write to the agent's user-level settings file (e.g. `~/.claude/settings.json`).
    #[default]
    User,
    /// Write to the agent's project-level settings file (e.g. `.claude/settings.json`).
    Project,
}

/// One hook entry from the user config.
///
/// Two forms:
///
/// * `use = "block-rm-rf"` — reference a built-in library hook.
/// * Inline `name` + `event` + `command` — raw shell, gated by
///   `allow_custom_commands` AND `ConfigOrigin::Local`.
///
/// Optional `agents` narrows the entry to a subset of the top-level
/// `agents` list (e.g. apply a hook only to Claude Code + Cursor).
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct HookEntry {
    /// Reference to a built-in library hook by name.
    #[serde(rename = "use")]
    pub use_library: Option<String>,

    /// Free-form name. Required when `use_library` is absent.
    pub name: Option<String>,

    /// Event the hook fires on. Required when `use_library` is absent.
    pub event: Option<HookEvent>,

    /// Tool-name matcher (agent-specific — e.g. "Bash", "Shell", "Edit").
    /// `None` = "all tools".
    pub matcher: Option<String>,

    /// Raw command. Refused unless `allow_custom_commands = true` AND
    /// the config is local.
    pub command: Option<String>,

    /// PowerShell variant for Windows targets (Windsurf/Codex commandWindows).
    pub command_windows: Option<String>,

    /// Per-hook timeout in milliseconds. Default: 5000.
    pub timeout_ms: Option<u64>,

    /// Restrict this entry to a subset of the top-level `agents` list.
    /// Empty = apply to all configured agents.
    #[serde(default)]
    pub agents: Vec<AgentTarget>,

    /// Optional parameters passed to library hooks (e.g. branch glob).
    #[serde(default)]
    pub params: toml::Table,
}

impl HookEntry {
    /// True when this entry references a library hook and does NOT
    /// override `command`.
    pub fn is_library(&self) -> bool {
        self.use_library.is_some() && self.command.is_none()
    }

    /// True when this entry has a raw shell command. The
    /// `use_library + command` combination is rejected by `resolve_one`
    /// outright so callers don't have to worry about it here.
    pub fn is_custom_command(&self) -> bool {
        self.command.is_some() && self.use_library.is_none()
    }

    /// Human-friendly identifier used in reports + as the `_jarvy_managed`
    /// marker in agent settings files.
    pub fn identifier(&self) -> String {
        if let Some(ref name) = self.name {
            name.clone()
        } else if let Some(ref lib) = self.use_library {
            lib.clone()
        } else {
            "unnamed".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_library_entry() {
        let toml = r#"
            agents = ["claude-code"]
            [[hook]]
            use = "block-rm-rf"
        "#;
        let cfg: AiHooksConfig = toml::from_str(toml).unwrap();
        assert_eq!(cfg.agents, vec![AgentTarget::ClaudeCode]);
        assert!(cfg.hooks[0].is_library());
        assert_eq!(cfg.hooks[0].identifier(), "block-rm-rf");
        assert_eq!(cfg.origin, ConfigOrigin::Local);
    }

    #[test]
    fn parses_inline_custom_entry() {
        let toml = r#"
            agents = ["cursor"]
            allow_custom_commands = true
            [[hook]]
            name = "warn-on-write"
            event = "pre_tool_use"
            matcher = "Bash"
            command = "echo hi"
        "#;
        let cfg: AiHooksConfig = toml::from_str(toml).unwrap();
        assert!(cfg.allow_custom_commands);
        assert!(cfg.hooks[0].is_custom_command());
    }

    #[test]
    fn agent_slug_round_trip() {
        for a in AgentTarget::ALL {
            assert_eq!(AgentTarget::from_slug(a.slug()), Some(*a));
        }
    }

    #[test]
    fn unique_agents_dedups_and_sorts() {
        let cfg = AiHooksConfig {
            agents: vec![
                AgentTarget::Cursor,
                AgentTarget::ClaudeCode,
                AgentTarget::Cursor,
            ],
            ..Default::default()
        };
        let unique = cfg.unique_agents();
        assert_eq!(unique, vec![AgentTarget::ClaudeCode, AgentTarget::Cursor]);
    }

    #[test]
    fn agents_bitset_membership() {
        let cfg = AiHooksConfig {
            agents: vec![AgentTarget::Cursor, AgentTarget::Cline],
            ..Default::default()
        };
        let bits = cfg.agents_bitset();
        assert_ne!(bits & (1 << AgentTarget::Cursor as u8), 0);
        assert_ne!(bits & (1 << AgentTarget::Cline as u8), 0);
        assert_eq!(bits & (1 << AgentTarget::ClaudeCode as u8), 0);
    }

    #[test]
    fn rejects_unknown_fields() {
        let toml = r#"
            agents = ["claude-code"]
            mystery = true
        "#;
        let err = toml::from_str::<AiHooksConfig>(toml).unwrap_err();
        assert!(err.to_string().contains("mystery"));
    }

    #[test]
    fn rejects_merge_strategy_field_now_deleted() {
        // Backstop: confirm we removed the dead `merge_strategy` field
        // entirely. If a future PR re-introduces it, this test fails
        // before the field can ship.
        let toml = r#"
            agents = ["claude-code"]
            merge_strategy = "replace"
        "#;
        assert!(toml::from_str::<AiHooksConfig>(toml).is_err());
    }

    #[test]
    fn library_entry_with_command_override_is_neither_library_nor_custom() {
        // Anti-bug guard. The runner explicitly rejects `use + command`
        // so neither classifier should claim it.
        let entry = HookEntry {
            use_library: Some("block-rm-rf".to_string()),
            command: Some("echo hi".to_string()),
            ..Default::default()
        };
        assert!(!entry.is_library());
        assert!(!entry.is_custom_command());
    }
}
