//! Handler for `jarvy shell-init`
//!
//! Outputs a shell snippet to stdout for eval in RC files.

use crate::env::{detect_shell, parse_shell};
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

    print!("{}", generate_rc_snippet(shell_type));
    0
}
