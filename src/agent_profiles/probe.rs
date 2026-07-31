//! Running-IDE probe used by the symlink-tier swap guard.
//!
//! A profile swap re-points an agent's live config dir at a different
//! snapshot. A desktop IDE (Cursor / Windsurf) that is *running* keeps
//! its config in memory and re-fetches lazily — the swap succeeds on
//! disk but the editor observes inconsistent state until it restarts.
//! `jarvy agents profile use --global` therefore refuses to re-point an
//! agent whose process is currently running unless `--force` is passed.
//!
//! Contract (both directions matter):
//! - **Fail-safe**: any probe failure (spawn error, timeout, `pgrep`
//!   missing on a minimal container image) returns `false`. The guard's
//!   job is to protect a *positively detected* running IDE, not to be
//!   the last word on whether one is running. False negatives are a
//!   worse UX than a false positive; false positives are worse than
//!   letting the swap through.
//! - **Bounded time**: every probe caps at [`PROBE_TIMEOUT`]; a slow
//!   `pgrep` on a machine under load cannot make `use --global` hang.
//! - **Test hooks**: `JARVY_TEST_MODE=1` short-circuits to `false` so
//!   the CLI test suite doesn't observe the developer's live editors.
//!   `JARVY_MOCK_RUNNING_IDES=<slug>[,<slug>...]` short-circuits to
//!   `true` for the named agents so `use_refuses_on_running_ide` can
//!   exercise the guard without spawning an editor. Both env hooks are
//!   consulted in every build (including `cargo test`) — the real
//!   `pgrep`/`tasklist` code path is exercised by in-tree PATH-shim
//!   tests below; there is no cfg(test) short-circuit that would elide
//!   the real primitives.
//!
//! Agent → binary name table lives in `Agent::desktop_binary_names()`;
//! agents without a desktop binary (Cline / Continue, in-editor
//! extensions) return an empty list and this function returns `false`
//! for them without spawning anything.

use std::time::Duration;

use crate::agents::Agent;

/// Wall-clock cap per probe subprocess. Kept tight because it gates
/// the interactive path — a laptop-load-spike `pgrep` that finishes in
/// two seconds still exceeds any reasonable UI latency budget for the
/// swap prompt.
const PROBE_TIMEOUT: Duration = Duration::from_secs(1);

/// `true` when the desktop IDE for `agent` is currently running.
///
/// Never blocks longer than [`PROBE_TIMEOUT`]. Returns `false` for
/// agents with no desktop binary, when the probing utility is missing,
/// on any spawn/wait error, and when `JARVY_TEST_MODE=1` is set (the
/// integration tests spin up real jarvy binaries — they must not
/// observe the developer's own editors and refuse to swap).
pub fn is_ide_running(agent: Agent) -> bool {
    // Test-mode short circuit: any real spawn here would let host
    // process state leak into the test outcome. Same posture as
    // `JARVY_TEST_MODE=1` gates elsewhere in the codebase.
    if std::env::var_os("JARVY_TEST_MODE").is_some_and(|v| v == "1") {
        return false;
    }
    // Documented test hook — commas separate agent slugs to treat as
    // running. Keeps `use_refuses_on_running_ide` from having to spawn
    // Cursor. Not for production use.
    if let Some(mock) = std::env::var_os("JARVY_MOCK_RUNNING_IDES") {
        let s = mock.to_string_lossy();
        return s.split(',').map(str::trim).any(|slug| slug == agent.slug());
    }
    let names = agent.desktop_binary_names();
    if names.is_empty() {
        return false;
    }
    for name in names {
        if probe_one(name) {
            return true;
        }
    }
    false
}

#[cfg(unix)]
fn probe_one(name: &str) -> bool {
    use std::io::Read;
    use std::process::{Command, Stdio};

    // `pgrep -x <name>` — exact-match only, no partial substring, so
    // "code" doesn't accidentally match "code-server" et al. `-x` is
    // supported by both procps-ng (Linux) and macOS pgrep.
    let mut child = match Command::new("pgrep")
        .arg("-x")
        .arg(name)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        // `pgrep` not on PATH (minimal container, BusyBox without
        // procps) → fail-safe false, do not block the swap.
        Err(_) => return false,
    };

    let deadline = std::time::Instant::now() + PROBE_TIMEOUT;
    // try_wait poll loop keeps the probe non-blocking and lets us
    // enforce PROBE_TIMEOUT without pulling in tokio.
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                // pgrep exits 0 when at least one match, 1 when none.
                // Only a 0 exit AND some stdout counts as "running";
                // an empty stdout with exit 0 is impossible in
                // practice but treated as false for safety.
                if !status.success() {
                    return false;
                }
                let mut buf = String::new();
                let read_ok = child
                    .stdout
                    .as_mut()
                    .map(|s| s.read_to_string(&mut buf).is_ok())
                    .unwrap_or(false);
                return read_ok && buf.split_whitespace().next().is_some();
            }
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return false;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
}

#[cfg(windows)]
fn probe_one(name: &str) -> bool {
    use std::io::Read;
    use std::process::{Command, Stdio};

    // `tasklist /FI "IMAGENAME eq <name>.exe" /NH /FO CSV`:
    // - /FI narrows to the exact image name (Cursor.exe, Windsurf.exe)
    // - /NH suppresses the header line so a "no processes" run has
    //   empty stdout that we can detect without string-parsing.
    // - /FO CSV keeps the output machine-friendly.
    // tasklist ALWAYS exits 0 (even with no matches), so match detection
    // is stdout-based, not exit-status-based.
    let mut child = match Command::new("tasklist")
        .arg("/FI")
        .arg(format!("IMAGENAME eq {name}.exe"))
        .arg("/NH")
        .arg("/FO")
        .arg("CSV")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return false,
    };

    let deadline = std::time::Instant::now() + PROBE_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                let mut buf = String::new();
                let read_ok = child
                    .stdout
                    .as_mut()
                    .map(|s| s.read_to_string(&mut buf).is_ok())
                    .unwrap_or(false);
                if !read_ok {
                    return false;
                }
                // tasklist with no match prints the localized string
                // "INFO: No tasks are running which match the specified criteria."
                // (or the language equivalent) to stdout. A real match
                // is a CSV row starting with `"Cursor.exe"` — that
                // opening quote is the discriminator.
                return buf.trim_start().starts_with('"');
            }
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return false;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
}

#[cfg(not(any(unix, windows)))]
fn probe_one(_name: &str) -> bool {
    // No supported probe for this platform — fail-safe.
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    /// Serial guard: the probe reads process-wide env vars and the
    /// tests mutate them. Keep them off the shared jarvy_home lane —
    /// they don't touch the profile store.
    #[test]
    #[serial(probe_env)]
    fn probe_returns_false_in_test_mode() {
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("JARVY_TEST_MODE", "1");
            std::env::remove_var("JARVY_MOCK_RUNNING_IDES");
        }
        // Even for agents with a real binary name, test mode wins —
        // an assertion here that depended on the developer's machine
        // state would flake in CI vs local runs.
        assert!(!is_ide_running(Agent::Cursor));
        assert!(!is_ide_running(Agent::Windsurf));
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("JARVY_TEST_MODE");
        }
    }

    #[test]
    #[serial(probe_env)]
    fn probe_returns_false_when_no_binary_names() {
        // In-editor extensions have no separate binary — the guard is
        // a no-op for them (no `--force` needed to swap continue/cline
        // even if they're technically "running" inside VS Code).
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("JARVY_TEST_MODE");
            std::env::remove_var("JARVY_MOCK_RUNNING_IDES");
        }
        assert!(!is_ide_running(Agent::Cline));
        assert!(!is_ide_running(Agent::Continue));
        // Env-tier agents also have no desktop binary.
        assert!(!is_ide_running(Agent::ClaudeCode));
        assert!(!is_ide_running(Agent::Codex));
    }

    #[test]
    #[serial(probe_env)]
    fn mock_env_var_marks_agents_as_running() {
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("JARVY_TEST_MODE");
            std::env::set_var("JARVY_MOCK_RUNNING_IDES", "cursor, windsurf");
        }
        assert!(is_ide_running(Agent::Cursor));
        assert!(is_ide_running(Agent::Windsurf));
        // Agents NOT in the mock list stay false even when the env is set.
        assert!(!is_ide_running(Agent::Cline));
        assert!(!is_ide_running(Agent::Continue));
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("JARVY_MOCK_RUNNING_IDES");
        }
    }

    #[test]
    #[serial(probe_env)]
    fn mock_env_var_empty_returns_false() {
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("JARVY_TEST_MODE");
            std::env::set_var("JARVY_MOCK_RUNNING_IDES", "");
        }
        // A set-but-empty env var still enters the mock branch and
        // reports "no matches" (splitting "" on `,` yields one empty
        // slug, which matches no real agent slug). The spawn path is
        // NOT reached — an empty explicit setting means "nothing is
        // mocked as running", not "please probe for real".
        assert!(!is_ide_running(Agent::Cursor));
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("JARVY_MOCK_RUNNING_IDES");
        }
    }

    // ── PATH-shim tests for the real probe primitives ──────────────
    //
    // Without these the real `pgrep`/`tasklist` code path is only
    // reachable from the developer's workstation with a live editor
    // running — a review item flagged that the previous cfg(test)
    // short-circuit meant no in-tree test exercised the real spawn +
    // stdout parse + timeout loop. These tests build a fake `pgrep`
    // shell script in a tempdir, prepend that dir to PATH, and clear
    // the env hooks so `is_ide_running` falls through to `probe_one`.

    /// RAII guard: prepends `dir` to PATH and clears the two env
    /// short-circuits. Restores original PATH on drop and clears any
    /// mock we set. Serialized on `probe_env` alongside the env-hook
    /// tests above.
    #[cfg(unix)]
    struct PathShimGuard {
        prev_path: Option<std::ffi::OsString>,
        _tmp: tempfile::TempDir,
    }

    #[cfg(unix)]
    impl PathShimGuard {
        fn new(pgrep_body: &str) -> Self {
            use std::os::unix::fs::PermissionsExt;
            let tmp = tempfile::tempdir().unwrap();
            let script = tmp.path().join("pgrep");
            std::fs::write(&script, pgrep_body).unwrap();
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

            let prev_path = std::env::var_os("PATH");
            let new_path = match &prev_path {
                Some(p) => {
                    let mut parts = vec![tmp.path().to_path_buf()];
                    parts.extend(std::env::split_paths(p));
                    std::env::join_paths(parts).unwrap()
                }
                None => tmp.path().as_os_str().to_os_string(),
            };
            #[allow(unsafe_code)]
            unsafe {
                std::env::set_var("PATH", &new_path);
                std::env::remove_var("JARVY_TEST_MODE");
                std::env::remove_var("JARVY_MOCK_RUNNING_IDES");
            }
            Self {
                prev_path,
                _tmp: tmp,
            }
        }
    }

    #[cfg(unix)]
    impl Drop for PathShimGuard {
        fn drop(&mut self) {
            #[allow(unsafe_code)]
            unsafe {
                match &self.prev_path {
                    Some(p) => std::env::set_var("PATH", p),
                    None => std::env::remove_var("PATH"),
                }
                std::env::remove_var("JARVY_MOCK_RUNNING_IDES");
            }
        }
    }

    /// Fake pgrep that ignores its args, prints a pid, and exits 0 —
    /// simulates a match. `is_ide_running(Cursor)` must return true.
    #[cfg(unix)]
    #[test]
    #[serial(probe_env)]
    fn probe_uses_pgrep_when_available() {
        let _shim = PathShimGuard::new("#!/bin/sh\necho 12345\nexit 0\n");
        assert!(
            is_ide_running(Agent::Cursor),
            "shimmed pgrep exit-0 + stdout must be treated as a running match"
        );
    }

    /// Fake pgrep that exits 1 with empty stdout — the "no match"
    /// case. `is_ide_running` must return false without treating an
    /// empty run as positive.
    #[cfg(unix)]
    #[test]
    #[serial(probe_env)]
    fn probe_returns_false_on_nonzero_pgrep_exit() {
        let _shim = PathShimGuard::new("#!/bin/sh\nexit 1\n");
        assert!(
            !is_ide_running(Agent::Cursor),
            "shimmed pgrep exit-1 must be treated as no match"
        );
    }

    /// Fake pgrep that hangs (sleep 10). `is_ide_running` must return
    /// false within `PROBE_TIMEOUT + slack` — the kill path fires
    /// rather than blocking the interactive swap.
    #[cfg(unix)]
    #[test]
    #[serial(probe_env)]
    fn probe_kills_hung_pgrep_after_timeout() {
        let _shim = PathShimGuard::new("#!/bin/sh\nsleep 10\n");
        let start = std::time::Instant::now();
        let result = is_ide_running(Agent::Cursor);
        let elapsed = start.elapsed();
        assert!(!result, "hung pgrep must fail-safe to false");
        // PROBE_TIMEOUT is 1s; allow generous slack for the 25ms poll
        // loop + subprocess kill/wait on a loaded CI box.
        assert!(
            elapsed < Duration::from_millis(2500),
            "probe must return within timeout budget; took {elapsed:?}"
        );
    }
}
