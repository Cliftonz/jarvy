//! pnpm — fast, disk-space-efficient Node.js package manager.
//!
//! Detected via `pnpm-lock.yaml` at the repo root (see the `pnpm` rule
//! in `discover/rules.rs`). Version alignment: `packageManager` in
//! `package.json` + corepack usually pins it project-side; we install
//! the CLI standalone as a fallback for repos that haven't opted in.

use crate::define_tool;

define_tool!(PNPM, {
    command: "pnpm",
    macos: { brew: "pnpm" },
    linux: { brew: "pnpm" },
    windows: { winget: "pnpm.pnpm" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pnpm_registration_shape() {
        assert_eq!(PNPM.command, "pnpm");
        assert_eq!(PNPM.macos.unwrap().brew, Some("pnpm"));
        assert_eq!(PNPM.windows.unwrap().winget, Some("pnpm.pnpm"));
    }
}
