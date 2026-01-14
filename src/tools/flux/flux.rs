//! flux - GitOps toolkit for Kubernetes
//!
//! This tool uses the ToolSpec pattern for declarative installation.
//! Note: Linux/Windows may require custom installation.

use crate::define_tool;

define_tool!(FLUX, {
    command: "flux",
    macos: { brew: "fluxcd/tap/flux" },
    linux: { brew: "fluxcd/tap/flux" },
    windows: { winget: "Fluxcd.Flux" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_flux_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
