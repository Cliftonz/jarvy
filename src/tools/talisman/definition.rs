//! talisman - git hook secret scanner
//!
//! Talisman by ThoughtWorks validates outgoing changesets for secrets,
//! tokens, passwords, and private keys via pre-commit or pre-push hooks.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(TALISMAN, {
    command: "talisman",
    repo: "thoughtworks/talisman",
    macos: { brew: "talisman" },
    linux: { brew: "talisman" },
    depends_on: &["git"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn talisman_registration_shape() {
        assert_eq!(TALISMAN.command, "talisman");
        let mac = TALISMAN.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("talisman"));
    }
}
