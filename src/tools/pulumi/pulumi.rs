//! pulumi - Infrastructure as code in any programming language
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(PULUMI, {
    command: "pulumi",
    macos: { brew: "pulumi" },
    linux: { uniform: "pulumi" },
    windows: { winget: "Pulumi.Pulumi" },
    bsd: { pkg: "pulumi" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_pulumi_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
