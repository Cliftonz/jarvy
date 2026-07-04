//! lynis - Security auditing tool for Unix systems
//!
//! This tool uses the ToolSpec pattern for declarative installation.
//! Note: No Windows support available.

use crate::define_tool;

define_tool!(LYNIS, {
    command: "lynis",
    repo: "CISOfy/lynis",
    macos: { brew: "lynis" },
    linux: { uniform: "lynis" },
    bsd: { pkg: "lynis" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lynis_registration_shape() {
        assert_eq!(LYNIS.command, "lynis");
        let mac = LYNIS.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("lynis"));
    }
}
