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
    fn golangci_lint_registration_shape() {
        assert_eq!(GOLANGCI_LINT.command, "golangci-lint");
        let mac = GOLANGCI_LINT.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("golangci-lint"));
        let win = GOLANGCI_LINT.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("GolangCI.golangci-lint"));
    }
}
