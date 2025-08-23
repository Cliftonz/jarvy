use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::{fs, path::PathBuf};

#[derive(Parser)]
#[command(name = "cargo-jarvy", version)]
#[command(about = "Jarvy workspace subcommands")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Create a new tool: generates src/tools/<name>.rs and updates mod.rs
    NewTool {
        /// Tool name (e.g., git, docker, nvm)
        name: String,
        /// Optional binary to probe (defaults to name)
        #[arg(long)]
        bin: Option<String>,
    },
}

fn main() -> Result<()> {
    let Cli { cmd } = Cli::parse();
    match cmd {
        Cmd::NewTool { name, bin } => new_tool(name, bin)?,
    }
    Ok(())
}

fn new_tool(name: String, bin: Option<String>) -> Result<()> {
    // Resolve paths relative to repo root (assume run from root)
    let tools_dir = PathBuf::from("src/tools");
    let mod_rs = tools_dir.join("mod.rs");
    let template = tools_dir.join("_template.rs");
    let target_rs = tools_dir.join(format!("{}.rs", &name));

    if target_rs.exists() {
        anyhow::bail!("src/tools/{}.rs already exists", name);
    }

    // Read template
    let mut contents = fs::read_to_string(&template)
        .with_context(|| format!("missing template at {}", template.display()))?;

    // Substitute placeholders
    let tool_mod = name.to_string(); // snake_case by convention
    let tool_bin = bin.unwrap_or_else(|| name.clone()); // default probe bin
    contents = contents
        .replace("__TOOL_MOD__", &tool_mod)
        .replace("__TOOL_BIN__", &tool_bin)
        .replace("__PKG_BREW__", &tool_mod) // sane default
        .replace("__PKG_CASK__", &tool_mod)
        .replace("__PKG_LINUX__", &tool_mod)
        .replace("__PKG_WINGET_ID__", &tool_mod);

    // Write the new tool module
    fs::write(&target_rs, contents)
        .with_context(|| format!("failed writing {}", target_rs.display()))?;

    // If src/tools/mod.rs exists, declare the module; otherwise, skip (project may be using flat src/tools.rs)
    if mod_rs.exists() {
        let mut mod_body = fs::read_to_string(&mod_rs).unwrap_or_else(|_| String::from(""));
        let decl = format!("pub mod {};\n", &tool_mod);
        if !mod_body.contains(&decl) {
            mod_body.push_str(&decl);
            fs::write(&mod_rs, mod_body)
                .with_context(|| format!("failed updating {}", mod_rs.display()))?;
        }
    } else {
        eprintln!(
            "note: src/tools/mod.rs not found; skipped module declaration. If you're using a flat src/tools.rs, wire `pub mod {}` manually.",
            &tool_mod
        );
    }

    // (Optional) run rustfmt; ignore errors if not available
    let _ = std::process::Command::new("cargo").args(["fmt"]).status();

    println!("✔ Created {} and updated tools/mod.rs", target_rs.display());
    println!(
        "→ Open {} and fill in per-OS installers.",
        target_rs.display()
    );
    Ok(())
}
