//! Team command handler - manage team configuration sources

use std::fs;

use crate::cli::TeamAction;
use crate::remote::fetch_remote_config;
use crate::team;

/// Handle team subcommands
pub fn run_team(action: &TeamAction) {
    use team::registry::Registry;

    match action {
        TeamAction::Add {
            name,
            url,
            description,
        } => {
            let mut registry = Registry::load();
            match registry.add_source(name, url, description.as_deref()) {
                Ok(()) => {
                    if let Err(e) = registry.save() {
                        eprintln!("Warning: Failed to save registry: {}", e);
                    }
                    println!("Added team source '{}' -> {}", name, url);
                    println!("Run 'jarvy team sync {}' to fetch available configs.", name);
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        TeamAction::List {} => {
            let registry = Registry::load();
            let sources = registry.list_sources();

            if sources.is_empty() {
                println!("No team sources registered.");
                println!("Add one with: jarvy team add <name> <url>");
                return;
            }

            println!("Team Configuration Sources");
            println!("==========================");
            for source in sources {
                println!();
                println!("  {} ({})", source.name, source.url);
                if let Some(ref desc) = source.description {
                    println!("    {}", desc);
                }
                if let Some(last_sync) = source.last_sync {
                    let ago = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs().saturating_sub(last_sync))
                        .unwrap_or(0);
                    println!(
                        "    Last sync: {}s ago ({} configs)",
                        ago,
                        source.configs.len()
                    );
                } else {
                    println!("    Not synced yet");
                }
            }
        }
        TeamAction::Browse { source } => {
            let registry = Registry::load();
            match registry.get_source(source) {
                Some(src) => {
                    if src.configs.is_empty() {
                        println!(
                            "No configs found for '{}'. Run 'jarvy team sync {}' first.",
                            source, source
                        );
                        return;
                    }
                    println!("Available configs from '{}':", source);
                    println!();
                    for config in &src.configs {
                        println!("  {}/{}", source, config.name);
                        if let Some(ref desc) = config.description {
                            println!("    {}", desc);
                        }
                        if !config.tags.is_empty() {
                            println!("    Tags: {}", config.tags.join(", "));
                        }
                    }
                }
                None => {
                    eprintln!("Source '{}' not found.", source);
                    std::process::exit(1);
                }
            }
        }
        TeamAction::Sync { source } => {
            let mut registry = Registry::load();

            let sources_to_sync: Vec<String> = match source {
                Some(s) => vec![s.clone()],
                None => registry.sources.keys().cloned().collect(),
            };

            if sources_to_sync.is_empty() {
                println!("No sources to sync.");
                return;
            }

            for source_name in sources_to_sync {
                print!("Syncing '{}'... ", source_name);
                match registry.sync_source(&source_name) {
                    Ok(count) => {
                        println!("found {} configs", count);
                    }
                    Err(e) => {
                        println!("failed: {}", e);
                    }
                }
            }

            if let Err(e) = registry.save() {
                eprintln!("Warning: Failed to save registry: {}", e);
            }
        }
        TeamAction::Remove { name } => {
            let mut registry = Registry::load();
            match registry.remove_source(name) {
                Ok(_) => {
                    if let Err(e) = registry.save() {
                        eprintln!("Warning: Failed to save registry: {}", e);
                    }
                    println!("Removed team source '{}'", name);
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        TeamAction::Init { from, output } => {
            let registry = Registry::load();
            match registry.get_config_url(from) {
                Some(url) => {
                    println!("Fetching config from {}...", url);
                    match fetch_remote_config(&url, false, &[]) {
                        Ok(cached_path) => {
                            // Copy to output location
                            if let Err(e) = fs::copy(&cached_path, output) {
                                eprintln!("Failed to write config: {}", e);
                                std::process::exit(1);
                            }
                            println!("Created {} from {}", output, from);
                        }
                        Err(e) => {
                            eprintln!("Error fetching config: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                None => {
                    eprintln!(
                        "Config '{}' not found. Use 'jarvy team browse <source>' to see available configs.",
                        from
                    );
                    std::process::exit(1);
                }
            }
        }
    }
}
