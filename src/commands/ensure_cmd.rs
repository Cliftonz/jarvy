//! Handler for `jarvy ensure`
//!
//! Lightweight check-and-install for shell startup.
//! Reads [shell_init] from ~/.jarvy/config.toml.

use crate::init::initialize;
use crate::logging;
use crate::observability::telemetry_gate;
use crate::shell_init::{self, EnsureStamp};

/// Env var set by the rc snippet so `ensure` can distinguish
/// rc-triggered runs from manual invocations in telemetry.
const RC_INVOCATION_ENV: &str = "JARVY_ENSURE_INVOCATION";

fn invocation_source() -> &'static str {
    match std::env::var(RC_INVOCATION_ENV).as_deref() {
        Ok("rc_snippet") => "rc_snippet",
        _ => "manual",
    }
}

pub fn run_ensure(force: bool, quiet: bool, foreground: bool) -> i32 {
    let source = invocation_source();
    let config = initialize();

    let shell_init = match config.shell_init {
        Some(ref si) if si.enabled => si.clone(),
        _ => {
            if !quiet {
                eprintln!("jarvy ensure: [shell_init] not enabled in ~/.jarvy/config.toml");
            }
            return 0;
        }
    };

    // Background mode: if not foreground and stamp is stale, spawn detached and exit
    if !foreground && shell_init.background {
        let config_hash = shell_init.config_hash();
        if !force {
            if let Some(stamp) = EnsureStamp::load() {
                if stamp.is_fresh(&config_hash, shell_init.check_interval) {
                    return 0;
                }
            }
        }

        // Spawn background process
        let exe = match std::env::current_exe() {
            Ok(e) => e,
            Err(err) => {
                emit_failed("current_exe", &err.to_string(), "background", source);
                if !quiet {
                    eprintln!("jarvy ensure: cannot find executable: {}", err);
                }
                return 0;
            }
        };

        let mut cmd = std::process::Command::new(exe);
        cmd.args(["ensure", "--foreground", "--quiet"]);
        if force {
            cmd.arg("--force");
        }

        cmd.stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());

        match cmd.spawn() {
            Ok(_) => {}
            Err(e) => {
                // Background spawn failure is invisible to the user
                // (rc snippet's `|| echo` sees exit 0 here) — the
                // file-log event is the only trail on-call has.
                emit_failed("spawn", &e.to_string(), "background", source);
                if !quiet {
                    eprintln!("jarvy ensure: failed to spawn background process: {}", e);
                }
            }
        }
        return 0;
    }

    // Foreground mode: do the actual work
    if let Err(e) = shell_init::run_ensure(&shell_init, force, quiet) {
        emit_failed("run_ensure", &e, "foreground", source);
        if !quiet {
            eprintln!("jarvy ensure: {}", e);
        }
        // Point the user at the full trace even in `--quiet` mode.
        // `jarvy ticket create` bundles logs through the redaction
        // pass; recommend that over the raw path so sensitive fields
        // (hostnames, workspace paths, stderr tails) don't ship
        // unredacted into support channels.
        eprintln!(
            "jarvy ensure: run `jarvy ticket create` to bundle logs with redaction;\n\
             raw log at {} if you need it",
            logging::current_log_file().display()
        );
        return 1;
    }

    emit_completed("foreground", source);
    0
}

fn emit_failed(error_kind: &str, error: &str, mode: &'static str, invocation_source: &'static str) {
    if telemetry_gate::is_enabled() {
        tracing::error!(
            event = "ensure.failed",
            error_kind = error_kind,
            error = error,
            mode = mode,
            invocation_source = invocation_source,
        );
    }
}

fn emit_completed(mode: &'static str, invocation_source: &'static str) {
    if telemetry_gate::is_enabled() {
        tracing::info!(
            event = "ensure.completed",
            mode = mode,
            invocation_source = invocation_source,
        );
    }
}
