//! gitversion - automatic SemVer derivation from git history
//!
//! GitVersion is the de-facto SemVer-from-tags tool in the .NET CI/CD
//! ecosystem. The standalone binary is the same engine as the
//! `GitVersion.Tool` dotnet global tool; jarvy ships the standalone
//! form here for shells that don't have the .NET SDK available (CI
//! runners, container images), with a recommendation to use the
//! [nuget] global tool in .NET projects that already pin the SDK.

use crate::define_tool;

define_tool!(GITVERSION, {
    command: "gitversion",
    repo: "GitTools/GitVersion",
    macos: { brew: "gitversion" },
    linux: { uniform: "gitversion" },
    windows: { winget: "GitTools.GitVersion" },
    depends_on: &["git"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gitversion_registration_shape() {
        assert_eq!(GITVERSION.command, "gitversion");
        let mac = GITVERSION.macos.expect("gitversion must support macOS");
        assert_eq!(mac.brew, Some("gitversion"));
        let win = GITVERSION.windows.expect("gitversion must support Windows");
        assert_eq!(win.winget, Some("GitTools.GitVersion"));
    }
}
