//! AWS SAM CLI - build, test, and deploy serverless applications
//!
//! The AWS Serverless Application Model (SAM) CLI (`sam`) builds,
//! locally tests, debugs, and deploys Lambda-based applications
//! defined with SAM or CloudFormation templates.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(AWS_SAM_CLI, {
    command: "sam",
    macos: { brew: "aws-sam-cli" },
    // Linux: no distro package; homebrew-core ships arm64/x86_64
    // Linux bottles for `aws-sam-cli`.
    linux: { brew: "aws-sam-cli" },
    windows: { winget: "Amazon.SAM-CLI" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aws_sam_cli_registration_shape() {
        assert_eq!(AWS_SAM_CLI.command, "sam");
        let mac = AWS_SAM_CLI.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("aws-sam-cli"));
        let linux = AWS_SAM_CLI.linux.expect("must support Linux");
        assert_eq!(linux.brew, Some("aws-sam-cli"));
        let win = AWS_SAM_CLI.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Amazon.SAM-CLI"));
    }
}
