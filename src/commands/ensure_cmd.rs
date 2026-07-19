//! Handler for `jarvy ensure`
//!
//! Lightweight check-and-install for shell startup.
//! Reads [shell_init] from ~/.jarvy/config.toml.

use crate::init::initialize;
use crate::shell_init::{self, EnsureStamp};

pub fn run_ensure(force: bool, quiet: bool, foreground: bool) -> i32 {
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
                if !quiet {
                    eprintln!("jarvy ensure: failed to spawn background process: {}", e);
                }
            }
        }
        return 0;
    }

    // Foreground mode: do the actual work
    if let Err(e) = shell_init::run_ensure(&shell_init, force, quiet) {
        if !quiet {
            eprintln!("jarvy ensure: {}", e);
        }
        // Point the user at the full trace even in `--quiet` mode.
        // With the WarnOnly console default, INFO/DEBUG only reach
        // the file appender — users would otherwise have to know
        // that path exists.
        eprintln!("jarvy ensure: see ~/.jarvy/logs/jarvy.log for details");
        return 1;
    }

    0
}
