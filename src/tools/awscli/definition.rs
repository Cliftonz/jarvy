//! awscli - AWS Command Line Interface
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(AWSCLI, {
    command: "aws",
    repo: "aws/aws-cli",
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
    fn awscli_registration_shape() {
        assert_eq!(AWSCLI.command, "aws");
        let mac = AWSCLI.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("awscli"));
        let win = AWSCLI.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Amazon.AWSCLI"));
    }
}
