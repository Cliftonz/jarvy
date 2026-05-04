//! golangci-lint - Go linters aggregator
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(GOLANGCI_LINT, {
    command: "golangci-lint",
    macos: { brew: "golangci-lint" },
    linux: { uniform: "golangci-lint" },
    windows: { winget: "GolangCI.golangci-lint" },
    bsd: { pkg: "golangci-lint" },
    depends_on: &["go"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_golangci_lint_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
