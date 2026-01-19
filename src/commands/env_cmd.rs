//! Env command handler - manage environment variables from jarvy.toml

use std::collections::HashMap;

use crate::config::Config;
use crate::env::{
    DotenvConfig, EnvContext, SecretsConfig, ShellConfig, collect_secrets, detect_shell,
    generate_dotenv, parse_shell, preview_dotenv, preview_shell_rc, update_shell_rc,
};
use crate::error_codes;

/// Run the env command
pub fn run_env(
    file: &str,
    dotenv: bool,
    shell: bool,
    dry_run: bool,
    export: bool,
    shell_type: Option<&str>,
    force: bool,
) {
    let config = Config::new(file);
    let env_config = config.get_env();

    // Determine shell type
    let target_shell = if let Some(shell_str) = shell_type {
        match parse_shell(shell_str) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(error_codes::CONFIG_ERROR);
            }
        }
    } else if let Some(ref shell_str) = env_config.config.shell {
        parse_shell(shell_str).unwrap_or_else(|_| detect_shell())
    } else {
        detect_shell()
    };

    // Create context for variable expansion
    let ctx = EnvContext::new();

    // Collect all regular vars
    let vars: HashMap<String, String> = env_config
        .vars
        .iter()
        .map(|(k, v)| (k.clone(), v.value().to_string()))
        .collect();

    // Handle --export flag (output for shell eval)
    if export {
        let preview = preview_shell_rc(target_shell, &vars, &ctx);
        println!("{}", preview);
        return;
    }

    // Collect secrets (in CI mode, won't prompt)
    let secrets_config = SecretsConfig::default();
    let secrets = match collect_secrets(&env_config.secrets, &ctx, &secrets_config) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error collecting secrets: {}", e);
            std::process::exit(error_codes::CONFIG_ERROR);
        }
    };

    // Merge vars and secrets
    let secrets_count = secrets.len();
    let mut all_vars = vars.clone();
    all_vars.extend(secrets);

    // Determine what to do
    let do_dotenv = dotenv || (!shell && env_config.config.generate_dotenv);
    let do_shell = shell || (!dotenv && env_config.config.update_rc);

    if !config.has_env() {
        println!("No environment variables configured in {}", file);
        return;
    }

    // Generate .env file
    if do_dotenv {
        let dotenv_path = &env_config.config.dotenv_path;

        if dry_run {
            println!(
                "=== .env file preview (would be written to {}) ===",
                dotenv_path.display()
            );
            let content = preview_dotenv(&all_vars, &ctx);
            println!("{}", content);
        } else {
            let dotenv_config = DotenvConfig {
                backup: true,
                force,
                add_to_gitignore: env_config.config.add_to_gitignore,
            };

            match generate_dotenv(dotenv_path, &all_vars, &ctx, &dotenv_config) {
                Ok(()) => {
                    println!("Generated .env file at: {}", dotenv_path.display());
                }
                Err(e) => {
                    eprintln!("Failed to generate .env file: {}", e);
                    if !force {
                        eprintln!("Tip: Use --force to overwrite existing non-Jarvy .env files");
                    }
                    std::process::exit(error_codes::CONFIG_ERROR);
                }
            }
        }
    }

    // Update shell rc file
    if do_shell {
        if dry_run {
            println!("\n=== Shell rc preview ({}) ===", target_shell);
            let preview = preview_shell_rc(target_shell, &vars, &ctx);
            println!("{}", preview);
        } else {
            let shell_config = ShellConfig {
                backup: env_config.config.backup_rc,
                validate: false,
            };

            match update_shell_rc(target_shell, &vars, &ctx, &shell_config) {
                Ok(path) => {
                    println!("Updated shell rc file: {}", path.display());
                    println!(
                        "Tip: Run 'source {}' or restart your shell to apply changes",
                        path.display()
                    );
                }
                Err(e) => {
                    eprintln!("Failed to update shell rc file: {}", e);
                    std::process::exit(error_codes::CONFIG_ERROR);
                }
            }
        }
    }

    // Summary
    if !dry_run {
        println!("\nEnvironment configuration applied:");
        println!("  - Variables: {}", vars.len());
        if secrets_count > 0 {
            println!("  - Secrets: configured");
        }
    }
}
