//! Console chatter gate — the on/off switch for the plain `println!`
//! narration in the setup path ("Detecting Platform is: macos",
//! "Installing Required Tools", "Docker is already installed", …).
//!
//! Tracing lives in `crate::analytics`; this module governs only the
//! non-tracing stdout narration and the console tracing cap that pairs
//! with it. When chatter is off:
//!
//! 1. `chatter!` becomes a no-op.
//! 2. `main.rs` picks `LogLevel::WarnOnly` for the console tracing
//!    layer (INFO `event="..."` lines never reach stderr).
//!
//! File + OTLP sinks are untouched — the debug artifact stays whole.
//!
//! Precedence (highest wins):
//!
//! 1. env `JARVY_CHATTER=1|true|on|0|false|off`
//! 2. explicit `[logging] chatter = true|false` in `jarvy.toml`
//! 3. `-v` on the caller command → on
//! 4. `--quiet` / `-q` → off
//! 5. `stderr` is a TTY → on; not a TTY (piped, npm predev, CI) → off

use std::io::IsTerminal;
use std::sync::OnceLock;

static CHATTER_ON: OnceLock<bool> = OnceLock::new();

/// Inputs collected at startup — main.rs fills this from CLI flags and
/// the loaded project config, then calls `init` exactly once.
#[derive(Debug, Default, Clone, Copy)]
pub struct ChatterCfg {
    /// `[logging] chatter = true|false` from `jarvy.toml`, if present.
    pub explicit_toml: Option<bool>,
    /// User passed `-v` / `-vv` / `-vvv` on the current command.
    pub verbose: bool,
    /// User passed `--quiet` / `-q`.
    pub quiet: bool,
}

/// Compute whether chatter should be on. Pure — no globals read here so
/// the precedence is table-testable.
pub fn resolve(cfg: ChatterCfg, env_value: Option<&str>, stderr_is_tty: bool) -> bool {
    if let Some(raw) = env_value {
        match raw.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "on" | "yes" => return true,
            "0" | "false" | "off" | "no" => return false,
            _ => {}
        }
    }
    if cfg.quiet {
        return false;
    }
    if let Some(v) = cfg.explicit_toml {
        return v;
    }
    if cfg.verbose {
        return true;
    }
    stderr_is_tty
}

/// One-shot init. Second call is a no-op (matches `OnceLock` semantics).
pub fn init(cfg: ChatterCfg) {
    let env_value = std::env::var("JARVY_CHATTER").ok();
    let on = resolve(cfg, env_value.as_deref(), std::io::stderr().is_terminal());
    let _ = CHATTER_ON.set(on);
}

/// True when the chatter macros should emit. Defaults to `true` if
/// `init` was never called (test binaries, unit-test bootstraps) so
/// existing behaviour under `cargo test` doesn't change.
pub fn is_enabled() -> bool {
    *CHATTER_ON.get().unwrap_or(&true)
}

/// `println!` if chatter is enabled, else no-op. Use for the narration
/// lines that describe "here's what setup is doing right now."
///
/// Warnings and errors must stay on `eprintln!` — those aren't chatter.
/// Structured tracing events (`tracing::info!` with `event = "..."`)
/// remain governed by `analytics.rs`; do not funnel those through here.
#[macro_export]
macro_rules! chatter {
    ($($arg:tt)*) => {
        if $crate::console::is_enabled() {
            println!($($arg)*);
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_var_overrides_everything() {
        let cfg = ChatterCfg {
            explicit_toml: Some(false),
            verbose: false,
            quiet: true,
        };
        assert!(resolve(cfg, Some("1"), false));
        assert!(!resolve(cfg, Some("0"), true));
    }

    #[test]
    fn quiet_beats_toml_and_verbose() {
        let cfg = ChatterCfg {
            explicit_toml: Some(true),
            verbose: true,
            quiet: true,
        };
        assert!(!resolve(cfg, None, true));
    }

    #[test]
    fn explicit_toml_beats_tty_and_verbose() {
        let on_cfg = ChatterCfg {
            explicit_toml: Some(true),
            verbose: false,
            quiet: false,
        };
        assert!(resolve(on_cfg, None, false));

        let off_cfg = ChatterCfg {
            explicit_toml: Some(false),
            verbose: true,
            quiet: false,
        };
        assert!(!resolve(off_cfg, None, true));
    }

    #[test]
    fn verbose_beats_tty_default() {
        let cfg = ChatterCfg {
            explicit_toml: None,
            verbose: true,
            quiet: false,
        };
        assert!(resolve(cfg, None, false));
    }

    #[test]
    fn tty_default_when_no_other_signal() {
        let cfg = ChatterCfg::default();
        assert!(resolve(cfg, None, true));
        assert!(!resolve(cfg, None, false));
    }

    #[test]
    fn unknown_env_value_falls_through() {
        let cfg = ChatterCfg::default();
        assert!(resolve(cfg, Some("maybe"), true));
        assert!(!resolve(cfg, Some(""), false));
    }
}
