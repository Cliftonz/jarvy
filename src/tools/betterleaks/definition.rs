//! betterleaks - secrets scanner
//!
//! Betterleaks is a secrets scanner built for configurability and speed.
//! It is the successor to Gitleaks with improved detection via token efficiency.
//!
//! Declarative brew slots on macOS/Linux plus a go-install route on
//! Windows: no first-party winget/choco/scoop manifest exists as of
//! 2026-08 (upstream ships release zips + WSL), but upstream documents
//! `go install github.com/betterleaks/betterleaks@latest` — when go is
//! on PATH we use it; otherwise the standard `tool.unsupported` UX.

use crate::define_tool;
use crate::tools::common::InstallError;
#[cfg(windows)]
use crate::tools::common::{has, run};

/// Windows: go-install when go is present; other platforms fall
/// through to the declarative brew slots.
///
/// REVISIT: no first-party winget/choco/scoop manifest as of 2026-08
/// (upstream ships release zips only — tracked in the jarvy repo issue
/// "betterleaks: switch Windows install to winget"). When a first-party
/// manifest lands, replace this route with a `windows: { winget: ... }`
/// block and delete the go path.
#[cfg(windows)]
fn install_betterleaks(version: &str) -> Result<(), InstallError> {
    if has("go") {
        let spec = go_module_spec(version);
        println!("  Installing betterleaks via `go install {spec}`");
        run("go", &["install", &spec])?;
        return Ok(());
    }
    println!(
        "  go not found — install go first, or download a Windows build from \
         https://github.com/betterleaks/betterleaks/releases"
    );
    BETTERLEAKS.install_platform()
}

#[cfg(not(windows))]
fn install_betterleaks(_version: &str) -> Result<(), InstallError> {
    BETTERLEAKS.install_platform()
}

/// Concrete pins (`1.7.3`, optional leading `v`) become `@v1.7.3`;
/// everything else (`latest`, ranges, empty) goes to `@latest`. The
/// digits+dots charset check doubles as the injection guard — the
/// value lands in the `go install` argv.
#[cfg_attr(not(windows), allow(dead_code))] // windows-only call site; tests are cross-platform
fn go_module_spec(version: &str) -> String {
    let v = version.trim().trim_start_matches('v');
    let concrete = !v.is_empty()
        && v.chars().all(|c| c.is_ascii_digit() || c == '.')
        && v.split('.').all(|p| !p.is_empty())
        && v.split('.').count() <= 3;
    if concrete {
        format!("github.com/betterleaks/betterleaks@v{v}")
    } else {
        "github.com/betterleaks/betterleaks@latest".to_string()
    }
}

define_tool!(BETTERLEAKS, {
    command: "betterleaks",
    macos: { brew: "betterleaks" },
    linux: { brew: "betterleaks" },
    custom_install: install_betterleaks,
    default_hook: {
        description: "Install git pre-push hook to scan for secrets before each push",
        script: r##"
HOOK_DIR="$(git rev-parse --show-toplevel 2>/dev/null)/.git/hooks"
if [ -d "$HOOK_DIR" ]; then
    HOOK_FILE="$HOOK_DIR/pre-push"
    MARKER="# jarvy:betterleaks-pre-push"
    if [ ! -f "$HOOK_FILE" ] || ! grep -q "$MARKER" "$HOOK_FILE"; then
        if [ ! -f "$HOOK_FILE" ]; then
            printf '#!/bin/sh\n' > "$HOOK_FILE"
            chmod +x "$HOOK_FILE"
        fi
        cat >> "$HOOK_FILE" <<'HOOK'

# jarvy:betterleaks-pre-push
echo "Running betterleaks secret scan..."
betterleaks git . --no-banner
if [ $? -ne 0 ]; then
    echo "betterleaks: secrets detected, push blocked"
    exit 1
fi
HOOK
        echo "betterleaks pre-push hook installed"
    fi
fi
"##
    },
    depends_on: &["git"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn betterleaks_registration_shape() {
        assert_eq!(BETTERLEAKS.command, "betterleaks");
        let mac = BETTERLEAKS.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("betterleaks"));
        // No winget block on purpose (no first-party manifest); the
        // Windows path is the go-install route in custom_install.
        assert!(BETTERLEAKS.windows.is_none());
        assert!(BETTERLEAKS.custom_install.is_some());
    }

    #[test]
    fn concrete_pins_become_versioned_go_specs() {
        assert_eq!(
            go_module_spec("1.7.3"),
            "github.com/betterleaks/betterleaks@v1.7.3"
        );
        assert_eq!(
            go_module_spec("v1.7"),
            "github.com/betterleaks/betterleaks@v1.7"
        );
        assert_eq!(
            go_module_spec(" 2 "),
            "github.com/betterleaks/betterleaks@v2"
        );
    }

    #[test]
    fn non_concrete_hints_fall_back_to_latest() {
        for hint in [
            "latest", "", "*", ">=1.7", "~1.7", "1.x", "1..3", ".1", "1.2.3.4",
        ] {
            assert_eq!(
                go_module_spec(hint),
                "github.com/betterleaks/betterleaks@latest",
                "hint {hint:?}"
            );
        }
    }
}
