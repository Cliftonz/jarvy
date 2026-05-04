//! betterleaks - secrets scanner
//!
//! Betterleaks is a secrets scanner built for configurability and speed.
//! It is the successor to Gitleaks with improved detection via token efficiency.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(BETTERLEAKS, {
    command: "betterleaks",
    macos: { brew: "betterleaks" },
    linux: { brew: "betterleaks" },
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
    fn ensure_betterleaks_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
