//! WSL2 bootstrap + delegation.
//!
//! Ships a `wsl` tool that (a) installs Store-WSL2 plus a named distro
//! instance on Windows, (b) optionally curl-pipes jarvy inside the
//! distro, (c) optionally delegates `jarvy setup` into the distro on
//! the translated project path (`C:\...` -> `/mnt/c/...`).
//!
//! Everything is opt-in via `[windows.wsl] enabled = true`. Jarvy
//! never triggers UAC — when elevation is required (first-time WSL
//! platform install), jarvy prints the elevated `wsl --install`
//! command and refuses. See `src/windows/config.rs::WslConfig`.
//!
//! Non-Windows targets: the `wsl` tool returns `Unsupported` so it
//! stays visible in `jarvy tools list` and `jarvy diagnose wsl`
//! without misfiring during cross-platform config validation.

pub mod bootstrap;
pub mod definition;
pub mod refusal;
pub mod rootfs;

#[allow(unused_imports)] // Re-exported for parity with other tool modules
pub use definition::WSL;
