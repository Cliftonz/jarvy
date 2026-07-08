//! cfn-lint - CloudFormation template linter
//!
//! `cfn-lint` validates AWS CloudFormation templates (JSON/YAML)
//! against the CloudFormation resource specification plus additional
//! best-practice checks. Python-based; homebrew-core packages it with
//! its own virtualenv on both macOS and Linux.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(CFN_LINT, {
    command: "cfn-lint",
    macos: { brew: "cfn-lint" },
    // Linux: no distro package; install via Linuxbrew (or `pip install cfn-lint`).
    linux: { brew: "cfn-lint" },
    // No first-party winget manifest as of 2026-07; install with
    // `pip install cfn-lint` per https://github.com/aws-cloudformation/cfn-lint.
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cfn_lint_registration_shape() {
        assert_eq!(CFN_LINT.command, "cfn-lint");
        let mac = CFN_LINT.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("cfn-lint"));
        let linux = CFN_LINT.linux.expect("must support Linux");
        assert_eq!(linux.brew, Some("cfn-lint"));
        assert!(CFN_LINT.windows.is_none(), "no first-party winget manifest");
    }
}
