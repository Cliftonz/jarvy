//! Codex - OpenAI's CLI coding agent
//!
//! `codex` is OpenAI's terminal-based coding agent, a peer of Claude
//! Code for agentic editing, review, and exec-style automation
//! (`codex exec`). Jarvy distributes ai_hooks, skills, and MCP config
//! to Codex; this definition installs the CLI itself.

use crate::define_tool;

define_tool!(CODEX, {
    command: "codex",
    // No brew formula exists for Codex as of 2026-08; the npm fallback
    // route covers macOS/Linux via `@openai/codex` (bin = `codex`,
    // verified 2026-08). Winget is first-party on Windows.
    windows: { winget: "OpenAI.Codex" },
    fallback: { npm: "@openai/codex" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_registration_shape() {
        assert_eq!(CODEX.command, "codex");
        assert!(
            CODEX.macos.is_none(),
            "no brew formula; npm fallback covers macOS"
        );
        assert!(
            CODEX.linux.is_none(),
            "no distro package; npm fallback covers Linux"
        );
        let win = CODEX.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("OpenAI.Codex"));
        assert_eq!(CODEX.fallback.len(), 1);
        assert_eq!(CODEX.fallback[0].eco, crate::tools::spec::FallbackEco::Npm);
        assert_eq!(CODEX.fallback[0].package, "@openai/codex");
    }
}
