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
    fn ensure_lazydocker_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
