//! talisman - git hook secret scanner
//!
//! Talisman by ThoughtWorks validates outgoing changesets for secrets,
//! tokens, passwords, and private keys via pre-commit or pre-push hooks.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(TALISMAN, {
    command: "talisman",
    macos: { brew: "talisman" },
    linux: { brew: "talisman" },
    depends_on: &["git"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_talisman_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
