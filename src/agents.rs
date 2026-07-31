//! Canonical agent enum shared by every subsystem that targets the
//! six AI dev agents (Claude Code, Cursor, Codex, Windsurf, Cline,
//! Continue). Review item 19 (maint P1) — previously three near-
//! identical enums (`ai_hooks::AgentTarget`, `mcp_register::McpAgentTarget`,
//! `skills::SkillAgent`) carried the same six variants and the same
//! slug mapping, with only per-subsystem method bolt-ons differing.
//!
//! The merged shape exposes the superset of methods on one enum.
//! Each subsystem calls only the methods it needs; the maintainability
//! cost of an unused method is far smaller than the cost of
//! cross-subsystem drift (a Cursor variant added here but not there
//! is now impossible).
//!
//! The serde representation (`rename_all = "kebab-case"`) matches the
//! prior shapes byte-for-byte so existing `jarvy.toml` configs deserialise
//! unchanged. `#[repr(u8)]` matches the prior layout so the
//! `[T; Agent::COUNT]` fixed-size-array pattern used by
//! `ai_hooks::runner` keeps working.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(
    Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "kebab-case")]
#[repr(u8)]
pub enum Agent {
    #[default]
    ClaudeCode = 0,
    Cursor = 1,
    Codex = 2,
    Windsurf = 3,
    Cline = 4,
    Continue = 5,
}

/// How a profile switch is delivered for an agent (PRD-058).
///
/// `#[non_exhaustive]` because v1.1 adds a `GlobalStorage` tier for the
/// VS Code-family agents (IDE user-data `globalStorage` slice swap).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileMechanism {
    /// The agent honors an official env var pointing at an alternate
    /// config dir — per-terminal switching, nothing global changes.
    Env { var: &'static str },
    /// The agent's config dir is swapped globally by re-pointing a
    /// symlink at `~/.{agent}` into the profile store.
    Symlink,
}

impl Agent {
    /// Every variant in stable order. Used by setup loops, agent-flag
    /// completion, and as the iteration source for fixed-size-array
    /// indexing patterns (`[T; Agent::COUNT]`).
    pub const ALL: &'static [Agent] = &[
        Agent::ClaudeCode,
        Agent::Cursor,
        Agent::Codex,
        Agent::Windsurf,
        Agent::Cline,
        Agent::Continue,
    ];

    /// Number of variants. Held as a const so call sites can declare
    /// `[T; Agent::COUNT]` without pulling in a separate constant.
    pub const COUNT: usize = 6;

    /// Stable telemetry / CLI identifier. Used everywhere a string
    /// representation of the agent is needed — telemetry tags,
    /// `jarvy.toml` keys, CLI flag values.
    pub fn slug(self) -> &'static str {
        match self {
            Agent::ClaudeCode => "claude-code",
            Agent::Cursor => "cursor",
            Agent::Codex => "codex",
            Agent::Windsurf => "windsurf",
            Agent::Cline => "cline",
            Agent::Continue => "continue",
        }
    }

    /// Reverse of [`Self::slug`]. Case-insensitive match so a user
    /// typing `Cursor` resolves to `cursor`.
    pub fn from_slug(slug: &str) -> Option<Agent> {
        Self::ALL
            .iter()
            .copied()
            .find(|a| a.slug().eq_ignore_ascii_case(slug))
    }

    /// Whether this agent supports project-scope MCP-server
    /// registration. Windsurf, Cline, and Continue (in its current
    /// single-file mode) do not — registrars fall back to user scope
    /// with a warning when project is requested.
    ///
    /// Used by `mcp_register`; inert for `ai_hooks` / `skills`.
    /// Currently consulted only by the project-scope unit tests + the
    /// registrar fallback logic; reserved for future per-agent CLI
    /// flags that would surface "this agent doesn't support project
    /// scope, falling back to user" up-front.
    #[allow(dead_code)]
    pub fn supports_project_scope(self) -> bool {
        matches!(self, Agent::ClaudeCode | Agent::Cursor | Agent::Codex)
    }

    /// Agent's config directory under `$HOME` (or `JARVY_HOME` for
    /// tests). Returns `None` if home lookup fails.
    ///
    /// Used by `skills` to compute the per-agent `skills/` install path;
    /// also the proxy for "is this agent installed on this machine?"
    /// via [`Self::is_installed`].
    pub fn config_dir(self) -> Option<PathBuf> {
        let home = home_dir()?;
        Some(match self {
            Agent::ClaudeCode => home.join(".claude"),
            Agent::Cursor => home.join(".cursor"),
            Agent::Codex => home.join(".codex"),
            Agent::Windsurf => home.join(".windsurf"),
            Agent::Cline => home.join(".cline"),
            Agent::Continue => home.join(".continue"),
        })
    }

    /// Profile-switching mechanism(s) for this agent (PRD-058).
    ///
    /// Claude Code and Codex honor official config-dir env vars
    /// (`CLAUDE_CONFIG_DIR` / `CODEX_HOME`), so two terminals can run
    /// different profiles simultaneously. The rest only support a
    /// global symlink swap of the config dir itself.
    pub fn profile_mechanisms(self) -> &'static [ProfileMechanism] {
        match self {
            Agent::ClaudeCode => &[ProfileMechanism::Env {
                var: "CLAUDE_CONFIG_DIR",
            }],
            Agent::Codex => &[ProfileMechanism::Env { var: "CODEX_HOME" }],
            Agent::Cursor | Agent::Windsurf | Agent::Cline | Agent::Continue => {
                &[ProfileMechanism::Symlink]
            }
        }
    }

    /// Whether profile switching is supported for this agent (PRD-058).
    /// All six agents are switchable — env-tier agents (claude-code,
    /// codex) via config-dir env vars, symlink-tier agents (cursor,
    /// windsurf, cline, continue) via a global symlink swap of the
    /// dotdir. windsurf/cline/continue graduated from storage-only to
    /// switchable in the follow-up to PRD-058 v1; their denylists stay
    /// empty for now (empty denylist keeps everything — safe by the
    /// "denylist not allowlist" invariant).
    pub fn profile_switchable(self) -> bool {
        // Every current variant is switchable; keep the method as a
        // named boolean so status output / preference filters read as
        // intent instead of "true".
        matches!(
            self,
            Agent::ClaudeCode
                | Agent::Codex
                | Agent::Cursor
                | Agent::Windsurf
                | Agent::Cline
                | Agent::Continue
        )
    }

    /// Desktop-app process names to probe for a running IDE when a
    /// symlink-tier swap is about to re-point the live config dir. Empty
    /// for agents that run as in-editor extensions (cline / continue)
    /// or that expose no separate binary — their absence just means the
    /// probe returns `false`, i.e. never blocks a swap.
    ///
    /// Names are matched literally against `pgrep -x <name>` on Unix and
    /// `tasklist /FI "IMAGENAME eq <name>.exe"` on Windows (see
    /// `agent_profiles::probe`).
    pub fn desktop_binary_names(self) -> &'static [&'static str] {
        match self {
            // Cursor + Windsurf are Electron desktop apps; both spell
            // the process name with and without leading capital.
            Agent::Cursor => &["Cursor", "cursor"],
            Agent::Windsurf => &["Windsurf", "windsurf"],
            // Cline / Continue live inside another editor (VS Code /
            // JetBrains); no separate binary means the probe never
            // reports them as running, which is the correct fail-safe.
            Agent::Cline => &[],
            Agent::Continue => &[],
            // Env-tier agents don't need this — their swaps are
            // per-terminal and don't require a global re-point.
            _ => &[],
        }
    }

    /// Whether the agent's profile mechanism includes the symlink tier.
    pub fn is_symlink_tier(self) -> bool {
        self.profile_mechanisms()
            .iter()
            .any(|m| matches!(m, ProfileMechanism::Symlink))
    }

    /// Where skills land for this agent.
    pub fn skills_dir(self) -> Option<PathBuf> {
        self.config_dir().map(|p| p.join("skills"))
    }

    /// `true` when the agent's config directory exists on disk —
    /// proxy for "agent is installed."
    pub fn is_installed(self) -> bool {
        self.config_dir().map(|p| p.exists()).unwrap_or(false)
    }
}

impl std::fmt::Display for Agent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.slug())
    }
}

/// Honors `JARVY_HOME` for tests; otherwise standard `HOME` /
/// `USERPROFILE` lookup. Mirrors the prior helper that lived in
/// `skills::agents`. `pub(crate)` so `agent_profiles::switcher` can
/// enforce its stays-under-home boundary against the same root that
/// produced `config_dir()`.
pub(crate) fn home_dir() -> Option<PathBuf> {
    if let Some(v) = std::env::var_os("JARVY_HOME") {
        return Some(PathBuf::from(v));
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn slug_round_trips() {
        for a in Agent::ALL {
            assert_eq!(Agent::from_slug(a.slug()), Some(*a));
        }
    }

    #[test]
    fn from_slug_is_case_insensitive() {
        assert_eq!(Agent::from_slug("Cursor"), Some(Agent::Cursor));
        assert_eq!(Agent::from_slug("CLAUDE-CODE"), Some(Agent::ClaudeCode));
    }

    #[test]
    fn from_slug_unknown_returns_none() {
        assert_eq!(Agent::from_slug("zed"), None);
    }

    #[test]
    fn supports_project_scope_matches_prior_matrix() {
        assert!(Agent::ClaudeCode.supports_project_scope());
        assert!(Agent::Cursor.supports_project_scope());
        assert!(Agent::Codex.supports_project_scope());
        assert!(!Agent::Windsurf.supports_project_scope());
        assert!(!Agent::Cline.supports_project_scope());
        assert!(!Agent::Continue.supports_project_scope());
    }

    #[test]
    fn profile_mechanisms_match_prd_058_table() {
        assert_eq!(
            Agent::ClaudeCode.profile_mechanisms(),
            &[ProfileMechanism::Env {
                var: "CLAUDE_CONFIG_DIR"
            }]
        );
        assert_eq!(
            Agent::Codex.profile_mechanisms(),
            &[ProfileMechanism::Env { var: "CODEX_HOME" }]
        );
        for agent in [
            Agent::Cursor,
            Agent::Windsurf,
            Agent::Cline,
            Agent::Continue,
        ] {
            assert_eq!(agent.profile_mechanisms(), &[ProfileMechanism::Symlink]);
        }
    }

    #[test]
    fn profile_switchable_covers_all_agents() {
        // windsurf / cline / continue graduated from storage-only to
        // switchable — every agent now returns true. The method is kept
        // as a named boolean so callers still read intent, not a bare
        // literal.
        for agent in Agent::ALL {
            assert!(
                agent.profile_switchable(),
                "{} must be profile-switchable",
                agent.slug()
            );
        }
    }

    #[test]
    fn desktop_binary_names_covers_symlink_desktop_apps_only() {
        assert_eq!(Agent::Cursor.desktop_binary_names(), &["Cursor", "cursor"]);
        assert_eq!(
            Agent::Windsurf.desktop_binary_names(),
            &["Windsurf", "windsurf"]
        );
        // In-editor extensions: no dedicated binary, probe reports false.
        assert!(Agent::Cline.desktop_binary_names().is_empty());
        assert!(Agent::Continue.desktop_binary_names().is_empty());
        // Env-tier: probe isn't relevant, empty is correct.
        assert!(Agent::ClaudeCode.desktop_binary_names().is_empty());
        assert!(Agent::Codex.desktop_binary_names().is_empty());
    }

    #[test]
    fn is_symlink_tier_matches_mechanism_table() {
        assert!(!Agent::ClaudeCode.is_symlink_tier());
        assert!(!Agent::Codex.is_symlink_tier());
        assert!(Agent::Cursor.is_symlink_tier());
        assert!(Agent::Windsurf.is_symlink_tier());
        assert!(Agent::Cline.is_symlink_tier());
        assert!(Agent::Continue.is_symlink_tier());
    }

    #[test]
    fn count_matches_all_len() {
        assert_eq!(Agent::COUNT, Agent::ALL.len());
    }

    #[test]
    fn display_matches_slug() {
        assert_eq!(format!("{}", Agent::ClaudeCode), "claude-code");
        assert_eq!(format!("{}", Agent::Continue), "continue");
    }

    #[test]
    fn serde_kebab_case_round_trip() {
        let raw = "\"claude-code\"";
        let a: Agent = serde_json::from_str(raw).unwrap();
        assert_eq!(a, Agent::ClaudeCode);
        assert_eq!(serde_json::to_string(&a).unwrap(), raw);
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn config_dir_honors_jarvy_home() {
        // SAFETY: scoped JARVY_HOME for this test only.
        #[allow(unsafe_code)]
        unsafe {
            let tmp = tempdir().unwrap();
            std::env::set_var("JARVY_HOME", tmp.path());
            let dir = Agent::ClaudeCode.config_dir().unwrap();
            assert_eq!(dir, tmp.path().join(".claude"));
            assert!(!Agent::ClaudeCode.is_installed());
            std::fs::create_dir(&dir).unwrap();
            assert!(Agent::ClaudeCode.is_installed());
            std::env::remove_var("JARVY_HOME");
        }
    }
}
