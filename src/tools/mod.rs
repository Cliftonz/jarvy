pub mod air;
pub mod atlas;
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
pub mod python;
pub mod registry;
pub mod ripgrep;
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
    InstallError, Os, PackageManager, PkgOps, cmd_satisfies, current_os, has, require, require_any,
    run,
};

#[allow(unused_imports)]
pub use registry::{ToolAdder, add, get_tool, register_tool, registered_tool_names};

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
    let _ = register_tool("nvim", crate::tools::nvim::nvim::add_handler);
    let _ = register_tool("rust", crate::tools::rust::rust::add_handler);
    let _ = register_tool("talosctl", crate::tools::talosctl::talosctl::add_handler);
    let _ = register_tool("python", crate::tools::python::python::add_handler);
    let _ = register_tool("node", crate::tools::node::node::add_handler);
    let _ = register_tool("go", crate::tools::go::go::add_handler);
    let _ = register_tool("awscli", crate::tools::awscli::awscli::add_handler);
    let _ = register_tool("cue", crate::tools::cue::cue::add_handler);
    let _ = register_tool("iterm2", crate::tools::iterm2::iterm2::add_handler);
    let _ = register_tool("tilt", crate::tools::tilt::tilt::add_handler);
    let _ = register_tool("up", crate::tools::up::up::add_handler);
    let _ = register_tool("zsh", crate::tools::zsh::zsh::add_handler);
    let _ = register_tool("atlas", crate::tools::atlas::atlas::add_handler);
    let _ = register_tool("ripgrep", crate::tools::ripgrep::ripgrep::add_handler);
    let _ = register_tool("xz", crate::tools::xz::xz::add_handler);

    // Newly added tools
    let _ = register_tool("air", crate::tools::air::air::add_handler);
    let _ = register_tool("dotnet", crate::tools::dotnet::dotnet::add_handler);
    let _ = register_tool("elixir", crate::tools::elixir::elixir::add_handler);
    let _ = register_tool("gleam", crate::tools::gleam::gleam::add_handler);
}
