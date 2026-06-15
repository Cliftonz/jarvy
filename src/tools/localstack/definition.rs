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
    fn localstack_registration_shape() {
        assert_eq!(LOCALSTACK.command, "localstack");
        let mac = LOCALSTACK.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("localstack"));
    }
}
