use crate::tools;
use crate::tools::common::InstallContext;

/// Install a configurable set of tools, registering the tool index first.
pub fn bootstrap_tools(tasks: &[(&str, &str)]) {
    tools::register_all();

    let ctx = InstallContext::none();
    for (name, hint) in tasks {
        match tools::add(name, hint, &ctx) {
            Ok(_) => println!("✅ {} is installed or now installed", name),
            Err(e) => eprintln!("⛔ {} install/ensure failed: {}", name, e),
        }
    }
}

/// Bootstrap a machine with base tools required across OSes.
/// Installs or ensures the presence of git, docker, and vscode.
///
/// This function aims to be idempotent and will attempt installation only when
/// the tool is missing or version hint is not satisfied.
pub fn bootstrap() {
    bootstrap_tools(&[("git", ""), ("docker", ""), ("vscode", "")]);
}
