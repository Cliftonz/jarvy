//! Skill-drop mode: write a `jarvy-setup` `SKILL.md` to one agent's
//! skills dir and tell the user to invoke it from inside the agent.
//!
//! Reuses the per-agent install paths defined on `Agent` (the same
//! ones used by `src/skills/installer.rs` for the existing skills
//! registry). The skill body is bundled into the binary via
//! `include_str!` so a fresh install has zero network dependency.

use crate::agents::Agent;
use std::fs;
use std::io;
use std::path::PathBuf;

/// Skill body shipped with the binary. Single template for all 6
/// agents — Claude Code's skill loader, Cursor's rules surface, and
/// the rest each read Markdown with frontmatter, so one document
/// covers them.
pub const SKILL_BODY: &str = include_str!("../../assets/wizard-skill/SKILL.md");

/// Stable skill identifier — also the directory name under each
/// agent's `skills/` root.
pub const SKILL_NAME: &str = "jarvy-setup";

#[derive(Debug, thiserror::Error)]
pub enum SkillDropError {
    /// The agent variant doesn't have a defined skills dir on this
    /// platform (rare; current `Agent` enum covers all six).
    #[error("agent `{agent}` has no skills dir on this platform")]
    NoSkillsDir { agent: String },

    /// IO failure writing the SKILL.md or its parent dirs.
    #[error("write {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

/// Compute the full install path for a given agent. Format:
/// `<agent_skills_dir>/jarvy-setup/SKILL.md`. Reuses `Agent::skills_dir()`
/// so the destination matches what the existing skills CLI shows.
pub fn install_path(agent: Agent) -> Result<PathBuf, SkillDropError> {
    let dir = agent
        .skills_dir()
        .ok_or_else(|| SkillDropError::NoSkillsDir {
            agent: agent.slug().to_string(),
        })?;
    Ok(dir.join(SKILL_NAME).join("SKILL.md"))
}

/// Write the bundled SKILL.md to the agent's skills dir. Creates
/// parent dirs if missing. Overwrite is unconditional — the skill
/// is a single source of truth shipped with the binary; users
/// shouldn't be hand-editing it (any divergence would silently
/// drift from the MCP tool surface the skill documents).
pub fn install(agent: Agent) -> Result<PathBuf, SkillDropError> {
    let path = install_path(agent)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| SkillDropError::Write {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }
    fs::write(&path, SKILL_BODY).map_err(|e| SkillDropError::Write {
        path: path.clone(),
        source: e,
    })?;
    Ok(path)
}

/// One-liner the user can paste into their agent to trigger the
/// skill. Centralized so the CLI output and the help docs stay in
/// sync.
pub fn invocation_phrase(agent: Agent) -> &'static str {
    match agent {
        // Claude Code's skill loader matches on natural-language hints
        // in user messages; the phrase is intentionally close to what
        // SKILL.md says it activates on.
        Agent::ClaudeCode => "set up jarvy for this project",
        Agent::Cursor | Agent::Windsurf | Agent::Cline | Agent::Continue => {
            "set up jarvy for this project"
        }
        Agent::Codex => "set up jarvy for this project",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_path_contains_skill_name() {
        for &agent in Agent::ALL {
            let p = install_path(agent).expect("every agent has a skills_dir");
            let s = p.to_string_lossy().into_owned();
            assert!(s.contains(SKILL_NAME), "path missing skill name: {s}");
            assert!(
                s.ends_with("SKILL.md"),
                "path doesn't end with SKILL.md: {s}"
            );
        }
    }

    #[test]
    fn skill_body_has_frontmatter() {
        // Pinned: the bundled skill must start with a YAML frontmatter
        // block so Claude Code's loader (and the others' Markdown
        // parsers) pick up the `name` and `description` fields.
        assert!(
            SKILL_BODY.starts_with("---\n"),
            "SKILL.md must start with YAML frontmatter"
        );
        assert!(
            SKILL_BODY.contains("name: jarvy-setup"),
            "SKILL.md frontmatter must declare name: jarvy-setup"
        );
    }
}
