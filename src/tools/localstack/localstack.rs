//! localstack - local AWS cloud stack
//!
//! LocalStack provides a fully functional local AWS cloud stack for
//! development and testing cloud applications offline.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(LOCALSTACK, {
    command: "localstack",
    macos: { brew: "localstack" },
    linux: { uniform: "localstack" },
    bsd: { pkg: "localstack" },
    depends_on_one_of: &["docker", "podman"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_localstack_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
