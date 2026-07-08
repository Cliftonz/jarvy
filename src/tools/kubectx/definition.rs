//! kubectx - fast Kubernetes context and namespace switching
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(KUBECTX, {
    command: "kubectx",
    macos: { brew: "kubectx" },
    linux: { brew: "kubectx" },
    windows: { winget: "ahmetb.kubectx" },
    bsd: { pkg: "kubectx" },
    // Shell completions ship with the package manager install (brew/pkg
    // drop them into the completions dir); the hook adds the upstream-
    // documented kctx/kns aliases, mirroring kubectl's `k` alias hook.
    default_hook: {
        description: "Add kctx/kns aliases for kubectx and kubens",
        script: r#"
for rc in "$HOME/.bashrc" "$HOME/.zshrc"; do
    if [ -f "$rc" ] && ! grep -q 'alias kctx=' "$rc"; then
        echo 'alias kctx="kubectx"' >> "$rc"
        echo 'alias kns="kubens"' >> "$rc"
        echo "Added kctx/kns aliases to $rc"
    fi
done
"#
    },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kubectx_registration_shape() {
        assert_eq!(KUBECTX.command, "kubectx");
        let mac = KUBECTX.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("kubectx"));
        let win = KUBECTX.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("ahmetb.kubectx"));
    }
}
