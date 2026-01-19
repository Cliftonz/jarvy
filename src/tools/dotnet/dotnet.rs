//! dotnet - .NET SDK and runtime
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(DOTNET, {
    command: "dotnet",
    macos: { cask: "dotnet-sdk" },
    linux: { apt: "dotnet-sdk-8.0", dnf: "dotnet-sdk", pacman: "dotnet-sdk", apk: "dotnet-sdk" },
    windows: { winget: "Microsoft.DotNet.SDK.8" },
    bsd: { pkg: "dotnet" },
    default_hook: {
        description: "Configure DOTNET_ROOT and add .NET tools to PATH",
        script: r#"
# .NET PATH configuration
# Set up DOTNET_ROOT and add tools directory to PATH

# Detect installation path
DOTNET_INSTALL_DIR=""
if [ -d "/usr/share/dotnet" ]; then
    DOTNET_INSTALL_DIR="/usr/share/dotnet"
elif [ -d "/usr/local/share/dotnet" ]; then
    DOTNET_INSTALL_DIR="/usr/local/share/dotnet"
elif [ -d "$HOME/.dotnet" ]; then
    DOTNET_INSTALL_DIR="$HOME/.dotnet"
fi

if [ -n "$DOTNET_INSTALL_DIR" ]; then
    DOTNET_PATH_EXPORTS="
# .NET environment
export DOTNET_ROOT=\"$DOTNET_INSTALL_DIR\"
export PATH=\"\$PATH:\$HOME/.dotnet/tools\"
"

    # Add to .bashrc if not present
    if [ -f "$HOME/.bashrc" ] && ! grep -q 'DOTNET_ROOT' "$HOME/.bashrc"; then
        echo "$DOTNET_PATH_EXPORTS" >> "$HOME/.bashrc"
        echo "Added .NET PATH configuration to ~/.bashrc"
    fi

    # Add to .zshrc if not present
    if [ -f "$HOME/.zshrc" ] && ! grep -q 'DOTNET_ROOT' "$HOME/.zshrc"; then
        echo "$DOTNET_PATH_EXPORTS" >> "$HOME/.zshrc"
        echo "Added .NET PATH configuration to ~/.zshrc"
    fi
fi

# Create tools directory if it doesn't exist
mkdir -p "$HOME/.dotnet/tools"
"#
    },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_dotnet_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
