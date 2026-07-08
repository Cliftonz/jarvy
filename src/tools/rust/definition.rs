//! rust - Rust toolchain via rustup
//!
//! Migrated from a legacy manual `ensure()/install()` impl to the
//! ToolSpec pattern so the tool can carry a `default_hook` (H3 in
//! tasks/additional-post-install-hooks.json — previously skipped
//! because hooks are a `define_tool!` slot). Install logic is
//! unchanged: rustup script on Unix, `Rustlang.Rustup` winget on
//! Windows, routed through `custom_install`.

use crate::define_tool;
#[cfg(target_os = "windows")]
use crate::tools::common::has;
use crate::tools::common::{InstallError, run};

fn install_rust(_min_hint: &str) -> Result<(), InstallError> {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        // Use bash -lc to ensure shell expands the pipe correctly
        return run(
            "bash",
            &[
                "-lc",
                "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y",
            ],
        )
        .map(|_| ());
    }
    #[cfg(target_os = "windows")]
    {
        if !has("winget") {
            return Err(InstallError::Prereq(
                "winget not found. Install Windows Package Manager, then re-run.",
            ));
        }
        // Official rustup package ID
        return run("winget", &["install", "-e", "--id", "Rustlang.Rustup"]).map(|_| ());
    }
    #[allow(unreachable_code)]
    Err(InstallError::Unsupported)
}

define_tool!(RUST, {
    command: "rustc",
    custom_install: install_rust,
    default_hook: {
        description: "Install clippy + rustfmt components and source cargo env in shell rc",
        script: r#"
# Ensure future shells pick up ~/.cargo/bin without a re-login
if [ -f "$HOME/.cargo/env" ]; then
    for rc in "$HOME/.bashrc" "$HOME/.zshrc"; do
        if [ -f "$rc" ] && ! grep -q '.cargo/env' "$rc"; then
            echo '. "$HOME/.cargo/env"' >> "$rc"
            echo "Added cargo env sourcing to $rc"
        fi
    done
fi

# Common components — rustup skips anything already installed
if command -v rustup >/dev/null 2>&1; then
    rustup component add clippy rustfmt 2>/dev/null || true
fi
"#
    },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_registration_shape() {
        assert_eq!(RUST.command, "rustc");
        assert!(RUST.custom_install.is_some());
        assert!(RUST.default_hook.is_some());
    }
}
