//! Handler for `jarvy run` — execute a named command from the `[commands]`
//! section of jarvy.toml, npm-run style.
//!
//! `jarvy run` with no name lists the available commands (`--format json`
//! supported); `jarvy run <name> [-- extra args]` executes one and
//! propagates the child's exit code. Commands run with the config file's
//! directory as cwd, so `--file ../other/jarvy.toml` executes
//! project-relative scripts against the right project.
//!
//! Trust model: this deliberately diverges from the interactive menu's
//! `classify_shell_command` gauntlet. The menu refuses chaining metachars
//! and prompts before custom commands because the user picks a *label*
//! without ever seeing the command — a hostile jarvy.toml could hide a
//! payload behind a look-alike entry. An explicit `jarvy run <name>` is
//! consent to run whatever the project defines under that name, exactly
//! like `npm run <script>` or `make <target>`; the resolved command line
//! is printed (ANSI-stripped) before execution so nothing runs invisibly.
//! Chaining (`&&`, `|`) is therefore allowed here. NUL bytes are still
//! refused, extras keys go through the shared Trojan-Source sanitizer
//! (`config::sanitize_extras_keys`), and on Windows `%`-bearing trailing
//! args are refused because cmd.exe expands `%VAR%` even inside quotes.
//!
//! Unlike the menu, `run`/`test` have NO implicit `cargo run`/`cargo test`
//! fallback — the menu is a Rust-repo convenience; an unconfigured
//! `jarvy run test` in a Node project silently invoking cargo would be
//! wrong. Only what jarvy.toml declares is runnable.

use std::path::Path;

use crate::commands::shared::{quote_shell_arg, sanitize_for_display, short_cmd_hash, spawn_shell};
use crate::config::{CommandsConfig, read_commands_config};
use crate::error_codes;
use crate::observability::telemetry_gate;

/// Entry in the command listing.
struct CommandEntry<'a> {
    name: &'a str,
    command: &'a str,
    well_known: bool,
}

/// Run a named `[commands]` entry, or list them when `name` is None.
/// Returns the process exit code (child's code on execution).
pub fn run_run(file: &str, name: Option<&str>, extra_args: &[String], output_format: &str) -> i32 {
    let path = Path::new(file);
    let cfg = match read_commands_config(path) {
        Ok(c) => c,
        Err(e) => {
            if telemetry_gate::is_enabled() {
                tracing::warn!(event = "run.command.config_error", error_kind = e.kind());
            }
            eprintln!("Error: {}", e);
            return error_codes::CONFIG_ERROR;
        }
    };

    let Some(name) = name else {
        return list_commands(file, &cfg, output_format);
    };
    // Same field name (`label`) and sanitization as `interactive.command.*`
    // so the two domains join on one dimension. `start`/`complete`/`failed`
    // only fire for names that matched a sanitized key, but sanitize
    // uniformly anyway — `not_found`/`refused` carry arbitrary CLI input.
    let label = sanitize_for_display(name);

    let Some(cmd) = resolve(&cfg, name) else {
        eprintln!("No command named `{}` in [commands] of {}.", label, file);
        let entries = command_entries(&cfg);
        if entries.is_empty() {
            eprintln!("\nNo [commands] section is defined. Add one, e.g.:\n");
            eprintln!("[commands]\n{} = \"echo hello\"", label);
        } else {
            let names: Vec<&str> = entries.iter().map(|e| e.name).collect();
            eprintln!("Available: {}", names.join(", "));
        }
        if name == "setup" {
            eprintln!("Hint: `jarvy setup` runs the environment setup directly.");
        }
        if telemetry_gate::is_enabled() {
            tracing::warn!(event = "run.command.not_found", label = %label);
        }
        return error_codes::CONFIG_ERROR;
    };

    // Windows cmd.exe expands %VAR% even inside double quotes, so a
    // `-- %SOMEVAR%` arg would be substituted (CI env vars often hold
    // secrets) before the child sees it. No escape exists — refuse.
    #[cfg(windows)]
    if let Some(bad) = extra_args
        .iter()
        .find(|a| !crate::commands::shared::windows_arg_is_expansion_safe(a))
    {
        eprintln!(
            "Refusing to run `{}`: argument {:?} contains `%`, which cmd.exe \
             would expand as a variable reference; there is no way to pass it \
             verbatim through `cmd /C`.",
            label,
            sanitize_for_display(bad)
        );
        if telemetry_gate::is_enabled() {
            tracing::warn!(
                event = "run.command.refused",
                label = %label,
                reason = "percent_windows",
            );
        }
        return error_codes::CONFIG_ERROR;
    }

    // Run from the config file's directory so project-relative commands
    // (`cargo test`, `./scripts/build`) act on the project that defined
    // them, not the caller's cwd. A bare `jarvy.toml` has an empty parent
    // — treat that as "current dir" (None).
    let workdir = path.parent().filter(|p| !p.as_os_str().is_empty());

    // npm-style lifecycle hooks: `pre<name>`/`pre:<name>` runs before the
    // command and `post<name>`/`post:<name>` after it, when defined in
    // [commands]. Matching npm's semantics: extra `--` args go to the
    // MAIN command only, a failing pre aborts the run, and post only runs
    // after a successful main — the first non-zero exit anywhere is the
    // process exit code.
    if let Some((pre_name, pre_cmd)) = resolve_hook(&cfg, "pre", name) {
        let code = execute_one(&pre_name, pre_cmd, &[], workdir);
        if code != 0 {
            eprintln!(
                "`{}` failed (exit {}); not running `{}`",
                sanitize_for_display(&pre_name),
                code,
                label
            );
            return code;
        }
    }

    let code = execute_one(name, cmd, extra_args, workdir);
    if code != 0 {
        return code;
    }

    if let Some((post_name, post_cmd)) = resolve_hook(&cfg, "post", name) {
        let post_code = execute_one(&post_name, post_cmd, &[], workdir);
        if post_code != 0 {
            return post_code;
        }
    }
    0
}

/// Resolve a lifecycle hook for `name` in either spelling: `pre:build`
/// (colon, checked first) or `prebuild` (npm concatenation). When both
/// are defined the colon form wins — it's the more explicit spelling —
/// and a note is printed so the duplicate gets cleaned up rather than
/// silently ignored.
fn resolve_hook<'a>(cfg: &'a CommandsConfig, kind: &str, name: &str) -> Option<(String, &'a str)> {
    let colon = format!("{}:{}", kind, name);
    let concat = format!("{}{}", kind, name);
    match (resolve(cfg, &colon), resolve(cfg, &concat)) {
        (Some(cmd), Some(_)) => {
            eprintln!(
                "note: both `{}` and `{}` are defined; running `{}`",
                sanitize_for_display(&colon),
                sanitize_for_display(&concat),
                sanitize_for_display(&colon)
            );
            Some((colon, cmd))
        }
        (Some(cmd), None) => Some((colon, cmd)),
        (None, Some(cmd)) => Some((concat, cmd)),
        (None, None) => None,
    }
}

/// Execute one `[commands]` entry: NUL guard, telemetry start/complete/
/// failed under its own label, `> cmd` echo, spawn, exit-code mapping.
/// Shared by the main command and its pre/post lifecycle hooks.
fn execute_one(name: &str, cmd: &str, extra_args: &[String], workdir: Option<&Path>) -> i32 {
    let label = sanitize_for_display(name);
    let full_cmd = append_args(cmd, extra_args);
    if full_cmd.contains('\0') {
        eprintln!("Refusing to run `{}`: command contains NUL byte", label);
        if telemetry_gate::is_enabled() {
            tracing::warn!(
                event = "run.command.refused",
                label = %label,
                reason = "nul_byte",
            );
        }
        return error_codes::CONFIG_ERROR;
    }

    let start = std::time::Instant::now();
    let well_known = matches!(name, "run" | "test" | "setup");
    // Privacy contract shared with interactive.command.*: hash, never the
    // command text. Computed only when an event will actually carry it.
    let cmd_hash = if telemetry_gate::is_enabled() {
        Some(short_cmd_hash(&full_cmd))
    } else {
        None
    };
    if let Some(hash) = &cmd_hash {
        tracing::info!(
            event = "run.command.start",
            label = %label,
            cmd_hash = %hash,
            well_known,
            extra_args_count = extra_args.len(),
        );
    }

    println!("> {}", sanitize_for_display(&full_cmd));
    match spawn_shell(&full_cmd, workdir) {
        Ok(status) => {
            // Telemetry sentinel for signal-killed children is -1 (matches
            // interactive.command.complete) so it can't be confused with a
            // real `exit 1`; the PROCESS exit code stays 1.
            let telemetry_code = status.code().unwrap_or(-1);
            let process_code = status.code().unwrap_or(1);
            if let Some(hash) = &cmd_hash {
                tracing::info!(
                    event = "run.command.complete",
                    label = %label,
                    cmd_hash = %hash,
                    exit_code = telemetry_code,
                    duration_ms = start.elapsed().as_millis() as u64,
                );
            }
            if !status.success() {
                eprintln!("`{}` exited with code {}", label, process_code);
            }
            process_code
        }
        Err(e) => {
            if let Some(hash) = &cmd_hash {
                tracing::warn!(
                    event = "run.command.failed",
                    label = %label,
                    cmd_hash = %hash,
                    error = %e,
                );
            }
            eprintln!("Failed to execute `{}`: {}", label, e);
            1
        }
    }
}

/// Resolve a name against the three well-known slots, then extras.
fn resolve<'a>(cfg: &'a CommandsConfig, name: &str) -> Option<&'a str> {
    match name {
        "run" => cfg.run.as_deref(),
        "test" => cfg.test.as_deref(),
        "setup" => cfg.setup.as_deref(),
        _ => cfg.extras.get(name).map(String::as_str),
    }
}

/// All defined commands: well-known slots first (fixed order), extras sorted.
fn command_entries(cfg: &CommandsConfig) -> Vec<CommandEntry<'_>> {
    let mut entries = Vec::with_capacity(3 + cfg.extras.len());
    for (name, slot) in [
        ("run", &cfg.run),
        ("test", &cfg.test),
        ("setup", &cfg.setup),
    ] {
        if let Some(cmd) = slot.as_deref() {
            entries.push(CommandEntry {
                name,
                command: cmd,
                well_known: true,
            });
        }
    }
    let mut extra_keys: Vec<&str> = cfg.extras.keys().map(String::as_str).collect();
    extra_keys.sort_unstable();
    for name in extra_keys {
        entries.push(CommandEntry {
            name,
            command: &cfg.extras[name],
            well_known: false,
        });
    }
    entries
}

fn list_commands(file: &str, cfg: &CommandsConfig, output_format: &str) -> i32 {
    let entries = command_entries(cfg);
    if telemetry_gate::is_enabled() {
        tracing::info!(
            event = "run.command.list",
            format = %output_format,
            count = entries.len(),
        );
    }
    if output_format == "json" {
        let commands: Vec<serde_json::Value> = entries
            .iter()
            .map(|e| {
                serde_json::json!({
                    "name": e.name,
                    "command": e.command,
                    "well_known": e.well_known,
                })
            })
            .collect();
        let envelope = serde_json::json!({
            "file": file,
            "commands": commands,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&envelope).unwrap_or_else(|_| "{}".to_string())
        );
        return 0;
    }

    if entries.is_empty() {
        println!("No [commands] defined in {}.", file);
        println!(
            "\n\
             Add a [commands] table to {file} — each entry is a name and the\n\
             shell command it runs (like npm scripts):\n\
             \n\
             [commands]\n\
             build = \"cargo build\"\n\
             test  = \"cargo test\"\n\
             dev   = \"docker compose up -d && cargo watch -x run\"\n\
             \n\
             Then:\n\
             \n\
             jarvy run build          # run one by name\n\
             jarvy run test -- --lib  # pass extra args after `--`\n\
             jarvy run                # list what's defined\n\
             \n\
             Commands run from the directory containing {file}.\n\
             Tip: `jarvy shell-init --apply` sets up `jr` as a shorthand for `jarvy run`.",
        );
        return 0;
    }
    let width = entries.iter().map(|e| e.name.len()).max().unwrap_or(0);
    println!("Commands defined in {}:\n", file);
    for e in &entries {
        println!(
            "  {:width$}  {}",
            e.name,
            sanitize_for_display(e.command),
            width = width
        );
    }
    println!("\nRun one with: jarvy run <name>");
    0
}

/// Append extra CLI args (everything after `--`) to the command line,
/// each shell-quoted for the shell we hand the string to.
fn append_args(cmd: &str, args: &[String]) -> String {
    if args.is_empty() {
        return cmd.to_string();
    }
    let mut out = String::from(cmd);
    for a in args {
        out.push(' ');
        out.push_str(&quote_shell_arg(a));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn cfg_with(extras: &[(&str, &str)]) -> CommandsConfig {
        CommandsConfig {
            run: Some("cargo run".into()),
            test: None,
            setup: None,
            extras: extras
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect::<HashMap<_, _>>(),
        }
    }

    #[test]
    fn resolve_prefers_well_known_slots_over_extras() {
        // `serde(flatten)` means `run` can never actually land in extras,
        // but the resolution order should not depend on that invariant.
        let mut cfg = cfg_with(&[]);
        cfg.extras.insert("run".into(), "evil".into());
        assert_eq!(resolve(&cfg, "run"), Some("cargo run"));
    }

    #[test]
    fn resolve_finds_extras_and_misses_unknown() {
        let cfg = cfg_with(&[("fmt", "cargo fmt")]);
        assert_eq!(resolve(&cfg, "fmt"), Some("cargo fmt"));
        assert_eq!(resolve(&cfg, "test"), None, "unset slot is not runnable");
        assert_eq!(resolve(&cfg, "nope"), None);
    }

    #[test]
    fn entries_are_well_known_first_then_sorted_extras() {
        let cfg = cfg_with(&[("zeta", "z"), ("alpha", "a")]);
        let names: Vec<&str> = command_entries(&cfg).iter().map(|e| e.name).collect();
        assert_eq!(names, vec!["run", "alpha", "zeta"]);
    }

    #[test]
    fn append_args_quotes_each_arg() {
        let cmd = append_args("cargo test", &["--nocapture".into(), "a b".into()]);
        #[cfg(not(windows))]
        assert_eq!(cmd, "cargo test '--nocapture' 'a b'");
        #[cfg(windows)]
        assert_eq!(cmd, "cargo test \"--nocapture\" \"a b\"");
    }
}
