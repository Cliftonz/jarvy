//! betterleaks - secrets scanner
//!
//! Betterleaks is a secrets scanner built for configurability and speed.
//! It is the successor to Gitleaks with improved detection via token efficiency.
//!
//! Declarative brew slots on macOS/Linux plus a PRD-060 go fallback
//! route everywhere else: no first-party winget/choco/scoop manifest
//! exists as of 2026-08 (upstream ships release zips + WSL), but
//! upstream documents `go install github.com/betterleaks/betterleaks@latest`.
//! The fallback runtime bootstraps go through jarvy's own registry when
//! it's missing, so this file no longer carries a bespoke installer.
//!
//! REVISIT: when a first-party winget manifest lands (tracked in the
//! jarvy repo issue "betterleaks: switch Windows install to winget"),
//! add a `windows: { winget: ... }` block — the platform slot then wins
//! over the fallback route automatically.

use crate::define_tool;

define_tool!(BETTERLEAKS, {
    command: "betterleaks",
    macos: { brew: "betterleaks" },
    linux: { brew: "betterleaks" },
    fallback: { go: "github.com/betterleaks/betterleaks" },
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
    use crate::tools::spec::FallbackEco;

    #[test]
    fn betterleaks_registration_shape() {
        assert_eq!(BETTERLEAKS.command, "betterleaks");
        let mac = BETTERLEAKS.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("betterleaks"));
        // No winget block on purpose (no first-party manifest); Windows
        // is covered by the go fallback route instead of a bespoke
        // custom_install (PRD-060 first consumer).
        assert!(BETTERLEAKS.windows.is_none());
        assert!(BETTERLEAKS.custom_install.is_none());
    }

    #[test]
    fn betterleaks_declares_go_fallback_route() {
        assert_eq!(BETTERLEAKS.fallback.len(), 1);
        let route = &BETTERLEAKS.fallback[0];
        assert_eq!(route.eco, FallbackEco::Go);
        assert_eq!(route.package, "github.com/betterleaks/betterleaks");
    }
}
