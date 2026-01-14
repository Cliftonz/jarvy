pub mod air;
pub mod atlas;
pub mod spec;
pub mod version;
pub mod awscli;
pub mod brew;
pub mod common;
pub mod cue;
pub mod docker;
pub mod dotnet;
pub mod elixir;
pub mod git;
pub mod gleam;
pub mod go;
pub mod htop;
pub mod iterm2;
pub mod jq;
pub mod k6;
pub mod make;
pub mod ngrok;
pub mod node;
pub mod nvim;
pub mod nvm;
pub mod opentofu;
pub mod packer;
pub mod php;
pub mod powershell;
pub mod python;
pub mod registry;
pub mod ripgrep;
pub mod ruby;
pub mod rust;
pub mod talosctl;
pub mod terraform;
pub mod tilt;
pub mod tmux;
pub mod tree;
pub mod up;
pub mod vscode;
pub mod wget;
pub mod xz;
pub mod yq;
pub mod zsh;

#[allow(unused_imports)]
pub use common::{
    InstallError, Os, PackageManager, PkgOps, cmd_satisfies, current_os, default_use_sudo, has,
    require, require_any, run, set_default_use_sudo,
};

#[allow(unused_imports)]
pub use registry::{ToolAdder, add, get_tool, register_tool, registered_tool_names};

/// Register all built-in tools to the registry. Call this early in the program init if you
/// want to add("git", ...)/add("docker", ...) to work without manual registration.
///
/// Tools defined with `define_tool!` are automatically discovered via the `inventory` crate.
/// Tools with custom installation logic (nvm, rust, brew) are registered manually.
pub fn register_all() {
    // Auto-register all tools defined with define_tool! macro
    // The inventory crate collects these at compile time
    for entry in spec::iter_tools() {
        let _ = register_tool(entry.spec.name, entry.handler);
    }

    // Manual registration for tools with custom installers
    // These tools don't use the ToolSpec pattern due to complex installation logic
    let _ = register_tool("nvm", crate::tools::nvm::nvm::add_handler);
    let _ = register_tool("rust", crate::tools::rust::rust::add_handler);
    let _ = register_tool("brew", crate::tools::brew::brew::add_handler);
}
