//! Which paths inside an agent's config dir a snapshot skips.
//!
//! A profile captures *identity* — credentials, settings, skills, MCP
//! registrations, plugin selections. Agent config dirs also accumulate
//! conversation transcripts, re-downloadable package/extension trees and
//! log databases, which dominate the byte count without carrying any
//! config. On the machine this list was measured against, `~/.claude` +
//! `~/.codex` + `~/.cursor` totalled 2.6 GB against roughly 2 MB of
//! identity — snapshotting the lot made every profile a multi-gigabyte
//! copy, and `create --from` cloned one identity's whole conversation
//! history into another.
//!
//! This is a denylist, not an allowlist, on purpose: an unknown new file
//! in an agent dir must be *kept* (worst case, a profile is bigger than
//! it needs to be) rather than silently dropped (worst case, switching
//! profiles loses config the user cared about). Every entry below names
//! something observed on disk and says why it is not identity.

use crate::agents::Agent;

/// Relative paths (slash-separated, matched against the snapshot root)
/// that `copy_tree` skips. A pattern matches a path when it equals it or
/// is a parent directory of it. A trailing `*` on the final component
/// matches by prefix, which is how the sqlite sidecar files (`-wal`,
/// `-shm`) come along with their database.
pub fn excluded_paths(agent: Agent) -> &'static [&'static str] {
    match agent {
        Agent::ClaudeCode => &[
            // Per-project conversation transcripts. The single largest
            // consumer, and copying it into a second profile would clone
            // one identity's history into another.
            "projects",
            // Re-fetchable git clones of plugin/marketplace repos. The
            // sibling `plugins/*.json` files (which plugins are
            // installed) ARE identity and are deliberately kept.
            "plugins/cache",
            "plugins/marketplaces",
            // Edit-undo history and downloaded artifacts — rebuildable.
            "file-history",
            "downloads",
            // Diagnostics and per-session scratch state.
            "debug",
            "shell-snapshots",
            "paste-cache",
            "session-env",
            "todos",
            "tasks",
            "statsig",
            "telemetry",
            // Backups of the very dir being snapshotted.
            "backups",
            // Prompt history: user activity, not configuration.
            "history.jsonl",
            // Finder metadata.
            ".DS_Store",
        ],
        Agent::Codex => &[
            // Verified against codex-cli 0.135.0: `doctor` resolves
            // packages/ at the real ~/.codex even when CODEX_HOME points
            // elsewhere, so a copy in the profile is never read.
            "packages",
            "plugins/cache",
            "cache",
            "sessions",
            // Live IPC endpoints (also skipped by file type).
            "ipc",
            // Log database plus its WAL/SHM sidecars.
            "logs_2.sqlite*",
            // Scratch space, and by far the biggest thing left once the
            // package tree is gone. codex recreates `tmp/` under whatever
            // CODEX_HOME points at.
            ".tmp",
            "tmp",
            ".DS_Store",
        ],
        Agent::Cursor => &[
            // `extensions` is deliberately NOT excluded. Cursor is
            // symlink-tier: its snapshot *is* the live config dir, so a
            // profile without the extension tree is an editor with no
            // extensions — and nothing re-installs them. Every other entry
            // here is per-project state the agent regenerates on demand.
            // Per-project and per-worktree state, not identity.
            "projects",
            "worktrees",
            "ai-tracking",
            "plugins/cache",
            ".DS_Store",
        ],
        // Storage-only agents (not switchable until v1.1). No measured
        // layout yet — keep everything rather than guess.
        Agent::Windsurf | Agent::Cline | Agent::Continue => &[],
    }
}

/// The pattern that excludes `rel` (a path relative to the snapshot root,
/// slash-joined), or `None` when the path is kept. Returning the pattern
/// rather than a bool lets the `agent_profile.path_excluded` event name
/// which rule fired — without that, a rule that stops matching anything
/// can never be retired. Patterns are jarvy constants, so emitting one
/// is cardinality-safe in a way the path itself would not be.
pub fn matched_pattern(rel: &str, patterns: &[&'static str]) -> Option<&'static str> {
    patterns.iter().copied().find(|p| matches_pattern(rel, p))
}

/// True when `rel` is covered by one of `patterns`.
#[cfg(test)]
pub fn is_excluded(rel: &str, patterns: &[&'static str]) -> bool {
    matched_pattern(rel, patterns).is_some()
}

fn matches_pattern(rel: &str, pattern: &str) -> bool {
    let mut rel_parts = rel.split('/');
    let mut pat_parts = pattern.split('/').peekable();
    while let Some(pat) = pat_parts.next() {
        let Some(part) = rel_parts.next() else {
            // `rel` ran out first: it's an ancestor of the pattern, not a
            // match (we must still descend into it).
            return false;
        };
        let last = pat_parts.peek().is_none();
        let hit = match pat.strip_suffix('*') {
            Some(prefix) if last => part.starts_with(prefix),
            _ => part == pat,
        };
        if !hit {
            return false;
        }
    }
    // Every pattern component matched; any remaining `rel` components are
    // children of an excluded directory.
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_dir_and_its_children() {
        let pats = &["projects"];
        assert!(is_excluded("projects", pats));
        assert!(is_excluded("projects/foo/bar.jsonl", pats));
        assert!(!is_excluded("projectsx", pats));
        assert!(!is_excluded("settings.json", pats));
    }

    #[test]
    fn nested_pattern_keeps_siblings() {
        let pats = &["plugins/cache"];
        assert!(is_excluded("plugins/cache", pats));
        assert!(is_excluded("plugins/cache/repo/file", pats));
        // The identity half of plugins/ must survive.
        assert!(!is_excluded("plugins", pats));
        assert!(!is_excluded("plugins/installed_plugins.json", pats));
    }

    #[test]
    fn star_suffix_catches_sqlite_sidecars() {
        let pats = &["logs_2.sqlite*"];
        assert!(is_excluded("logs_2.sqlite", pats));
        assert!(is_excluded("logs_2.sqlite-wal", pats));
        assert!(is_excluded("logs_2.sqlite-shm", pats));
        assert!(!is_excluded("state_5.sqlite", pats));
    }

    /// Codex memories and state are identity — a switch that silently
    /// dropped them would lose user data.
    #[test]
    fn codex_keeps_identity_surface() {
        let pats = excluded_paths(Agent::Codex);
        for keep in [
            "auth.json",
            "config.toml",
            "AGENTS.md",
            "skills/foo/SKILL.md",
            "prompts/p.md",
            "memories_1.sqlite",
            "state_5.sqlite",
        ] {
            assert!(!is_excluded(keep, pats), "{keep} must be snapshotted");
        }
    }

    #[test]
    fn claude_keeps_identity_surface() {
        let pats = excluded_paths(Agent::ClaudeCode);
        for keep in [
            "settings.json",
            "CLAUDE.md",
            ".credentials.json",
            "skills/x/SKILL.md",
            "agents/a.md",
            "commands/c.md",
            "plugins/installed_plugins.json",
        ] {
            assert!(!is_excluded(keep, pats), "{keep} must be snapshotted");
        }
    }

    #[test]
    fn matched_pattern_names_the_rule_that_fired() {
        let pats: &[&'static str] = &["projects", "plugins/cache"];
        assert_eq!(matched_pattern("projects/a/b", pats), Some("projects"));
        assert_eq!(
            matched_pattern("plugins/cache/x", pats),
            Some("plugins/cache")
        );
        assert_eq!(matched_pattern("settings.json", pats), None);
    }

    /// Cursor is symlink-tier: the snapshot IS the live config dir, so
    /// dropping `extensions` would hand the user an extension-less editor
    /// with no path back.
    #[test]
    fn cursor_keeps_extensions() {
        let pats = excluded_paths(Agent::Cursor);
        assert!(!is_excluded("extensions", pats));
        assert!(!is_excluded("extensions/foo.bar/package.json", pats));
        // Per-project scratch is still dropped.
        assert!(is_excluded("worktrees/x", pats));
    }

    #[test]
    fn storage_only_agents_exclude_nothing() {
        assert!(excluded_paths(Agent::Windsurf).is_empty());
        assert!(excluded_paths(Agent::Cline).is_empty());
        assert!(excluded_paths(Agent::Continue).is_empty());
    }
}
