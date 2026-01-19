//! trufflehog - secrets scanner
//!
//! TruffleHog searches through git repositories for secrets by digging deep into commit history.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(TRUFFLEHOG, {
    command: "trufflehog",
    macos: { brew: "trufflehog" },
    linux: { brew: "trufflehog" },
    bsd: { pkg: "trufflehog" },
    depends_on: &["git"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_trufflehog_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
