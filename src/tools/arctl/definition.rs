//! arctl - Agent Registry CLI
//!
//! arctl is the CLI for agentregistry, a centralized hub for managing LLMs,
//! Agents, Skills, and MCP Servers. Discover, deploy, run, and manage AI
//! artifacts from connected registries.
//!
//! This tool uses the ToolSpec pattern with a custom installer (no Homebrew formula).

use crate::define_tool;
use crate::tools::common::{InstallError, has, run};
use crate::tools::pinned_installer::PinnedInstaller;

/// Pinned commit of `agentregistry-dev/agentregistry`. Updating this constant
/// is the only way Jarvy will pull a newer arctl installer — no `main`/`HEAD`
/// fetches at runtime, so a compromise of the upstream branch tip cannot
/// silently land arbitrary code on the next `jarvy setup`.
///
/// To refresh: pick a commit, download
/// `https://raw.githubusercontent.com/agentregistry-dev/agentregistry/<sha>/scripts/get-arctl`,
/// compute its sha256, update both constants together.
const ARCTL_INSTALLER_COMMIT: &str = "2df820132f555380257510290cec498ab67db6bf";
const ARCTL_INSTALLER_SHA256: &str =
    "e90bfaf0e6e71000155f8aade195e51aea52624de3272813f7441d1712bcd377";

define_tool!(ARCTL, {
    command: "arctl",
    custom_install: install_arctl,
});

fn install_arctl(_min_hint: &str) -> Result<(), InstallError> {
    if has("arctl") {
        return Ok(());
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        let url = format!(
            "https://raw.githubusercontent.com/agentregistry-dev/agentregistry/{}/scripts/get-arctl",
            ARCTL_INSTALLER_COMMIT
        );
        let installer = PinnedInstaller {
            name: "arctl",
            url: &url,
            sha256: ARCTL_INSTALLER_SHA256,
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
    fn arctl_registration_shape() {
        assert_eq!(ARCTL.command, "arctl");
    }
}
