//! openclaw - open source AI coding assistant
//!
//! OpenClaw is a personal AI assistant that runs on your own devices.
//! It connects to WhatsApp, Telegram, Slack, Discord, and other messaging
//! platforms, with support for Anthropic, OpenAI, or local models.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(OPENCLAW, {
    command: "openclaw",
    macos: { brew: "openclaw-cli" },
    linux: { uniform: "openclaw-cli" },
    // No first-party winget manifest; the npm fallback route covers
    // Windows (verified 2026-08). Caveat: upstream docs recommend WSL2
    // on Windows; native npm install works but is second-choice.
    fallback: { npm: "openclaw" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openclaw_registration_shape() {
        assert_eq!(OPENCLAW.command, "openclaw");
        let mac = OPENCLAW.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("openclaw-cli"));
        assert!(OPENCLAW.windows.is_none(), "no first-party winget manifest");
        assert_eq!(OPENCLAW.fallback.len(), 1);
        assert_eq!(
            OPENCLAW.fallback[0].eco,
            crate::tools::spec::FallbackEco::Npm
        );
        assert_eq!(OPENCLAW.fallback[0].package, "openclaw");
    }
}
