//! grype - vulnerability scanner for containers
//!
//! Grype is a vulnerability scanner for container images and filesystems.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(GRYPE, {
    command: "grype",
    macos: { brew: "grype" },
    linux: { brew: "grype", apk: "grype" },
    windows: { choco: "grype" },
    bsd: { pkg: "grype" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_grype_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
