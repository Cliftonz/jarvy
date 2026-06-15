//! kmcp - Build, test, and deploy MCP servers on Kubernetes
//!
//! kmcp is a CLI tool and Kubernetes controller for scaffolding, building,
//! and deploying Model Context Protocol (MCP) servers. Companion tool to kagent.
//!
//! This tool uses the ToolSpec pattern with a custom installer (no Homebrew formula).

use crate::define_tool;
use crate::tools::common::{InstallError, has, run};
use crate::tools::pinned_installer::PinnedInstaller;

/// Pinned commit of `kagent-dev/kmcp`. Updating this constant is the only
/// way Jarvy will pull a newer kmcp installer — no `main`/`HEAD` fetches
/// at runtime, so a compromise of the upstream branch tip cannot silently
/// land arbitrary code on the next `jarvy setup`.
///
/// To refresh: pick a commit, download
/// `https://raw.githubusercontent.com/kagent-dev/kmcp/<sha>/scripts/get-kmcp.sh`,
/// compute its sha256, update both constants together.
const KMCP_INSTALLER_COMMIT: &str = "1cec6470560fa8ccc43de3d95c7567993ae13e95";
const KMCP_INSTALLER_SHA256: &str =
    "7336aa53391c0aa3e302d7cf914de5e412aae1c7f400f81fb5425be0c939e884";

define_tool!(KMCP, {
    command: "kmcp",
    custom_install: install_kmcp,
    depends_on: &["kubectl"],
});

fn install_kmcp(_min_hint: &str) -> Result<(), InstallError> {
    if has("kmcp") {
        return Ok(());
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        let url = format!(
            "https://raw.githubusercontent.com/kagent-dev/kmcp/{}/scripts/get-kmcp.sh",
            KMCP_INSTALLER_COMMIT
        );
        let installer = PinnedInstaller {
            name: "kmcp",
            url: &url,
            sha256: KMCP_INSTALLER_SHA256,
        };
        run("sh", &["-c", &installer.shell_command()])?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err(InstallError::Unsupported)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kmcp_registration_shape() {
        assert_eq!(KMCP.command, "kmcp");
    }
}
