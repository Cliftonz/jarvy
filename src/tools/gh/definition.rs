//! gh - GitHub's official CLI for PRs, issues, and repos
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(GH, {
    command: "gh",
    repo: "cli/cli",
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
    fn gh_registration_shape() {
        assert_eq!(GH.command, "gh");
        let mac = GH.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("gh"));
        let win = GH.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("GitHub.cli"));
    }
}
