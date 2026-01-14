pub mod actionlint;
pub mod age;
pub mod air;
pub mod argocd;
pub mod aria2;
pub mod atlas;
pub mod awscli;
pub mod azure_cli;
pub mod bat;
pub mod bottom;
pub mod brew;
pub mod btop;
pub mod common;
pub mod cue;
pub mod curl;
pub mod delta;
pub mod direnv;
pub mod docker;
pub mod dotnet;
pub mod duf;
pub mod eksctl;
pub mod elixir;
pub mod eza;
pub mod fd;
pub mod flux;
pub mod fzf;
pub mod gh;
pub mod git;
pub mod git_lfs;
pub mod glab;
pub mod gleam;
pub mod go;
pub mod hadolint;
pub mod helm;
pub mod htop;
pub mod httpie;
pub mod hugo;
pub mod iterm2;
pub mod jq;
pub mod just;
pub mod k6;
pub mod k9s;
pub mod kind;
pub mod kubectl;
pub mod kubectx;
pub mod kubescape;
pub mod kustomize;
pub mod lazygit;
pub mod lnav;
pub mod lynis;
pub mod make;
pub mod minikube;
pub mod mongosh;
pub mod mtr;
pub mod mysql;
pub mod ncdu;
pub mod ngrok;
pub mod nmap;
pub mod node;
pub mod nvim;
pub mod nvm;
pub mod openssh;
pub mod opentofu;
pub mod p7zip;
pub mod packer;
pub mod php;
pub mod podman;
pub mod powershell;
pub mod procs;
pub mod psql;
pub mod pulumi;
pub mod python;
pub mod rclone;
pub mod redis;
pub mod registry;
pub mod ripgrep;
pub mod ruby;
pub mod ruff;
pub mod rust;
pub mod shellcheck;
pub mod shfmt;
pub mod sops;
pub mod spec;
pub mod sqlite;
pub mod starship;
pub mod stern;
pub mod talosctl;
pub mod terraform;
pub mod tfsec;
pub mod tilt;
pub mod tmux;
pub mod tree;
pub mod trivy;
pub mod up;
pub mod vault;
pub mod version;
pub mod vscode;
pub mod wget;
pub mod xz;
pub mod yamllint;
pub mod yq;
pub mod zoxide;
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
