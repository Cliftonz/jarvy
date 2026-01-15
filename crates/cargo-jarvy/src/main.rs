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
    /// Create a new tool: generates src/provisioner/<name>.rs and updates mod.rs
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
    let template = tools_dir.join("_template.rs");

    // Create tool subdirectory (src/tools/<name>/)
    let tool_subdir = tools_dir.join(&name);
    let target_rs = tool_subdir.join(format!("{}.rs", &name));
    let mod_rs_subdir = tool_subdir.join("mod.rs");

    if target_rs.exists() {
        anyhow::bail!("src/tools/{}/{}.rs already exists", name, name);
    }

    // Create tool directory if it doesn't exist
    fs::create_dir_all(&tool_subdir)
        .with_context(|| format!("failed creating directory {}", tool_subdir.display()))?;

    // Read template
    let mut contents = fs::read_to_string(&template)
        .with_context(|| format!("missing template at {}", template.display()))?;

    // Substitute placeholders
    let tool_mod = name.to_string(); // snake_case by convention
    let tool_bin = bin.unwrap_or_else(|| name.clone()); // default probe bin
    let tool_upper = name.to_uppercase(); // TOOL_NAME for static
    let tool_desc = format!("{} tool", &name); // default description

    contents = contents
        .replace("__TOOL_MOD__", &tool_mod)
        .replace("__TOOL_BIN__", &tool_bin)
        .replace("__TOOL_UPPER__", &tool_upper)
        .replace("__TOOL_DESC__", &tool_desc)
        .replace("__PKG_BREW__", &tool_mod) // sane default
        .replace("__PKG_LINUX__", &tool_mod)
        .replace("__PKG_WINGET_ID__", &tool_mod);

    // Write the new tool module
    fs::write(&target_rs, &contents)
        .with_context(|| format!("failed writing {}", target_rs.display()))?;

    // Create mod.rs for the tool subdirectory
    let mod_contents = format!("pub use {}::*;\n", &tool_mod);
    fs::write(&mod_rs_subdir, &mod_contents)
        .with_context(|| format!("failed writing {}", mod_rs_subdir.display()))?;

    // Update parent src/tools/mod.rs to include the new tool module
    let parent_mod_rs = tools_dir.join("mod.rs");
    if parent_mod_rs.exists() {
        let mut mod_body = fs::read_to_string(&parent_mod_rs).unwrap_or_else(|_| String::from(""));
        let decl = format!("pub mod {};", &tool_mod);
        if !mod_body.contains(&decl) {
            // Insert before the last line or at end
            mod_body.push_str(&format!("\npub mod {};\n", &tool_mod));
            fs::write(&parent_mod_rs, mod_body)
                .with_context(|| format!("failed updating {}", parent_mod_rs.display()))?;
        }
    } else {
        eprintln!(
            "note: src/tools/mod.rs not found; skipped module declaration. Wire `pub mod {}` manually.",
            &tool_mod
        );
    }

    // (Optional) run rustfmt; ignore errors if not available
    let _ = std::process::Command::new("cargo").args(["fmt"]).status();

    println!(
        "✔ Created src/tools/{}/{}.rs using ToolSpec pattern",
        name, name
    );
    println!("✔ Created src/tools/{}/mod.rs", name);
    println!("✔ Updated src/tools/mod.rs");
    println!();
    println!(
        "→ Edit src/tools/{}/{}.rs to customize package names if needed.",
        name, name
    );
    println!("→ Update the tool description in the doc comment.");
    println!("→ Run `cargo test --lib` to verify the new tool.");
    Ok(())
}
