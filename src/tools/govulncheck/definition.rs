//! govulncheck - Go vulnerability checker
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(GOVULNCHECK, {
    command: "govulncheck",
    repo: "golang/vuln",
    macos: { brew: "govulncheck" },
    linux: { uniform: "govulncheck" },
    bsd: { pkg: "govulncheck" },
    depends_on: &["go"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn govulncheck_registration_shape() {
        assert_eq!(GOVULNCHECK.command, "govulncheck");
        let mac = GOVULNCHECK.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("govulncheck"));
    }
}
