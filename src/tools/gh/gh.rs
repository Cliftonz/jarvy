//! gh - GitHub's official CLI for PRs, issues, and repos
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(GH, {
    command: "gh",
    macos: { brew: "gh" },
    linux: { apt: "gh", dnf: "gh", pacman: "github-cli", apk: "github-cli" },
    windows: { winget: "GitHub.cli" },
    bsd: { pkg: "gh" },
    default_hook: {
        description: "Configure GitHub CLI shell completion",
        script: r#"
# Add gh CLI completion to .bashrc
if [ -f "$HOME/.bashrc" ] && ! grep -q 'gh completion' "$HOME/.bashrc"; then
    echo 'eval "$(gh completion -s bash)"' >> "$HOME/.bashrc"
    echo "Added gh CLI completion to .bashrc"
fi

# Add gh CLI completion to .zshrc
if [ -f "$HOME/.zshrc" ] && ! grep -q 'gh completion' "$HOME/.zshrc"; then
    echo 'eval "$(gh completion -s zsh)"' >> "$HOME/.zshrc"
    echo "Added gh CLI completion to .zshrc"
fi
"#
    },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_gh_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
