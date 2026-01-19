//! bat - cat clone with syntax highlighting
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(BAT, {
    command: "bat",
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
    fn ensure_bat_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
