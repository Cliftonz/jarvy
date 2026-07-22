//! Handler for `jarvy shell-init`
//!
//! Outputs a shell snippet to stdout for eval in RC files, or — with
//! `--apply` — writes the loader line into the shell's rc file directly
//! so `jr` works without any manual rc editing. Nothing else wires the
//! snippet up automatically (`cargo install` has no post-install hook,
//! and install.sh deliberately doesn't edit rc files), so `--apply` is
//! the one-command path for every install method.

use crate::env::{ShellType, detect_shell, get_rc_path, parse_shell};
use crate::observability::telemetry_gate;
use crate::shell_init::generate_rc_snippet;

pub fn run_shell_init(shell: Option<&str>, apply: bool) -> i32 {
    // Dedicated INFO witness that tests can assert on to prove the
    // console layer's INFO cap. Assertion-time coupling to another
    // event's fire pattern (e.g. `plugins.registered`) is fragile —
    // that event may be renamed, gated, or moved. This one exists
    // to be asserted on. NOT gated: its whole purpose is being a
    // stable console-visible signal, mirroring `plugins.registered`
    // which is also ungated for the same lifecycle-signal reason.
    tracing::info!(event = "shell_init.started", "shell-init invoked");

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

    if apply {
        return apply_rc_line(shell_type);
    }

    // Low-cardinality shell label — makes per-shell (e.g. nushell)
    // shell-init adoption graphable alongside env.shell_rc_updated.
    if telemetry_gate::is_enabled() {
        tracing::info!(event = "shell_init.generated", shell = %shell_type);
    }
    print!("{}", generate_rc_snippet(shell_type));
    0
}

/// Write the shell-appropriate loader line into the rc file, idempotently.
///
/// Most shells get a one-liner that re-evaluates `jarvy shell-init` on
/// every shell start (so snippet improvements arrive without re-applying).
/// Nushell has no `eval`, so the snippet is materialized to
/// `~/.jarvy/init.nu` and `source`d — re-run `--apply` after a jarvy
/// update to refresh that file.
fn apply_rc_line(shell: ShellType) -> i32 {
    let rc = match get_rc_path(shell) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error: cannot resolve the rc file for {}: {}", shell, e);
            return 1;
        }
    };

    let line = match shell {
        ShellType::Fish => "jarvy shell-init --shell fish | source".to_string(),
        ShellType::PowerShell => {
            "Invoke-Expression (& jarvy shell-init --shell powershell | Out-String)".to_string()
        }
        ShellType::Nushell => {
            // `source` needs a literal path known at parse time, so the
            // snippet lives in a real file.
            let init_path = match crate::paths::jarvy_home() {
                Ok(h) => h.join("init.nu"),
                Err(e) => {
                    eprintln!("Error: cannot resolve ~/.jarvy: {}", e);
                    return 1;
                }
            };
            if let Some(parent) = init_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Err(e) = std::fs::write(&init_path, generate_rc_snippet(shell)) {
                eprintln!("Error: cannot write {}: {}", init_path.display(), e);
                return 1;
            }
            format!("source {}", init_path.display())
        }
        // Bash, Zsh, Sh
        _ => format!("eval \"$(jarvy shell-init --shell {})\"", shell),
    };

    let existing = std::fs::read_to_string(&rc).unwrap_or_default();
    if existing.contains("jarvy shell-init") {
        println!(
            "Already set up — {} references `jarvy shell-init`.\n\
             Open a new shell (or `source {}`) if `jr` isn't available yet.",
            rc.display(),
            rc.display()
        );
        return 0;
    }

    if let Some(parent) = rc.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        eprintln!("Error: cannot create {}: {}", parent.display(), e);
        return 1;
    }
    let block = format!(
        "\n# Added by `jarvy shell-init --apply` — defines `jr` (jarvy run) and runs `jarvy ensure`\n{}\n",
        line
    );
    let mut content = existing;
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    content.push_str(&block);
    if let Err(e) = std::fs::write(&rc, content) {
        eprintln!("Error: cannot write {}: {}", rc.display(), e);
        return 1;
    }

    if telemetry_gate::is_enabled() {
        tracing::info!(event = "shell_init.applied", shell = %shell);
    }
    println!(
        "Added to {}:\n\n  {}\n\nOpen a new shell (or `source {}`) and `jr` is ready.",
        rc.display(),
        line,
        rc.display()
    );
    0
}
