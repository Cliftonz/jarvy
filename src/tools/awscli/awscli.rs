//! awscli - AWS Command Line Interface
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(AWSCLI, {
    command: "aws",
    macos: { brew: "awscli" },
    linux: { apt: "awscli", dnf: "awscli", pacman: "aws-cli-v2", apk: "aws-cli" },
    windows: { winget: "Amazon.AWSCLI" },
    bsd: { pkg: "awscli" },
    default_hook: {
        description: "Configure AWS CLI shell completion",
        script: r#"
# Add AWS CLI completion to .bashrc
if [ -f "$HOME/.bashrc" ] && ! grep -q 'aws_completer' "$HOME/.bashrc"; then
    echo 'complete -C aws_completer aws' >> "$HOME/.bashrc"
    echo "Added AWS CLI completion to .bashrc"
fi

# Add AWS CLI completion to .zshrc
if [ -f "$HOME/.zshrc" ] && ! grep -q 'aws_completer' "$HOME/.zshrc"; then
    echo 'autoload bashcompinit && bashcompinit && complete -C aws_completer aws' >> "$HOME/.zshrc"
    echo "Added AWS CLI completion to .zshrc"
fi
"#
    },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_awscli_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
