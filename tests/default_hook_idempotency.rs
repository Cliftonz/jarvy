//! Default-hook idempotency regression test.
//!
//! CLAUDE.md states default hooks are "Idempotent — Safe to run multiple times
//! (scripts check before modifying files)". A regression in a hook's `grep -q`
//! guard would silently grow `~/.bashrc` / `~/.zshrc` by another `eval $(...)`
//! line on every `jarvy setup` run. This test executes a curated subset of
//! shell-init hooks twice against an isolated `$HOME` and asserts byte-
//! identical post-run state. Curated subset is small enough to keep CI fast
//! while spanning the macro variants users hit most.
//!
//! Skipped on non-Unix and when `bash` is unavailable.

#![cfg(unix)]

use jarvy::tools::spec::list_tools_with_default_hooks;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

/// Tools whose default hooks must be byte-stable across multiple invocations.
/// Span the most-used shell-init shapes: starship (eval-init), zoxide,
/// direnv (hook), atuin (init), fzf (sourced files), and tools with
/// non-shell-init hook bodies (git, kubectl).
const TIER_1: &[&str] = &[
    "starship", "zoxide", "direnv", "atuin", "fzf", "git", "kubectl",
    // File-mutating hooks added with the hook-singles work — the exact
    // class this harness guards (rc-append / config-write idempotency).
    // `rust` is intentionally excluded: its hook shells out to
    // `rustup component add`, whose state lands in a rustup home and is
    // environment-dependent, so it isn't a pure `$HOME`-snapshot test.
    "kubectx", "nvim", "tmux",
];

fn bash_available() -> bool {
    Command::new("bash")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn run_hook(script: &str, home: &Path) {
    let out = Command::new("bash")
        .arg("-c")
        .arg(script)
        .env("HOME", home)
        // Wipe inherited XDG vars so hooks that consult them don't leak
        // outside the tempdir.
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("XDG_DATA_HOME")
        .env_remove("XDG_STATE_HOME")
        .output()
        .expect("invoke bash");
    assert!(
        out.status.success(),
        "hook exited non-zero:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}

/// Snapshot every regular file under `dir` keyed by its relative path.
fn snapshot(dir: &Path) -> HashMap<String, Vec<u8>> {
    let mut out = HashMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(read) = fs::read_dir(&d) else { continue };
        for entry in read.flatten() {
            let path = entry.path();
            let ty = entry.file_type().expect("file_type");
            if ty.is_dir() {
                stack.push(path);
            } else if ty.is_file() {
                let rel = path
                    .strip_prefix(dir)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                let bytes = fs::read(&path).unwrap_or_default();
                out.insert(rel, bytes);
            }
        }
    }
    out
}

fn assert_idempotent(tool: &str, script: &str) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let home = tmp.path();
    // Pre-create the rc files the scripts probe via `if [ -f ... ]` —
    // otherwise the body is a no-op and idempotency is trivial-pass.
    fs::write(home.join(".bashrc"), b"# pre-existing\n").unwrap();
    fs::write(home.join(".zshrc"), b"# pre-existing\n").unwrap();
    fs::write(home.join(".profile"), b"# pre-existing\n").unwrap();

    // Pre-seed the TPM checkout so the tmux hook takes its `.tmux.conf`
    // append branch (the idempotency-relevant path) instead of a network
    // `git clone`. The `[ -d tpm ]` guard makes the clone a no-op.
    fs::create_dir_all(home.join(".tmux/plugins/tpm")).unwrap();

    run_hook(script, home);
    let snap1 = snapshot(home);
    run_hook(script, home);
    let snap2 = snapshot(home);

    // Compare keys and values explicitly so the failure message names the
    // file that drifted, not just "maps differ".
    let keys1: std::collections::BTreeSet<_> = snap1.keys().collect();
    let keys2: std::collections::BTreeSet<_> = snap2.keys().collect();
    assert_eq!(
        keys1, keys2,
        "[{tool}] file set changed between hook invocations"
    );
    for key in keys1 {
        let a = &snap1[key];
        let b = &snap2[key];
        assert_eq!(
            a,
            b,
            "[{tool}] {key} drifted on second hook invocation\n--- run1 ---\n{}\n--- run2 ---\n{}",
            String::from_utf8_lossy(a),
            String::from_utf8_lossy(b),
        );
    }
}

#[test]
fn tier_one_default_hooks_are_idempotent() {
    if !bash_available() {
        eprintln!("skipping: bash not available on PATH");
        return;
    }

    // Tool names in the registry are stored as the literal macro identifier
    // (uppercase) — `STARSHIP`, not `starship` — so case-fold when matching
    // against TIER_1's lowercase canonical names.
    let registry: HashMap<String, &str> = list_tools_with_default_hooks()
        .into_iter()
        .map(|(name, hook)| (name.to_lowercase(), hook.script))
        .collect();

    let mut tested = 0_usize;
    for tool in TIER_1 {
        let Some(script) = registry.get(*tool) else {
            // Tool may not have a hook on this platform (Linux-only, etc.).
            // Don't fail — the iteration covers what's available here.
            continue;
        };
        assert_idempotent(tool, script);
        tested += 1;
    }
    assert!(
        tested > 0,
        "no tier-1 default hooks found on this platform — registry shape changed?"
    );
}

#[test]
fn every_registered_default_hook_has_idempotency_guard_keyword() {
    // Cheap structural smoke: idempotent shell-init hooks always have either
    // `grep -q` (check before write) or a true single-write operation. A
    // newly-added hook that lacks both is the regression we want to catch
    // before it ships.
    let mut offenders: Vec<String> = Vec::new();
    for (name, hook) in list_tools_with_default_hooks() {
        let script = hook.script;
        let has_guard =
            script.contains("grep -q") || script.contains("grep -F") || script.contains("[ -f ");
        // Allow scripts that don't append to rc files at all (no `>>` at all).
        let appends_to_rc = script.contains(">> \"$HOME") || script.contains(">>$HOME");
        if appends_to_rc && !has_guard {
            offenders.push(format!(
                "{name}: appends to rc files without an idempotency guard"
            ));
        }
    }
    assert!(
        offenders.is_empty(),
        "default_hook scripts missing idempotency guards:\n  {}",
        offenders.join("\n  ")
    );
}
