//! crane - container registry tool
//!
//! Crane is a tool for interacting with remote container images and registries.
//! It can copy, list, mutate, and inspect container images without a Docker daemon.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(CRANE, {
    command: "crane",
    macos: { brew: "crane" },
    linux: { uniform: "crane" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_crane_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
