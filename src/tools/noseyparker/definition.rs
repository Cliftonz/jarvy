//! noseyparker - secrets and sensitive data finder
//!
//! Nosey Parker is a high-performance tool for detecting secrets, passwords,
//! and sensitive data in text and git repositories.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(NOSEYPARKER, {
    command: "noseyparker",
    repo: "praetorian-inc/noseyparker",
    macos: { brew: "noseyparker" },
    linux: { brew: "noseyparker" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noseyparker_registration_shape() {
        assert_eq!(NOSEYPARKER.command, "noseyparker");
        let mac = NOSEYPARKER.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("noseyparker"));
    }
}
