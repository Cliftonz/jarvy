pub mod brew;
pub mod common;
pub mod docker;
pub mod git;
pub mod htop;
pub mod jq;
pub mod k6;
pub mod make;
pub mod ngrok;
pub mod nvm;
pub mod opentofu;
pub mod packer;
pub mod registry;
pub mod terraform;
pub mod tmux;
pub mod tree;
pub mod vscode;
pub mod wget;
pub mod yq;

#[allow(unused_imports)]
pub use common::{
    InstallError, Os, PackageManager, PkgOps, cmd_satisfies, current_os, has, require, require_any,
    run,
};

#[allow(unused_imports)]
pub use registry::{ToolAdder, add, get_tool, register_tool};

/// Register all built-in tools to the registry. Call this early in the program init if you
/// want to add("git", ...)/add("docker", ...) to work without manual registration.
pub fn register_all() {
    // Each tool exposes a small add_handler(version) function we can register.
    // Ignore duplicate returns; the last one wins.
    let _ = register_tool("git", crate::tools::git::git::add_handler);
    let _ = register_tool("brew", crate::tools::brew::brew::add_handler);
    let _ = register_tool("vscode", crate::tools::vscode::vscode::add_handler);
    let _ = register_tool("docker", crate::tools::docker::docker::add_handler);
    let _ = register_tool("wget", crate::tools::wget::wget::add_handler);
    let _ = register_tool("jq", crate::tools::jq::jq::add_handler);
    let _ = register_tool("nvm", crate::tools::nvm::nvm::add_handler);
    let _ = register_tool("tree", crate::tools::tree::tree::add_handler);
    let _ = register_tool("tmux", crate::tools::tmux::tmux::add_handler);
    let _ = register_tool("htop", crate::tools::htop::htop::add_handler);
    let _ = register_tool("opentofu", crate::tools::opentofu::opentofu::add_handler);
    let _ = register_tool("terraform", crate::tools::terraform::terraform::add_handler);
    let _ = register_tool("packer", crate::tools::packer::packer::add_handler);
    let _ = register_tool("yq", crate::tools::yq::yq::add_handler);
    let _ = register_tool("make", crate::tools::make::make::add_handler);
    let _ = register_tool("k6", crate::tools::k6::k6::add_handler);
    let _ = register_tool("ngrok", crate::tools::ngrok::ngrok::add_handler);
}
