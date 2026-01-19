//! python - Python programming language
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(PYTHON, {
    command: "python3",
    macos: { brew: "python" },
    linux: { apt: "python3", dnf: "python3", pacman: "python", apk: "python3" },
    windows: { winget: "Python.Python.3" },
    bsd: { pkg: "python3" },
    default_hook: {
        description: "Upgrade pip and configure user site-packages PATH",
        script: r#"
# Upgrade pip to latest version
python3 -m pip install --upgrade pip --quiet 2>/dev/null || true

# Get user site-packages bin directory
USER_SITE=$(python3 -m site --user-base 2>/dev/null)/bin

# Add user site-packages bin to PATH in .bashrc
if [ -n "$USER_SITE" ] && [ -f "$HOME/.bashrc" ] && ! grep -q 'python.*user-base' "$HOME/.bashrc"; then
    echo "export PATH=\"$USER_SITE:\$PATH\"  # python user-base" >> "$HOME/.bashrc"
    echo "Added Python user bin to .bashrc"
fi

# Add user site-packages bin to PATH in .zshrc
if [ -n "$USER_SITE" ] && [ -f "$HOME/.zshrc" ] && ! grep -q 'python.*user-base' "$HOME/.zshrc"; then
    echo "export PATH=\"$USER_SITE:\$PATH\"  # python user-base" >> "$HOME/.zshrc"
    echo "Added Python user bin to .zshrc"
fi
"#
    },
    // Install pyenv before python if both are in the config
    depends_on: &["pyenv"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_python_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
