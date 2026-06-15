//! opentofu - open-source Terraform alternative
//!
//! This tool uses the ToolSpec pattern for declarative installation.
//! Note: The command is `tofu`, not `opentofu`.

use crate::define_tool;

define_tool!(OPENTOFU, {
    command: "tofu",
    macos: { brew: "opentofu" },
    linux: { uniform: "opentofu" },
    windows: { winget: "OpenTofu.OpenTofu" },
    bsd: { pkg: "opentofu" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opentofu_registration_shape() {
        assert_eq!(OPENTOFU.command, "tofu");
        let mac = OPENTOFU.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("opentofu"));
        let win = OPENTOFU.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("OpenTofu.OpenTofu"));
    }
}
