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
    default_hook: {
        description: "Install TPM (tmux plugin manager) and seed its run line in ~/.tmux.conf",
        script: r#"
# Clone TPM if git is available and it isn't already present
if command -v git >/dev/null 2>&1 && [ ! -d "$HOME/.tmux/plugins/tpm" ]; then
    git clone --depth 1 https://github.com/tmux-plugins/tpm "$HOME/.tmux/plugins/tpm" \
        && echo "Installed TPM to ~/.tmux/plugins/tpm"
fi

# Seed the TPM run line so `prefix + I` works out of the box
if [ -d "$HOME/.tmux/plugins/tpm" ]; then
    touch "$HOME/.tmux.conf"
    if ! grep -q "tpm/tpm" "$HOME/.tmux.conf"; then
        echo "run '~/.tmux/plugins/tpm/tpm'" >> "$HOME/.tmux.conf"
        echo "Added TPM run line to ~/.tmux.conf"
    fi
fi
"#
    },
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
