//! `wsl` tool spec. Windows-only in behavior; declared without a
//! `windows` install slot because bootstrap goes through
//! `custom_install`. Non-Windows targets short-circuit to
//! `InstallError::Unsupported` so the tool stays visible in
//! `jarvy tools list` / `jarvy diagnose wsl` without misfiring.
//!
//! The tool is inert unless the outer config has
//! `[windows.wsl] enabled = true`. The setup dispatcher populates the
//! active config via [`super::bootstrap::set_active_config`] before
//! tools install; standalone `jarvy tools install wsl` invocations
//! fall through to `WslConfig::default()` and refuse cleanly.

use crate::define_tool;
use crate::tools::common::{InstallContext, InstallError};

#[cfg(target_os = "windows")]
use super::bootstrap;
#[cfg(target_os = "windows")]
use super::refusal::RefusalReason;

fn bootstrap_wsl(_min_hint: &str, _ctx: &InstallContext) -> Result<(), InstallError> {
    #[cfg(target_os = "windows")]
    {
        let cfg = bootstrap::active_config().unwrap_or_default();
        if !cfg.enabled {
            // Print the opt-in refusal so users running
            // `jarvy tools install wsl` see the reason; the setup
            // path never reaches here because the outer dispatcher
            // gates on `enabled` first.
            eprintln!(
                "{}",
                bootstrap::render_refusal(&cfg, RefusalReason::NotOptedIn)
            );
            return Err(InstallError::Unsupported);
        }
        match bootstrap::bootstrap(&cfg) {
            Ok(bootstrap::BootstrapOutcome::NoOp) => {
                if cfg.install_jarvy {
                    if let Err(e) = bootstrap::install_jarvy_inside(&cfg) {
                        eprintln!(
                            "[wsl] jarvy install inside \"{}\" failed: {e}",
                            cfg.effective_name()
                        );
                    }
                }
                Ok(())
            }
            Ok(bootstrap::BootstrapOutcome::Installed { .. }) => {
                if cfg.install_jarvy {
                    if let Err(e) = bootstrap::install_jarvy_inside(&cfg) {
                        eprintln!(
                            "[wsl] jarvy install inside \"{}\" failed: {e}",
                            cfg.effective_name()
                        );
                    }
                }
                Ok(())
            }
            Err(reason) => {
                eprintln!("{}", bootstrap::render_refusal(&cfg, reason));
                Err(InstallError::Unsupported)
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err(InstallError::Unsupported)
    }
}

define_tool!(WSL, {
    command: "wsl",
    custom_install: bootstrap_wsl,
    category: "runtime",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wsl_registration_shape() {
        assert_eq!(WSL.command, "wsl");
        assert!(WSL.custom_install.is_some());
        assert_eq!(WSL.category, Some("runtime"));
    }
}
