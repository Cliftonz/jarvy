//! pnpm — fast, disk-space-efficient Node.js package manager.
//!
//! Detected via `pnpm-lock.yaml` at the repo root (see the `pnpm` rule
//! in `discover/rules.rs`). Version alignment: `packageManager` in
//! `package.json` + corepack usually pins it project-side; we install
//! the CLI standalone as a fallback for repos that haven't opted in.

use crate::define_tool;

define_tool!(PNPM, {
    command: "pnpm",
    repo: "pnpm/pnpm",
    macos: { brew: "pnpm" },
    linux: { brew: "pnpm" },
    windows: { winget: "pnpm.pnpm" },
});

// Registration-shape test deleted per Maint F3 — `define_tool!`
// already type-checks the field assignments at compile time.
