//! ollama - Run large language models locally
//!
//! Ollama is a tool for running open-source large language models locally.
//! It supports models like Llama, Mistral, Gemma, and more.
//!
//! This tool uses the ToolSpec pattern with a custom installer for Linux.

use crate::define_tool;
use crate::tools::common::{InstallError, has, run};
#[cfg(target_os = "linux")]
use crate::tools::pinned_installer::PinnedInstaller;

/// Pinned commit of `ollama/ollama`. Used only on Linux when Homebrew is not
/// available; the macOS/winget paths use signed first-party packages and
/// don't need this. Updating this constant is the only way Jarvy will pull a
/// newer install.sh.
///
/// To refresh: pick a commit, download
/// `https://raw.githubusercontent.com/ollama/ollama/<sha>/scripts/install.sh`,
/// compute its sha256, update both constants together.
#[cfg(target_os = "linux")]
const OLLAMA_INSTALLER_COMMIT: &str = "f866e7608f378dcfca6f8c717101df1945db3b97";
#[cfg(target_os = "linux")]
const OLLAMA_INSTALLER_SHA256: &str =
    "25f64b810b947145095956533e1bdf56eacea2673c55a7e586be4515fc882c9f";

define_tool!(OLLAMA, {
    command: "ollama",
    repo: "ollama/ollama",
    macos: { brew: "ollama" },
    linux: { brew: "ollama" },
    windows: { winget: "Ollama.Ollama" },
    custom_install: install_ollama,
});

fn install_ollama(_min_hint: &str) -> Result<(), InstallError> {
    // Check if already installed
    if has("ollama") {
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        if !has("brew") {
            return Err(InstallError::Prereq(
                "Homebrew not found. Install https://brew.sh and re-run.",
            ));
        }
        run("brew", &["install", "ollama"])?;
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        // Try Homebrew first if available
        if has("brew") {
            run("brew", &["install", "ollama"])?;
            return Ok(());
        }

        // Fall back to the official install script — pinned to a known
        // commit and sha256-verified before exec.
        let url = format!(
            "https://raw.githubusercontent.com/ollama/ollama/{}/scripts/install.sh",
            OLLAMA_INSTALLER_COMMIT
        );
        let installer = PinnedInstaller {
            name: "ollama",
            url: &url,
            sha256: OLLAMA_INSTALLER_SHA256,
        };
        run("sh", &["-c", &installer.shell_command()])?;
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        if !has("winget") {
            return Err(InstallError::Prereq(
                "winget not found. Install Windows Package Manager, then re-run.",
            ));
        }
        run("winget", &["install", "-e", "--id", "Ollama.Ollama"])?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err(InstallError::Unsupported)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ollama_registration_shape() {
        assert_eq!(OLLAMA.command, "ollama");
        let mac = OLLAMA.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("ollama"));
        let win = OLLAMA.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Ollama.Ollama"));
    }
}
