//! bat - cat clone with syntax highlighting
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(BAT, {
    command: "bat",
    repo: "sharkdp/bat",
    macos: { brew: "bat" },
    linux: { uniform: "bat" },
    windows: { winget: "sharkdp.bat" },
    bsd: { pkg: "bat" },
    default_hook: {
        description: "Configure bat as MANPAGER for colored man pages",
        script: r#"
# Add bat as MANPAGER to .bashrc
if [ -f "$HOME/.bashrc" ] && ! grep -q 'MANPAGER.*bat' "$HOME/.bashrc"; then
    echo 'export MANPAGER="sh -c '\''col -bx | bat -l man -p'\''"' >> "$HOME/.bashrc"
    echo "Configured bat as MANPAGER in .bashrc"
fi

# Add bat as MANPAGER to .zshrc
if [ -f "$HOME/.zshrc" ] && ! grep -q 'MANPAGER.*bat' "$HOME/.zshrc"; then
    echo 'export MANPAGER="sh -c '\''col -bx | bat -l man -p'\''"' >> "$HOME/.zshrc"
    echo "Configured bat as MANPAGER in .zshrc"
fi
"#
    },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bat_registration_shape() {
        assert_eq!(BAT.command, "bat");
        let mac = BAT.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("bat"));
        let win = BAT.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("sharkdp.bat"));
    }
}
