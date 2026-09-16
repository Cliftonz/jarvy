//! Refresh this process's in-memory `PATH` from the registry.
//!
//! Windows installers (winget, MSI, Chocolatey, nvm, ...) write new PATH
//! entries to `HKLM\...\Environment` / `HKCU\Environment` and broadcast
//! `WM_SETTINGCHANGE`, but neither reaches a process that's already
//! running — `jarvy setup` keeps the PATH it inherited at launch for its
//! entire lifetime. A tool installed mid-run is therefore invisible to
//! that tool's own `post_install` hook (Node's `corepack enable` is the
//! motivating case) and to any later `has()` check in the same process,
//! even though a brand new shell would see it immediately.
//!
//! Re-reading both registry keys and merging them into this process's
//! `PATH` before running a hook (or retrying a `has()` check) gives that
//! hook a freshly "logged in" PATH without requiring the user to open a
//! new terminal.

#[cfg(target_os = "windows")]
mod exec {
    use std::process::Command;
    use std::sync::Mutex;

    /// Serializes `refresh_current_process_path`. `jarvy setup` installs
    /// `custom_install` tools (Chocolatey's own bootstrap included) on a
    /// rayon thread pool, so more than one worker can call this
    /// concurrently in the same process; `std::env::set_var` is unsound
    /// under concurrent access from multiple threads, which is exactly
    /// what this mutex prevents.
    static PATH_REFRESH_LOCK: Mutex<()> = Mutex::new(());

    /// Read a `Path` value out of `reg query <hive> /v Path`. Returns
    /// `None` when the key/value is absent (a fresh HKCU with no
    /// user-scoped PATH is common and not an error) or the `reg`
    /// subprocess itself couldn't be spawned.
    fn read_registry_path(hive: &str) -> Option<String> {
        let output = Command::new("reg")
            .args(["query", hive, "/v", "Path"])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        parse_reg_query_path(&String::from_utf8_lossy(&output.stdout))
    }

    /// Parse `Path    REG_EXPAND_SZ    C:\...;C:\...` out of `reg query`
    /// stdout. Mirrors `windows::pathext::parse_reg_query_value`.
    fn parse_reg_query_path(stdout: &str) -> Option<String> {
        for line in stdout.lines() {
            let line = line.trim_start();
            if !line.starts_with("Path") {
                continue;
            }
            for token in ["REG_EXPAND_SZ", "REG_SZ"] {
                if let Some(idx) = line.find(token) {
                    let value = line[idx + token.len()..].trim();
                    if !value.is_empty() {
                        return Some(value.to_string());
                    }
                }
            }
        }
        None
    }

    /// Merge HKLM (machine) + HKCU (user) `Path` values — the same
    /// order Windows itself uses when building a new session's PATH —
    /// and install the result into this process's environment, ahead of
    /// whatever PATH this process already inherited (so a shell-profile
    /// PATH addition the registry doesn't know about still survives).
    ///
    /// Returns the registry-derived value it just merged in (`None` when
    /// neither hive had a `Path` value), so callers can tell whether a
    /// later read differs from an earlier one without re-parsing the
    /// process's own `PATH`, which keeps growing on every call regardless
    /// of whether the registry changed.
    pub fn refresh_current_process_path() -> Option<String> {
        // Best-effort: a poisoned lock (an earlier panic while holding
        // it) just means this refresh is skipped, same as any other
        // best-effort cache in this codebase (see `tools::common::has_cache`).
        let Ok(_guard) = PATH_REFRESH_LOCK.lock() else {
            return None;
        };
        let machine = read_registry_path(
            "HKLM\\SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment",
        );
        let user = read_registry_path("HKCU\\Environment");
        let combined = match (machine, user) {
            (Some(m), Some(u)) => format!("{m};{u}"),
            (Some(m), None) => m,
            (None, Some(u)) => u,
            (None, None) => return None,
        };
        let merged = match std::env::var("PATH") {
            Ok(existing) => format!("{combined};{existing}"),
            Err(_) => combined.clone(),
        };
        // SAFETY: `set_var` is unsound if it races another thread's
        // env read/write. `PATH_REFRESH_LOCK`, held for this whole
        // call, serializes every `refresh_current_process_path` caller
        // (the rayon-parallel custom-install phase can call this from
        // several worker threads at once) against each other. It does
        // not protect against unrelated `set_var` calls elsewhere in
        // the process — none exist on the concurrent `jarvy setup`
        // path this function runs on.
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("PATH", merged);
        }
        Some(combined)
    }

    /// Two consecutive reads returning the same registry value mean the
    /// write already landed, so there is nothing left to wait for.
    fn stabilized(reads: &[Option<String>]) -> bool {
        matches!(reads, [.., a, b] if a == b)
    }

    /// Call `refresh_current_process_path` up to `attempts` times, sleeping
    /// `delay` between attempts. Installers (winget in particular) can
    /// return "success" a moment before their own PATH write lands in the
    /// registry, so a single read immediately after install can still miss
    /// it; a short bounded retry gives that write time to land without
    /// blocking indefinitely. Stops as soon as two consecutive reads agree,
    /// since further attempts would just pay `delay` again to re-confirm a
    /// value that already stabilized; `attempts` stays the ceiling for the
    /// case where the value never settles.
    pub fn refresh_current_process_path_with_retry(attempts: u32, delay: std::time::Duration) {
        let mut reads = Vec::with_capacity(attempts as usize);
        for attempt in 0..attempts {
            reads.push(refresh_current_process_path());
            if stabilized(&reads) {
                break;
            }
            if attempt + 1 < attempts {
                std::thread::sleep(delay);
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn parses_reg_expand_sz_path() {
            let sample = "\r\nHKEY_LOCAL_MACHINE\\SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment\r\n    Path    REG_EXPAND_SZ    C:\\Windows;C:\\Windows\\System32\r\n";
            assert_eq!(
                parse_reg_query_path(sample),
                Some("C:\\Windows;C:\\Windows\\System32".to_string())
            );
        }

        #[test]
        fn parses_reg_sz_path() {
            let sample = "\r\nHKEY_CURRENT_USER\\Environment\r\n    Path    REG_SZ    C:\\Users\\me\\bin\r\n";
            assert_eq!(
                parse_reg_query_path(sample),
                Some("C:\\Users\\me\\bin".to_string())
            );
        }

        #[test]
        fn missing_value_returns_none() {
            let sample =
                "ERROR: The system was unable to find the specified registry key or value.\r\n";
            assert_eq!(parse_reg_query_path(sample), None);
        }

        fn simulate(reads: &[Option<String>], max_attempts: u32) -> u32 {
            let mut used = 0u32;
            for _ in reads.iter().take(max_attempts as usize) {
                used += 1;
                if stabilized(&reads[..used as usize]) {
                    break;
                }
            }
            used
        }

        #[test]
        fn three_identical_reads_stop_after_two_attempts() {
            let reads = vec![Some("A".to_string()); 3];
            assert_eq!(simulate(&reads, 3), 2);
        }

        #[test]
        fn three_different_reads_use_all_attempts() {
            let reads = vec![
                Some("A".to_string()),
                Some("B".to_string()),
                Some("C".to_string()),
            ];
            assert_eq!(simulate(&reads, 3), 3);
        }

        #[test]
        fn reads_differ_then_repeat_stops_at_the_repeat() {
            let reads = vec![
                Some("A".to_string()),
                Some("B".to_string()),
                Some("B".to_string()),
                Some("C".to_string()),
            ];
            assert_eq!(simulate(&reads, 4), 3);
        }

        #[test]
        fn single_read_never_stabilizes() {
            let reads = vec![Some("A".to_string())];
            assert_eq!(simulate(&reads, 3), 1);
        }

        #[test]
        fn two_none_reads_stabilize() {
            let reads = vec![None, None];
            assert_eq!(simulate(&reads, 3), 2);
        }
    }
}

#[cfg(target_os = "windows")]
pub use exec::{refresh_current_process_path, refresh_current_process_path_with_retry};

/// No-op off Windows — mid-process PATH staleness is a Windows-specific
/// problem (POSIX hooks already run in a fresh child shell that
/// re-sources `.bashrc`/`.zshrc`, which is how those hooks pick up PATH
/// changes made earlier in the same install).
#[cfg(not(target_os = "windows"))]
pub fn refresh_current_process_path() {}

/// No-op off Windows, same as `refresh_current_process_path`: no retry
/// loop or sleeping, since there is nothing to retry.
#[cfg(not(target_os = "windows"))]
pub fn refresh_current_process_path_with_retry(_attempts: u32, _delay: std::time::Duration) {}
