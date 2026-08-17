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
    // pnpm is a Node CLI even when its package-manager formula happens
    // to pull Node transitively. Model the real runtime dependency so a
    // fresh-machine plan cannot claim pnpm is complete without Node.
    depends_on: &["node"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pnpm_declares_node_runtime_dependency() {
        assert_eq!(PNPM.depends_on, Some(&["node"][..]));
    }
}
