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
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_openclaw_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
