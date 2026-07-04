//! grype - vulnerability scanner for containers
//!
//! Grype is a vulnerability scanner for container images and filesystems.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(GRYPE, {
    command: "grype",
    repo: "anchore/grype",
    macos: { brew: "grype" },
    linux: { brew: "grype", apk: "grype" },
    windows: { choco: "grype" },
    bsd: { pkg: "grype" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grype_registration_shape() {
        assert_eq!(GRYPE.command, "grype");
        let mac = GRYPE.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("grype"));
    }
}
