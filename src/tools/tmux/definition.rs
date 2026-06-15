//! tmux - terminal multiplexer
//!
//! This tool uses the ToolSpec pattern for declarative installation.
//! Note: Not supported on Windows natively - use WSL.

use crate::define_tool;

define_tool!(TMUX, {
    command: "tmux",
    macos: { brew: "tmux" },
    linux: { uniform: "tmux" },
    // No Windows support - tmux is Unix-only
    bsd: { pkg: "tmux" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tmux_registration_shape() {
        assert_eq!(TMUX.command, "tmux");
        let mac = TMUX.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("tmux"));
    }
}
