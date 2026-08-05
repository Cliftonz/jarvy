//! Claude Code - Anthropic's agentic coding CLI
//!
//! `claude` is Anthropic's terminal-based coding agent. Jarvy already
//! distributes ai_hooks, skills, and MCP server config to Claude Code;
//! this tool definition closes the loop by installing the CLI itself.

use crate::define_tool;

define_tool!(CLAUDE_CODE, {
    command: "claude",
    // No brew formula exists for Claude Code as of 2026-08; the npm
    // fallback route covers macOS/Linux via `@anthropic-ai/claude-code`
    // (bin = `claude`, verified 2026-08). Winget is first-party on
    // Windows.
    windows: { winget: "Anthropic.ClaudeCode" },
    fallback: { npm: "@anthropic-ai/claude-code" },
    category: "ai-agent",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_code_registration_shape() {
        assert_eq!(CLAUDE_CODE.command, "claude");
        assert!(
            CLAUDE_CODE.macos.is_none(),
            "no brew formula; npm fallback covers macOS"
        );
        assert!(
            CLAUDE_CODE.linux.is_none(),
            "no distro package; npm fallback covers Linux"
        );
        let win = CLAUDE_CODE.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Anthropic.ClaudeCode"));
        assert_eq!(CLAUDE_CODE.fallback.len(), 1);
        assert_eq!(
            CLAUDE_CODE.fallback[0].eco,
            crate::tools::spec::FallbackEco::Npm
        );
        assert_eq!(CLAUDE_CODE.fallback[0].package, "@anthropic-ai/claude-code");
    }
}
