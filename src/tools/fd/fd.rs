//! fd - fast and user-friendly alternative to find
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(FD, {
    command: "fd",
    macos: { brew: "fd" },
    linux: { apt: "fd-find", dnf: "fd-find", pacman: "fd", apk: "fd" },
    windows: { winget: "sharkdp.fd" },
    bsd: { pkg: "fd-find" },
    default_hook: {
        description: "Add fd alias for Debian/Ubuntu (fd-find package)",
        script: r#"
# On Debian/Ubuntu, fd is installed as fdfind due to naming conflict
# Add alias if fdfind exists but fd doesn't
if command -v fdfind >/dev/null 2>&1 && ! command -v fd >/dev/null 2>&1; then
    if [ -f "$HOME/.bashrc" ] && ! grep -q "alias fd='fdfind'" "$HOME/.bashrc"; then
        echo "alias fd='fdfind'" >> "$HOME/.bashrc"
        echo "Added fd alias for fdfind in .bashrc"
    fi
    if [ -f "$HOME/.zshrc" ] && ! grep -q "alias fd='fdfind'" "$HOME/.zshrc"; then
        echo "alias fd='fdfind'" >> "$HOME/.zshrc"
        echo "Added fd alias for fdfind in .zshrc"
    fi
fi
"#
    },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_fd_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
