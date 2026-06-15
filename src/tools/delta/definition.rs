//! delta - syntax-highlighting pager for git diff output
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(DELTA, {
    command: "delta",
    macos: { brew: "git-delta" },
    linux: { apt: "git-delta", dnf: "git-delta", pacman: "git-delta", apk: "git-delta" },
    windows: { winget: "dandavison.delta" },
    bsd: { pkg: "git-delta" },
    default_hook: {
        description: "Configure delta as git pager for beautiful diffs",
        script: r#"
# Configure git to use delta as the pager
if command -v git >/dev/null 2>&1; then
    current_pager=$(git config --global core.pager 2>/dev/null || echo "")
    if [ "$current_pager" != "delta" ]; then
        git config --global core.pager delta
        git config --global interactive.diffFilter 'delta --color-only'
        git config --global delta.navigate true
        git config --global delta.light false
        git config --global merge.conflictstyle diff3
        git config --global diff.colorMoved default
        echo "Configured git to use delta as pager"
    fi
fi
"#
    },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delta_registration_shape() {
        assert_eq!(DELTA.command, "delta");
        let mac = DELTA.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("git-delta"));
        let win = DELTA.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("dandavison.delta"));
    }
}
