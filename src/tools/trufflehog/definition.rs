//! trufflehog - secrets scanner
//!
//! TruffleHog searches through git repositories for secrets by digging deep into commit history.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(TRUFFLEHOG, {
    command: "trufflehog",
    repo: "trufflesecurity/trufflehog",
    macos: { brew: "trufflehog" },
    linux: { brew: "trufflehog" },
    bsd: { pkg: "trufflehog" },
    depends_on: &["git"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trufflehog_registration_shape() {
        assert_eq!(TRUFFLEHOG.command, "trufflehog");
        let mac = TRUFFLEHOG.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("trufflehog"));
    }
}
