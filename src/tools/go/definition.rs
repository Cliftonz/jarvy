//! go - Go programming language
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(GO, {
    command: "go",
    macos: { brew: "go" },
    linux: { apt: "golang", dnf: "golang", pacman: "go", apk: "go" },
    windows: { winget: "GoLang.Go" },
    bsd: { pkg: "go" },
    default_hook: {
        description: "Configure GOPATH and add Go binaries to PATH",
        script: r#"
# Go PATH configuration
# Set up GOPATH and add $GOPATH/bin to PATH

GO_PATH_EXPORTS='
# Go environment
export GOPATH="$HOME/go"
export PATH="$PATH:$GOPATH/bin"
'

# Add to .bashrc if not present
if [ -f "$HOME/.bashrc" ] && ! grep -q 'GOPATH' "$HOME/.bashrc"; then
    echo "$GO_PATH_EXPORTS" >> "$HOME/.bashrc"
    echo "Added Go PATH configuration to ~/.bashrc"
fi

# Add to .zshrc if not present
if [ -f "$HOME/.zshrc" ] && ! grep -q 'GOPATH' "$HOME/.zshrc"; then
    echo "$GO_PATH_EXPORTS" >> "$HOME/.zshrc"
    echo "Added Go PATH configuration to ~/.zshrc"
fi

# Create GOPATH directory if it doesn't exist
mkdir -p "$HOME/go/bin"
"#
    },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn go_registration_shape() {
        assert_eq!(GO.command, "go");
        let mac = GO.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("go"));
        let win = GO.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("GoLang.Go"));
    }
}
