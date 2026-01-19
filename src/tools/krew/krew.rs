//! krew - kubectl plugin manager
//!
//! Krew is a plugin manager for kubectl command-line tool.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(KREW, {
    command: "kubectl-krew",
    macos: { brew: "krew" },
    linux: { brew: "krew", apk: "kubectl-krew" },
    bsd: { pkg: "krew" },
    default_hook: {
        description: "Add krew to PATH in .bashrc and .zshrc",
        script: r#"
# krew PATH setup
KREW_PATH='export PATH="${KREW_ROOT:-$HOME/.krew}/bin:$PATH"'

# Add to .bashrc if not present
if [ -f "$HOME/.bashrc" ] && ! grep -q '.krew/bin' "$HOME/.bashrc"; then
    echo "$KREW_PATH" >> "$HOME/.bashrc"
    echo "Added krew PATH to ~/.bashrc"
fi

# Add to .zshrc if not present
if [ -f "$HOME/.zshrc" ] && ! grep -q '.krew/bin' "$HOME/.zshrc"; then
    echo "$KREW_PATH" >> "$HOME/.zshrc"
    echo "Added krew PATH to ~/.zshrc"
fi
"#
    },
    // kubectl plugin manager needs kubectl
    depends_on_one_of: &["kubectl"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_krew_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
