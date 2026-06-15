//! dagger - portable devkit for CI/CD pipelines
//!
//! Dagger is a portable devkit for CI/CD pipelines. It allows you to
//! develop powerful CI/CD pipelines locally and run them anywhere.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(DAGGER, {
    command: "dagger",
    macos: { brew: "dagger/tap/dagger" },
    linux: { uniform: "dagger" },
    windows: { winget: "Dagger.Dagger" },
    bsd: { pkg: "dagger" },
    depends_on_one_of: &["docker", "podman"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dagger_registration_shape() {
        assert_eq!(DAGGER.command, "dagger");
        let mac = DAGGER.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("dagger/tap/dagger"));
        let win = DAGGER.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Dagger.Dagger"));
    }
}
