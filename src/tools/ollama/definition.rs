//! ollama - Run large language models locally
//!
//! Ollama is a tool for running open-source large language models locally.
//! It supports models like Llama, Mistral, Gemma, and more.
//!
//! This tool uses the ToolSpec pattern with a custom installer for Linux.

use crate::define_tool;
use crate::tools::common::{InstallError, has, run};

define_tool!(OLLAMA, {
    command: "ollama",
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

        // Fall back to official install script
        run(
            "bash",
            &["-c", "curl -fsSL https://ollama.com/install.sh | sh"],
        )?;
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
    fn ensure_ollama_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
