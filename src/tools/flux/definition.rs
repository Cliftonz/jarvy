//! flux - GitOps toolkit for Kubernetes
//!
//! This tool uses the ToolSpec pattern for declarative installation.
//! Note: Linux/Windows may require custom installation.

use crate::define_tool;

define_tool!(FLUX, {
    command: "flux",
    repo: "fluxcd/flux2",
    macos: { brew: "fluxcd/tap/flux" },
    linux: { brew: "fluxcd/tap/flux" },
    windows: { winget: "Fluxcd.Flux" },
    bsd: { pkg: "flux" },
    default_hook: {
        description: "Install flux shell completions for bash and zsh",
        script: r#"
# Generate and install flux completions for bash
if [ -f "$HOME/.bashrc" ]; then
    mkdir -p "$HOME/.local/share/bash-completion/completions"
    flux completion bash > "$HOME/.local/share/bash-completion/completions/flux" 2>/dev/null || true
fi

# Generate and install flux completions for zsh
if [ -f "$HOME/.zshrc" ]; then
    mkdir -p "$HOME/.zsh/completions"
    flux completion zsh > "$HOME/.zsh/completions/_flux" 2>/dev/null || true
    if ! grep -q 'fpath.*\.zsh/completions' "$HOME/.zshrc"; then
        echo 'fpath=($HOME/.zsh/completions $fpath)' >> "$HOME/.zshrc"
    fi
fi
"#
    },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flux_registration_shape() {
        assert_eq!(FLUX.command, "flux");
        let mac = FLUX.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("fluxcd/tap/flux"));
        let win = FLUX.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Fluxcd.Flux"));
    }
}
