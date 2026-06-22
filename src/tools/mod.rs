pub mod act;
pub mod actionlint;
pub mod age;
pub mod air;
pub mod ansible;
pub mod arctl;
pub mod argo;
pub mod argocd;
pub mod aria2;
pub mod atlas;
pub mod atuin;
pub mod awscli;
pub mod azd;
pub mod azure_cli;
pub mod bat;
pub mod betterleaks;
pub mod bicep;
pub mod bottom;
pub mod brew;
pub mod broot;
pub mod btop;
pub mod buf;
pub mod buildah;
pub mod bun;
pub mod checkov;
pub mod choose;
pub mod cloudflared;
pub mod common;
pub mod cosign;
pub mod crane;
pub mod crystal;
pub mod cue;
pub mod curl;
pub mod cursor;
pub mod dagger;
pub mod dapr;
pub mod dbmate;
pub mod delta;
pub mod delve;
pub mod deno;
pub mod detect_secrets;
pub mod dfc;
pub mod direnv;
pub mod dive;
pub mod docker;
pub mod docker_desktop;
pub mod dog;
pub mod dotnet;
pub mod duckdb;
pub mod duf;
pub mod dust;
pub mod earthly;
pub mod eksctl;
pub mod elixir;
pub mod emacs;
pub mod emqx;
pub mod erlang;
pub mod eza;
pub mod fd;
pub mod flux;
pub mod freelens;
pub mod fzf;
pub mod gcloud;
pub mod gh;
pub mod git;
pub mod git_lfs;
pub mod git_secrets;
pub mod gitleaks;
pub mod gitversion;
pub mod glab;
pub mod gleam;
pub mod go;
pub mod gofumpt;
pub mod golangci_lint;
pub mod gopls;
pub mod goreleaser;
pub mod gotestsum;
pub mod govulncheck;
pub mod gping;
pub mod grafanactl;
pub mod grex;
pub mod grpcurl;
pub mod grype;
pub mod hadolint;
pub mod haskell;
pub mod headscale;
pub mod helix;
pub mod helm;
pub mod htop;
pub mod httpie;
pub mod hugo;
pub mod hyperfine;
pub mod infracost;
pub mod install_method;
pub mod iterm2;
pub mod java;
pub mod jetbrains_toolbox;
pub mod jq;
pub mod julia;
pub mod just;
pub mod k3d;
pub mod k6;
pub mod k9s;
pub mod kaf;
pub mod kafka;
pub mod kafkactl;
pub mod kagent;
pub mod kcat;
pub mod kind;
pub mod kmcp;
pub mod kn;
pub mod ko;
pub mod kotlin;
pub mod krew;
pub mod kubectl;
pub mod kubectx;
pub mod kubens;
pub mod kubescape;
pub mod kustomize;
pub mod lazydocker;
pub mod lazygit;
pub mod litecli;
pub mod litellm;
pub mod lnav;
pub mod localstack;
pub mod lsd;
pub mod lua;
pub mod luarocks;
pub mod lynis;
pub mod make;
pub mod micro;
pub mod minikube;
pub mod mise;
pub mod mockgen;
pub mod molecule;
pub mod mongosh;
pub mod mosquitto;
pub mod mssql_cli;
pub mod mtr;
pub mod mycli;
pub mod mysql;
pub mod nats;
pub mod nats_server;
pub mod ncdu;
pub mod nebula;
pub mod nerdctl;
pub mod netbird;
pub mod ngrok;
pub mod nim;
// `nk` removed (security review F2): the `nats-io/nats-tools` brew tap
// does not contain `nk.rb`, no Debian / RHEL package by that name
// exists, and the `Synadia.NK` winget id was never claimed in
// microsoft/winget-pkgs. Re-add when upstream ships a real package.
pub mod nmap;
pub mod node;
pub mod noseyparker;
pub mod nsc;
pub mod nushell;
pub mod nvim;
pub mod nvm;
pub mod ocaml;
pub mod ollama;
pub mod omnictl;
pub mod openclaw;
pub mod openssh;
pub mod opentofu;
pub mod openvpn;
pub mod oras;
pub mod p7zip;
pub mod packer;
pub mod pgcli;
pub mod php;
pub mod pinned_binary_installer;
pub mod pinned_installer;
pub mod plugins;
pub mod podman;
pub mod podman_desktop;
pub mod powershell;
pub mod pre_commit;
pub mod procs;
pub mod psql;
pub mod pulsar;
pub mod pulumi;
pub mod pyenv;
pub mod python;
pub mod rabbitmq;
pub mod rancher_desktop;
pub mod rbenv;
pub mod rclone;
pub mod redis;
pub mod registry;
pub mod ripgrep;
pub mod rpk;
pub mod ruby;
pub mod ruff;
pub mod rust;
pub mod scala;
pub mod sd;
pub mod sdkman;
pub mod semgrep;
pub mod shellcheck;
pub mod shfmt;
pub mod skopeo;
pub mod sonar_scanner;
pub mod sops;
pub mod spec;
pub mod sqlite;
pub mod starship;
pub mod stern;
pub mod syft;
pub mod tailscale;
pub mod talisman;
pub mod talosctl;
pub mod temporal;
pub mod terraform;
pub mod terraform_docs;
pub mod terragrunt;
pub mod tfsec;
pub mod tilt;
pub mod tmux;
pub mod tokei;
pub mod tree;
pub mod trivy;
pub mod trufflehog;
pub mod twingate;
pub mod unsupported;
pub mod up;
pub mod usql;
pub mod vagrant;
pub mod vault;
pub mod version;
pub mod vfox;
pub mod vim;
pub mod vllm;
pub mod vscode;
pub mod watchexec;
pub mod wget;
pub mod wireguard_tools;
pub mod xz;
pub mod yamllint;
pub mod yazi;
pub mod yq;
pub mod zed;
pub mod zerotier;
pub mod zig;
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
    let _ = register_tool("nvm", crate::tools::nvm::add_handler);
    let _ = register_tool("rust", crate::tools::rust::add_handler);
    let _ = register_tool("brew", crate::tools::brew::add_handler);

    // Load user-defined plugin tools from ~/.jarvy/tools.d/
    let plugin_count = plugins::load_user_tools();
    if plugin_count > 0 {
        tracing::info!(
            "Loaded {} plugin tool(s) from ~/.jarvy/tools.d/",
            plugin_count
        );
    }
}
