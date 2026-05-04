//! noseyparker - secrets and sensitive data finder
//!
//! Nosey Parker is a high-performance tool for detecting secrets, passwords,
//! and sensitive data in text and git repositories.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(NOSEYPARKER, {
    command: "noseyparker",
    macos: { brew: "noseyparker" },
    linux: { brew: "noseyparker" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_noseyparker_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
