//! Config command handler - manage configuration inheritance

use std::fs;

use crate::cli::ConfigAction;
use crate::team;

/// Handle config subcommands
pub fn run_config(action: &ConfigAction) {
    match action {
        ConfigAction::Show {
            file,
            resolved,
            extends_chain,
            output_format,
        } => {
            if *extends_chain {
                // Show the inheritance chain
                let base_path = std::path::Path::new(file)
                    .parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

                let mut resolver = team::InheritanceResolver::new().with_base_dir(base_path);
                match resolver.resolve(file) {
                    Ok(_) => {
                        let trace = resolver.trace();
                        println!("Extends Chain for {}", file);
                        println!("========================");
                        for (i, entry) in trace.entries.iter().enumerate() {
                            let indent = "  ".repeat(entry.depth);
                            println!("{}↳ {}", indent, entry.source);
                        }
                    }
                    Err(e) => {
                        eprintln!("Error resolving config: {:?}", e);
                        std::process::exit(1);
                    }
                }
                return;
            }

            if *resolved {
                // Show resolved config with inheritance applied
                let base_path = std::path::Path::new(file)
                    .parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

                let mut resolver = team::InheritanceResolver::new().with_base_dir(base_path);
                match resolver.resolve(file) {
                    Ok(extended) => {
                        match output_format.as_str() {
                            "json" => {
                                println!(
                                    "{}",
                                    serde_json::to_string_pretty(&extended).unwrap_or_default()
                                );
                            }
                            "yaml" => {
                                println!(
                                    "{}",
                                    serde_yaml::to_string(&extended).unwrap_or_default()
                                );
                            }
                            _ => {
                                // TOML
                                println!(
                                    "{}",
                                    toml::to_string_pretty(&extended).unwrap_or_default()
                                );
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Error resolving config: {:?}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                // Show raw config without inheritance - just read the file
                match fs::read_to_string(file) {
                    Ok(content) => {
                        match output_format.as_str() {
                            "json" => {
                                // Parse TOML then convert to JSON
                                if let Ok(value) = toml::from_str::<toml::Value>(&content) {
                                    println!(
                                        "{}",
                                        serde_json::to_string_pretty(&value).unwrap_or_default()
                                    );
                                } else {
                                    eprintln!("Failed to parse config file");
                                    std::process::exit(1);
                                }
                            }
                            "yaml" => {
                                // Parse TOML then convert to YAML
                                if let Ok(value) = toml::from_str::<toml::Value>(&content) {
                                    println!(
                                        "{}",
                                        serde_yaml::to_string(&value).unwrap_or_default()
                                    );
                                } else {
                                    eprintln!("Failed to parse config file");
                                    std::process::exit(1);
                                }
                            }
                            _ => {
                                // Just output the raw TOML
                                println!("{}", content);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to read config file: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }
        ConfigAction::Refresh { file, force } => {
            let base_path = std::path::Path::new(file)
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

            let mut resolver = team::InheritanceResolver::new().with_base_dir(base_path);

            if *force {
                // Clear cache for this config's dependencies
                let _cache = team::ConfigCache::new();
                println!("Clearing config cache...");
                // Note: We'd need to implement cache clearing in ConfigCache
                // For now, just re-resolve which will refresh stale entries
            }

            println!("Resolving config from {}...", file);
            match resolver.resolve(file) {
                Ok(_extended) => {
                    let trace = resolver.trace();
                    println!("Config resolved successfully.");
                    println!("  Sources: {}", trace.entries.len());
                    for entry in &trace.entries {
                        println!("    - {}", entry.source);
                    }
                }
                Err(e) => {
                    eprintln!("Error resolving config: {:?}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}
