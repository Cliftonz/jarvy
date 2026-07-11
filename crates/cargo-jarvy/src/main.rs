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
    // Validate the name before any filesystem effects — same gate the
    // `jarvy tools --request` path uses. Rejects shapes that would
    // produce broken Rust source (spaces, quotes, control chars, etc.).
    jarvy_templates::validate_tool_name(&name).map_err(|reason| {
        anyhow::anyhow!(
            "invalid tool name `{}`: {}. Must match [A-Za-z0-9._-] and be 1-{} bytes.",
            name,
            reason,
            jarvy_templates::MAX_TOOL_NAME_LEN
        )
    })?;

    // Resolve paths relative to repo root (assume run from root)
    let tools_dir = PathBuf::from("src/tools");

    // Create tool subdirectory (src/tools/<name>/)
    let tool_subdir = tools_dir.join(&name);
    // `definition.rs`, not `<name>.rs` — a file named after its containing
    // directory trips clippy::module_inception (CI gate).
    let target_rs = tool_subdir.join("definition.rs");
    let mod_rs_subdir = tool_subdir.join("mod.rs");

    if target_rs.exists() {
        anyhow::bail!("src/tools/{}/definition.rs already exists", name);
    }

    // Create tool directory if it doesn't exist
    fs::create_dir_all(&tool_subdir)
        .with_context(|| format!("failed creating directory {}", tool_subdir.display()))?;

    // Render the template via the shared helper — single source of
    // truth shared with `jarvy tools --request <name>`. Previously
    // this code re-implemented the substitution and had drifted (the
    // `__PKG_BSD__` placeholder was missing here).
    let contents = jarvy_templates::render_tool_template(&name, bin.as_deref());

    // Write the new tool module
    fs::write(&target_rs, &contents)
        .with_context(|| format!("failed writing {}", target_rs.display()))?;

    // Create mod.rs for the tool subdirectory. The `mod definition;`
    // declaration is required — `pub use definition::*;` alone is E0432.
    let mod_contents = "mod definition;\n#[allow(unused_imports)]\npub use definition::*;\n";
    fs::write(&mod_rs_subdir, mod_contents)
        .with_context(|| format!("failed writing {}", mod_rs_subdir.display()))?;

    // Update parent src/tools/mod.rs to include the new tool module
    let parent_mod_rs = tools_dir.join("mod.rs");
    if parent_mod_rs.exists() {
        let mut mod_body = fs::read_to_string(&parent_mod_rs).unwrap_or_else(|_| String::from(""));
        let decl = format!("pub mod {};", &name);
        if !mod_body.contains(&decl) {
            // Insert into the existing alphabetical `pub mod` block rather
            // than appending at EOF (which lands after `register_all()`).
            let mut before_first_greater = None;
            let mut after_last_decl = None;
            let mut offset = 0usize;
            for line in mod_body.lines() {
                if let Some(existing) = line
                    .strip_prefix("pub mod ")
                    .and_then(|rest| rest.strip_suffix(';'))
                {
                    if existing > name.as_str() && before_first_greater.is_none() {
                        before_first_greater = Some(offset);
                    }
                    after_last_decl = Some(offset + line.len() + 1);
                }
                offset += line.len() + 1;
            }
            match before_first_greater.or(after_last_decl) {
                Some(pos) if pos <= mod_body.len() => {
                    mod_body.insert_str(pos, &format!("{decl}\n"));
                }
                _ => mod_body.push_str(&format!("\n{decl}\n")),
            }
            fs::write(&parent_mod_rs, mod_body)
                .with_context(|| format!("failed updating {}", parent_mod_rs.display()))?;
        }
    } else {
        eprintln!(
            "note: src/tools/mod.rs not found; skipped module declaration. Wire `pub mod {}` manually.",
            &name
        );
    }

    // (Optional) run rustfmt; ignore errors if not available
    let _ = std::process::Command::new("cargo").args(["fmt"]).status();

    println!(
        "✔ Created src/tools/{}/definition.rs using ToolSpec pattern",
        name
    );
    println!("✔ Created src/tools/{}/mod.rs", name);
    println!("✔ Updated src/tools/mod.rs");
    println!();
    println!(
        "→ Edit src/tools/{}/definition.rs to customize package names if needed.",
        name
    );
    println!("→ Update the tool description in the doc comment.");
    println!("→ Run `cargo test --lib` to verify the new tool.");
    Ok(())
}
