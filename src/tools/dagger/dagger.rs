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
    fn ensure_dagger_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
