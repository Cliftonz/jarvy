//! Progress indicators for long-running operations (PRD-052)
//!
//! Thin wrapper over `indicatif` with consistent auto-disable rules so
//! spinners never end up in scraped log files, JSON output, or muted
//! `--quiet` invocations. Every long-running command goes through
//! `Progress::start` rather than constructing `ProgressBar` directly —
//! this keeps the muting decision in one place and prevents stray
//! spinners in CI logs (which kill jobs with 8MB+ of ANSI escapes when
//! a forgotten `tick()` runs in a loop).
//!
//! # Auto-disable
//!
//! Spinners are silently disabled (replaced by `ProgressBar::hidden()`)
//! when any of:
//!
//! - stdout is not a TTY (output redirected or piped)
//! - `JARVY_QUIET=1` or `--quiet`/`-q` on argv
//! - `--format json` / `--format=json` / `--log-format json` on argv
//! - CI / seamless sandbox detected by `crate::sandbox::is_seamless_auto()`
//! - `JARVY_NO_PROGRESS=1` (explicit kill switch for debugging)
//!
//! In CI mode (auto-disable triggered by sandbox detection), `Progress`
//! falls back to plain `println!` lines so the operator still sees
//! per-tool progress in a way that grep / log scrapers can parse.

use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
use std::sync::OnceLock;
use std::time::Duration;

/// Resolved once at first call — re-checking env vars / `isatty` on every
/// spinner construction would be wasteful and could produce inconsistent
/// behavior mid-run if a downstream subprocess flipped a stream.
static MUTED: OnceLock<bool> = OnceLock::new();

/// Decide whether progress output is muted. Cached after first call.
pub fn is_muted() -> bool {
    *MUTED.get_or_init(compute_muted)
}

fn compute_muted() -> bool {
    if std::env::var("JARVY_NO_PROGRESS").as_deref() == Ok("1") {
        return true;
    }
    if std::env::var("JARVY_QUIET").as_deref() == Ok("1") {
        return true;
    }
    if !is_stdout_tty() {
        return true;
    }
    // Walk argv for output-format flags. Matches both `--flag json` and
    // `--flag=json` shapes, plus `--quiet`/`-q`. Mirrors the muting walk
    // in `main.rs` for the sandbox banner — same flag set, same logic.
    let mut prev_was_format_flag = false;
    for a in std::env::args() {
        if a == "--quiet" || a == "-q" {
            return true;
        }
        if a == "--json" || a.starts_with("--format=json") || a.starts_with("--log-format=json") {
            return true;
        }
        if prev_was_format_flag && a == "json" {
            return true;
        }
        prev_was_format_flag = a == "--format" || a == "--log-format";
    }
    // Sandbox / CI auto-disable. Falls through to plain-line fallback in
    // `Progress::start`, so operators still see progress in log scrapers.
    crate::sandbox::is_seamless_auto()
}

fn is_stdout_tty() -> bool {
    // `std::io::IsTerminal` (stable since 1.70) covers Unix + Windows
    // without dragging in libc directly. `indicatif`'s own probe at draw
    // time is more thorough, but this is the muted-vs-fallback gate so
    // we want the answer up front.
    use std::io::IsTerminal;
    std::io::stdout().is_terminal()
}

/// Owns a `MultiProgress` plus its child spinners. Drop the whole
/// `Progress` to clear the display.
pub struct Progress {
    multi: MultiProgress,
    /// `true` when running in fallback (plain `println!`) mode — set
    /// when sandbox / CI is detected so callers know not to expect
    /// in-place updates.
    pub plain: bool,
}

impl Progress {
    /// Start a new progress group. When muted by TTY / format / quiet,
    /// returns a `Progress` whose `add` calls produce hidden bars (no
    /// stderr output at all). When muted by sandbox / CI, returns a
    /// `plain = true` group whose `Spinner::finish_with_message` falls
    /// through to a plain `println!`.
    pub fn start() -> Self {
        let muted = is_muted();
        let plain = muted && crate::sandbox::is_seamless_auto();
        let multi = if muted {
            MultiProgress::with_draw_target(ProgressDrawTarget::hidden())
        } else {
            MultiProgress::new()
        };
        Progress { multi, plain }
    }

    /// Add a child spinner labeled with `prefix` (e.g. `[3/12]`) and
    /// the current `message` (e.g. `Installing docker`). Returns a
    /// `Spinner` handle the caller updates on completion.
    pub fn add(&self, prefix: impl Into<String>, message: impl Into<String>) -> Spinner {
        let prefix = prefix.into();
        let message = message.into();
        if self.plain {
            // Fallback: print one line up front so log scrapers see the
            // start event. `finish_with_message` will print the
            // completion line.
            println!("{} {}", prefix, message);
            return Spinner {
                bar: None,
                plain_prefix: prefix,
            };
        }
        let bar = self.multi.add(ProgressBar::new_spinner());
        bar.set_style(default_spinner_style());
        bar.set_prefix(prefix.clone());
        bar.set_message(message);
        bar.enable_steady_tick(Duration::from_millis(120));
        Spinner {
            bar: Some(bar),
            plain_prefix: prefix,
        }
    }

    /// Print a line above the active spinners without disturbing them.
    /// Use this for one-shot status messages (`Skipped foo: already
    /// installed`) that should appear in scrollback while the spinners
    /// continue to animate below.
    pub fn println(&self, msg: impl AsRef<str>) {
        if self.plain || is_muted() {
            println!("{}", msg.as_ref());
        } else {
            // `MultiProgress::println` interleaves cleanly above the
            // bars; falls back to `println!` if no bars are active.
            let _ = self.multi.println(msg.as_ref());
        }
    }
}

/// Single in-flight spinner. Dropping without calling
/// `finish_with_message` / `finish_and_clear` leaves the spinner
/// frozen — the `Drop` impl cleans it up but no completion line is
/// printed.
pub struct Spinner {
    bar: Option<ProgressBar>,
    plain_prefix: String,
}

impl Spinner {
    /// Update the in-flight message without finishing.
    pub fn set_message(&self, message: impl Into<String>) {
        if let Some(bar) = &self.bar {
            bar.set_message(message.into());
        }
    }

    /// Mark complete with a success line. Format: `<prefix> ✓ <message>`.
    pub fn finish_ok(self, message: impl Into<String>) {
        let message = message.into();
        match self.bar {
            Some(bar) => {
                bar.set_style(done_style("✓"));
                bar.finish_with_message(message);
            }
            None => {
                println!("{} ✓ {}", self.plain_prefix, message);
            }
        }
    }

    /// Mark complete with a skip / already-satisfied line.
    pub fn finish_skipped(self, message: impl Into<String>) {
        let message = message.into();
        match self.bar {
            Some(bar) => {
                bar.set_style(done_style("·"));
                bar.finish_with_message(message);
            }
            None => {
                println!("{} · {}", self.plain_prefix, message);
            }
        }
    }

    /// Mark complete with a failure line.
    pub fn finish_failed(self, message: impl Into<String>) {
        let message = message.into();
        match self.bar {
            Some(bar) => {
                bar.set_style(done_style("✗"));
                bar.finish_with_message(message);
            }
            None => {
                println!("{} ✗ {}", self.plain_prefix, message);
            }
        }
    }
}

fn default_spinner_style() -> ProgressStyle {
    ProgressStyle::with_template("{prefix:.bold.dim} {spinner:.cyan} {msg}")
        .expect("static template")
        .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
}

fn done_style(mark: &'static str) -> ProgressStyle {
    let template = format!("{{prefix:.bold.dim}} {mark} {{msg}}");
    ProgressStyle::with_template(&template).expect("done template")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_construction_does_not_panic_when_muted() {
        // Force muted path by setting the kill switch BEFORE OnceLock
        // is initialized. If another test in the same process already
        // computed MUTED, this test's env var is a no-op — but the
        // construction path must still be panic-free.
        // SAFETY: env var set before any threads are spawned in this test.
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("JARVY_NO_PROGRESS", "1");
        }
        let p = Progress::start();
        let s = p.add("[1/1]", "test op");
        s.finish_ok("done");
    }
}
