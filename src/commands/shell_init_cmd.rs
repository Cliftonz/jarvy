//! Handler for `jarvy shell-init`
//!
//! Outputs a shell snippet to stdout for eval in RC files.

use crate::env::{detect_shell, parse_shell};
use crate::observability::telemetry_gate;
use crate::shell_init::generate_rc_snippet;

pub fn run_shell_init(shell: Option<&str>) -> i32 {
    let shell_type = match shell {
        Some(s) => match parse_shell(s) {
            Ok(st) => st,
            Err(e) => {
                eprintln!("Error: {}", e);
                return 1;
            }
        },
        None => detect_shell(),
    };

    // Low-cardinality shell label — makes per-shell (e.g. nushell)
    // shell-init adoption graphable alongside env.shell_rc_updated.
    if telemetry_gate::is_enabled() {
        tracing::info!(event = "shell_init.generated", shell = %shell_type);
    }
    print!("{}", generate_rc_snippet(shell_type));
    0
}
