//! litellm - Unified LLM API proxy
//!
//! LiteLLM is a Python SDK and proxy server that provides a unified API
//! to call 100+ LLM providers (OpenAI, Anthropic, Azure, Bedrock, etc.)
//! using the OpenAI format.
//!
//! This tool requires Python and uses pip/pipx for installation.

use crate::define_tool;
use crate::tools::common::{InstallError, has, run};

define_tool!(LITELLM, {
    command: "litellm",
    custom_install: install_litellm,
});

fn install_litellm(_min_hint: &str) -> Result<(), InstallError> {
    // Check if already installed
    if has("litellm") {
        return Ok(());
    }

    // Check for Python
    let has_python = has("python3") || has("python");
    if !has_python {
        return Err(InstallError::Prereq(
            "Python not found. Install Python 3.9+ and re-run.",
        ));
    }

    // Prefer pipx for isolated installation, fall back to pip
    if has("pipx") {
        run("pipx", &["install", "litellm"])?;
        return Ok(());
    }

    // Fall back to pip
    let pip_cmd = if has("pip3") { "pip3" } else { "pip" };
    if !has(pip_cmd) {
        return Err(InstallError::Prereq(
            "pip not found. Install pip and re-run, or install pipx for isolated installation.",
        ));
    }

    run(pip_cmd, &["install", "--user", "litellm"])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_litellm_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
