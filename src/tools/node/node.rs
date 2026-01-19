//! node - Node.js JavaScript runtime
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(NODE, {
    command: "node",
    macos: { brew: "node" },
    linux: { uniform: "nodejs" },
    windows: { winget: "OpenJS.NodeJS.LTS" },
    bsd: { pkg: "node" },
    default_hook: {
        description: "Configure npm global prefix and add to PATH",
        script: r#"
# Configure npm prefix for global installs without sudo
mkdir -p ~/.npm-global
npm config set prefix '~/.npm-global' 2>/dev/null || true

# Add npm global bin to PATH in .bashrc
if [ -f "$HOME/.bashrc" ] && ! grep -q '.npm-global/bin' "$HOME/.bashrc"; then
    echo 'export PATH="$HOME/.npm-global/bin:$PATH"' >> "$HOME/.bashrc"
    echo "Added npm global bin to .bashrc"
fi

# Add npm global bin to PATH in .zshrc
if [ -f "$HOME/.zshrc" ] && ! grep -q '.npm-global/bin' "$HOME/.zshrc"; then
    echo 'export PATH="$HOME/.npm-global/bin:$PATH"' >> "$HOME/.zshrc"
    echo "Added npm global bin to .zshrc"
fi
"#
    },
    // Install nvm before node if both are in the config
    depends_on: &["nvm"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_node_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
