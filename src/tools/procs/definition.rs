//! procs - Modern replacement for ps
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(PROCS, {
    command: "procs",
    repo: "dalance/procs",
    macos: { brew: "procs" },
    linux: { uniform: "procs" },
    windows: { winget: "dalance.procs" },
    bsd: { pkg: "procs" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn procs_registration_shape() {
        assert_eq!(PROCS.command, "procs");
        let mac = PROCS.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("procs"));
        let win = PROCS.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("dalance.procs"));
    }
}
