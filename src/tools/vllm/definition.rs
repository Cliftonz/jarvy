//! vllm - High-throughput LLM serving engine
//!
//! vLLM is a high-throughput and memory-efficient inference and serving
//! engine for large language models. It supports continuous batching,
//! PagedAttention, and various hardware backends.
//!
//! Note: vLLM requires Python 3.10+ and typically NVIDIA GPU with CUDA.
//! This tool requires Python and uses pip for installation.

use crate::define_tool;
use crate::tools::common::{InstallError, has, run};

define_tool!(VLLM, {
    command: "vllm",
    repo: "vllm-project/vllm",
    custom_install: install_vllm,
});

fn install_vllm(_min_hint: &str) -> Result<(), InstallError> {
    // Check if already installed
    if has("vllm") {
        return Ok(());
    }

    // Check for Python
    let has_python = has("python3") || has("python");
    if !has_python {
        return Err(InstallError::Prereq(
            "Python not found. Install Python 3.10+ and re-run.",
        ));
    }

    // vLLM is best installed with pip directly (not pipx) due to complex dependencies
    let pip_cmd = if has("pip3") { "pip3" } else { "pip" };
    if !has(pip_cmd) {
        return Err(InstallError::Prereq(
            "pip not found. Install pip and re-run.",
        ));
    }

    // Install vLLM - it will pull in PyTorch and other dependencies
    run(pip_cmd, &["install", "vllm"])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vllm_registration_shape() {
        assert_eq!(VLLM.command, "vllm");
    }
}
