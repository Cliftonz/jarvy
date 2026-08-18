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
    use crate::tools::spec::order_tools_by_dependencies;

    #[test]
    fn pnpm_declares_node_runtime_dependency() {
        assert_eq!(PNPM.depends_on, Some(&["node"][..]));
    }

    #[test]
    fn pnpm_installs_after_node_regardless_of_config_order() {
        // Behavioral guard — shape-only test (above) doesn't catch a
        // topo-sort regression. Feed the tools in reverse order and
        // verify the ordering primitive schedules node first.
        let inputs: Vec<(&str, &str)> = vec![("pnpm", "latest"), ("node", "20")];
        let ordered = order_tools_by_dependencies(inputs.into_iter());
        let node_i = ordered
            .iter()
            .position(|(n, _)| n == "node")
            .expect("node scheduled");
        let pnpm_i = ordered
            .iter()
            .position(|(n, _)| n == "pnpm")
            .expect("pnpm scheduled");
        assert!(
            node_i < pnpm_i,
            "node must install before pnpm; got order {ordered:?}"
        );
    }
}
