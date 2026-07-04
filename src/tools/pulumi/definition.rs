//! pulumi - Infrastructure as code in any programming language
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(PULUMI, {
    command: "pulumi",
    repo: "pulumi/pulumi",
    macos: { brew: "pulumi" },
    linux: { uniform: "pulumi" },
    windows: { winget: "Pulumi.Pulumi" },
    bsd: { pkg: "pulumi" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pulumi_registration_shape() {
        assert_eq!(PULUMI.command, "pulumi");
        let mac = PULUMI.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("pulumi"));
        let win = PULUMI.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Pulumi.Pulumi"));
    }
}
