//! lazydocker - simple terminal UI for docker
//!
//! Lazydocker is a terminal UI for docker and docker-compose.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(LAZYDOCKER, {
    command: "lazydocker",
    macos: { brew: "lazydocker" },
    linux: { brew: "lazydocker", apk: "lazydocker" },
    windows: { choco: "lazydocker" },
    bsd: { pkg: "lazydocker" },
    // Docker TUI requires Docker daemon to be installed
    depends_on: &["docker"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lazydocker_registration_shape() {
        assert_eq!(LAZYDOCKER.command, "lazydocker");
        let mac = LAZYDOCKER.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("lazydocker"));
    }
}
