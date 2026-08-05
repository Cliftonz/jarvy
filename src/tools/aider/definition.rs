//! aider - AI pair-programming CLI in the terminal
//!
//! `aider` edits code in a local git repo through conversation with an
//! LLM (Claude, GPT, and others), auto-committing each change with a
//! sensible message. One of the earliest and most widely used
//! terminal coding agents.

use crate::define_tool;

define_tool!(AIDER, {
    command: "aider",
    macos: { brew: "aider" },
    // Linux: no distro package; Linuxbrew installs the same
    // homebrew-core formula.
    linux: { brew: "aider" },
    // No first-party winget manifest as of 2026-08; the uv fallback
    // route covers Windows. Note: the PyPI package is `aider-chat` but
    // the installed binary is `aider` (verified 2026-08).
    fallback: { uv: "aider-chat" },
    category: "ai-agent",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aider_registration_shape() {
        assert_eq!(AIDER.command, "aider");
        let mac = AIDER.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("aider"));
        let linux = AIDER.linux.expect("must support Linux");
        assert_eq!(linux.brew, Some("aider"));
        assert!(AIDER.windows.is_none(), "no first-party winget manifest");
        assert_eq!(AIDER.fallback.len(), 1);
        assert_eq!(AIDER.fallback[0].eco, crate::tools::spec::FallbackEco::Uv);
        assert_eq!(AIDER.fallback[0].package, "aider-chat");
    }
}
