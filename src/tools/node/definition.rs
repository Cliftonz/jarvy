//! node - Node.js JavaScript runtime
//!
//! Declarative platform slots (brew/apt/winget/pkg) plus an nvm route:
//! when nvm is installed and the config pins a concrete version
//! (`node = "24"`), install through `nvm install` so the pin is honored
//! and no second node lands on the system (#61 — brew's `node` formula
//! is node-current regardless of the pin, and a brew node next to an
//! nvm node shadow each other in PATH). "latest" / range hints fall
//! back to the platform slots unchanged.

use crate::define_tool;
use crate::tools::common::{InstallError, run};

fn install_node(min_hint: &str) -> Result<(), InstallError> {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        if crate::tools::nvm::is_installed() {
            if let Some(ver) = nvm_installable_pin(min_hint) {
                return install_via_nvm(ver);
            }
            println!(
                "  nvm detected but node pin '{}' isn't a concrete version — using the platform installer",
                min_hint
            );
        }
    }
    NODE.install_platform()
}

/// A hint nvm can install directly: `24`, `24.1`, `24.1.0` (optional
/// leading `v`). Ranges (`>=`, `~`), `latest`, and empty hints return
/// `None` — those keep the declarative platform path. The charset
/// restriction (digits + dots) doubles as the injection guard: the
/// value is interpolated into a `bash -c` string below.
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn nvm_installable_pin(hint: &str) -> Option<&str> {
    let v = hint.trim().trim_start_matches('v');
    if v.is_empty() || !v.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    let parts: Vec<&str> = v.split('.').collect();
    if parts.len() > 3 || parts.iter().any(|p| p.is_empty()) {
        return None;
    }
    Some(v)
}

/// nvm is a shell function — source its init script explicitly (the
/// same `nvm.sh` marker `tools::nvm::is_installed` verified exists)
/// rather than relying on any rc file. `nvm alias default` makes new
/// shells resolve the pinned version too.
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn install_via_nvm(ver: &str) -> Result<(), InstallError> {
    println!("  Installing node {} via nvm", ver);
    run(
        "bash",
        &[
            "-c",
            &format!(
                r#". "${{NVM_DIR:-$HOME/.nvm}}/nvm.sh" && nvm install {ver} && nvm alias default {ver}"#
            ),
        ],
    )?;
    Ok(())
}

define_tool!(NODE, {
    command: "node",
    macos: { brew: "node" },
    linux: { uniform: "nodejs" },
    windows: { winget: "OpenJS.NodeJS.LTS" },
    bsd: { pkg: "node" },
    custom_install: install_node,
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
    // Install nvm before node when both are in the config — a real
    // provisioning edge now that install_node routes pinned versions
    // through nvm (previously it only ordered two unrelated installers).
    depends_on: &["nvm"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_registration_shape() {
        assert_eq!(NODE.command, "node");
        let mac = NODE.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("node"));
        let win = NODE.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("OpenJS.NodeJS.LTS"));
        assert!(NODE.custom_install.is_some(), "nvm routing installer");
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn concrete_pins_route_to_nvm() {
        assert_eq!(nvm_installable_pin("24"), Some("24"));
        assert_eq!(nvm_installable_pin("24.1"), Some("24.1"));
        assert_eq!(nvm_installable_pin("v24.1.0"), Some("24.1.0"));
        assert_eq!(nvm_installable_pin(" 24 "), Some("24"));
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn non_concrete_hints_fall_back_to_platform() {
        for hint in [
            "latest", "", ">=24", "~24.1", "24.x", "lts/*", "24..1", "24.1.0.0", ".24",
        ] {
            assert_eq!(nvm_installable_pin(hint), None, "hint {hint:?}");
        }
    }
}
