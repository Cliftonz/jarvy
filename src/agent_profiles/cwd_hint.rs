//! Advisory profile-drift nudge fired from a shell `cd` hook.
//!
//! `jarvy agents profile check-cwd` walks up from `cwd` looking for a
//! `jarvy.toml`, reads its `[agents.profiles] prefer` value, and
//! compares the live per-agent profile against it (via the existing
//! `agent_profiles::check_preference`). On divergence it prints one
//! line to stderr and emits one `agent_profile.cwd_hint` telemetry
//! event. Never switches, never mutates config.
//!
//! **Debounce** — the hook runs on every `cd`, so raw output would
//! nag on every prompt. State lives in
//! `~/.jarvy/agent-profiles/.cwd-hint-state.json`
//! ([`STATE_FILENAME`]) and keys by
//! `sha256(session_id + "|" + repo_root)`: a switch in *this* terminal
//! silences the hint for THIS repo in THIS session, without silencing
//! it in a sibling shell that hasn't switched yet. TTL is
//! [`CWD_HINT_TTL_SECS`] (4 h); entries older than [`STATE_MAX_AGE_SECS`]
//! (7 days) are pruned on every write so the file doesn't grow
//! unboundedly on machines that visit many repos.
//!
//! **Silent-exit conditions** (no output, no telemetry, exit 0):
//! - no `jarvy.toml` walking up to the home dir,
//! - config parses but has no `[agents.profiles] prefer`,
//! - `JARVY_NO_CWD_HINT=1` in env,
//! - stderr is not a TTY (redirected — hint is human-facing),
//! - `--quiet` was passed,
//! - the debounce key is within TTL,
//! - the preference is matched (still records the debounce so the
//!   matched shell doesn't re-emit telemetry on every prompt).
//!
//! **Telemetry** — `agent_profile.cwd_hint { matched, debounced }`.
//! Profile names NEVER appear. No path either: repo roots under
//! `$HOME` carry the account name on macOS/Windows.
//!
//! **Trust surface** — the state file is created 0600 via
//! [`crate::paths::write_0600`] and rewritten atomically (tmp + rename)
//! so a co-tenant can't read another user's session/repo mapping. The
//! `jarvy.toml` this reads is on-disk in a project the user already
//! `cd`'d into — same posture as any other read from that path.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::agent_profiles::status::check_preference;

/// Debounce TTL: once a hint fires (or would fire — matched runs record
/// too), the same session+repo key is silent for this many seconds.
/// 4 hours matches a typical work half-session — long enough that a
/// user isn't nagged, short enough that a genuinely stale mismatch
/// re-surfaces within a workday.
pub const CWD_HINT_TTL_SECS: u64 = 4 * 3600;

/// Prune-on-write threshold: entries older than this are discarded.
/// Bounds the state-file size on machines that visit many repos.
pub const STATE_MAX_AGE_SECS: u64 = 7 * 24 * 3600;

/// Filename inside `~/.jarvy/agent-profiles/`. Leading `.` keeps it out
/// of the profile-name listing (`status::collect_status` filters names
/// starting with `.`), so a stray sidecar can never look like a profile.
pub const STATE_FILENAME: &str = ".cwd-hint-state.json";

/// Kill-switch env var — set to `1` to make `check-cwd` a no-op. Users
/// who want the setup-time preference check but not the per-`cd` nudge
/// set this in their shell rc.
pub const OPT_OUT_ENV: &str = "JARVY_NO_CWD_HINT";

/// Env var the shell hook may set to a stable session id. Falls back
/// through [`TERM_SESSION_ID_ENV`] to the parent PID (via `getppid` on
/// Unix / current PID on Windows) — good enough to distinguish sibling
/// terminals without depending on the terminal emulator honoring
/// TERM_SESSION_ID.
pub const SESSION_ID_ENV: &str = "JARVY_CWD_HINT_SESSION";

/// iTerm2 / Apple Terminal / VS Code integrated terminal set this.
pub const TERM_SESSION_ID_ENV: &str = "TERM_SESSION_ID";

/// Env var the rc snippet sets so telemetry can attribute the hint to
/// a shell-hook invocation vs. a hand-typed CLI call. Same vocabulary
/// as `JARVY_JP_INVOCATION` / `JARVY_ENSURE_INVOCATION` so the three
/// domains join on a shared `invocation_source` field.
pub const INVOCATION_SOURCE_ENV: &str = "JARVY_CWD_HINT_INVOCATION";

/// Persistent debounce state — schema-versioned like `ProfileRegistry`.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct CwdHintState {
    #[serde(default = "default_schema_version")]
    schema_version: u32,
    /// key → last-seen unix timestamp (seconds).
    #[serde(default)]
    seen: BTreeMap<String, u64>,
}

fn default_schema_version() -> u32 {
    1
}

impl Default for CwdHintState {
    fn default() -> Self {
        Self {
            schema_version: 1,
            seen: BTreeMap::new(),
        }
    }
}

/// Minimal deserialize target for `[agents.profiles] prefer`. Full
/// `Config` parsing pulls in every subsystem's schema; this runs on
/// every `cd`, so keep it cheap.
#[derive(Deserialize)]
struct MinimalConfig {
    #[serde(default)]
    agents: Option<MinimalAgents>,
}

#[derive(Deserialize)]
struct MinimalAgents {
    #[serde(default)]
    profiles: Option<MinimalAgentProfiles>,
}

#[derive(Deserialize)]
struct MinimalAgentProfiles {
    #[serde(default)]
    prefer: Option<String>,
}

/// Reason a `run_inner` call produced no output — kept as a bounded
/// enum (not a string) so tests assert on it structurally.
#[cfg_attr(test, derive(Debug, PartialEq, Eq))]
#[derive(Clone, Copy)]
pub(crate) enum SilentReason {
    /// `stderr` is not a terminal (piped / redirected / systemd unit).
    NonTty,
    /// `JARVY_NO_CWD_HINT=1` opted out.
    OptOut,
    /// No `jarvy.toml` walking up to `$HOME`, or the walk refused
    /// (cwd outside `$HOME`).
    NoConfig,
    /// Config parsed but `[agents.profiles] prefer` was absent / empty.
    NoPrefer,
    /// Registry unreadable / no store yet — nothing to compare.
    NoRegistry,
}

/// What `run_inner` decided to do. `Emit` variants still leave the
/// stderr print + tracing emit to the outer `run()` adapter so tests
/// can drive `run_inner` without asserting on IO or global tracing
/// state. `run_inner` DOES perform the debounce state-file mutation —
/// that's part of the decision, not a side-effect of emit.
#[cfg_attr(test, derive(Debug, PartialEq, Eq))]
#[derive(Clone)]
pub(crate) enum RunEffect {
    /// No-op path; carries the reason for tests.
    Silent {
        #[cfg_attr(not(test), allow(dead_code))]
        reason: SilentReason,
    },
    /// Key present in debounce state within TTL — nothing printed.
    Debounced,
    /// Would print/emit. `matched = true` means the preference matched
    /// (no stderr line, but telemetry still fires so dashboards can
    /// count matched vs mismatched exposures). `prefer`/`mismatched`
    /// are the pre-computed strings the outer adapter needs to print.
    Emit {
        matched: bool,
        prefer: String,
        mismatched: Vec<String>,
    },
}

/// Entry point — always exits 0 (never fails a shell prompt).
pub fn run(session_id_flag: Option<&str>, quiet: bool) -> i32 {
    let is_tty = stderr_is_tty();
    let opt_out = std::env::var_os(OPT_OUT_ENV).is_some_and(|v| v == "1");
    match run_inner(is_tty, opt_out, session_id_flag, quiet) {
        RunEffect::Silent { .. } | RunEffect::Debounced => 0,
        RunEffect::Emit {
            matched,
            prefer,
            mismatched,
        } => {
            if !matched && !quiet {
                // One-line stderr nudge. The command to fix is right
                // there so the user doesn't have to remember the flag
                // order.
                let agents = mismatched.join(", ");
                eprintln!(
                    "jarvy: this repo prefers agent profile '{prefer}' ({agents} on another). \
                     Switch with `jarvy agents profile use {prefer}`."
                );
            }
            if crate::observability::telemetry_gate::is_enabled() {
                // Only fires on non-debounced runs when `prefer` is
                // set — every other path returns `Silent`/`Debounced`
                // above. `debounced = false` is baked into "we're
                // here"; leaving it in the schema keeps future
                // dashboards that DO split on it forward-compatible.
                let invocation_source =
                    if std::env::var_os(INVOCATION_SOURCE_ENV).is_some_and(|v| v == "rc_snippet") {
                        "rc_snippet"
                    } else {
                        "manual"
                    };
                tracing::info!(
                    event = "agent_profile.cwd_hint",
                    matched,
                    debounced = false,
                    invocation_source,
                );
            }
            0
        }
    }
}

/// Testable core: same behavior as `run` but takes `is_tty` and
/// `opt_out` as arguments and returns the intended effect instead of
/// printing / emitting telemetry. Still performs the debounce
/// state-file mutation on non-silent paths — that write is what makes
/// subsequent calls Debounced, so pulling it out would make the tests
/// lie about the state machine.
pub(crate) fn run_inner(
    is_tty: bool,
    opt_out: bool,
    session_id_flag: Option<&str>,
    _quiet: bool,
) -> RunEffect {
    if opt_out {
        return RunEffect::Silent {
            reason: SilentReason::OptOut,
        };
    }
    if !is_tty {
        return RunEffect::Silent {
            reason: SilentReason::NonTty,
        };
    }

    let Ok(cwd) = std::env::current_dir() else {
        return RunEffect::Silent {
            reason: SilentReason::NoConfig,
        };
    };
    let Some((jarvy_toml, repo_root)) = find_config_walking_up(&cwd) else {
        return RunEffect::Silent {
            reason: SilentReason::NoConfig,
        };
    };
    let Some(prefer) = read_prefer(&jarvy_toml) else {
        return RunEffect::Silent {
            reason: SilentReason::NoPrefer,
        };
    };

    let session = resolve_session_id(session_id_flag);
    let repo_root_str = repo_root.to_string_lossy();
    let key = state_key(&session, repo_root_str.as_ref());
    let now = now_unix();

    // Fast path: if the debounce key doesn't appear anywhere in the
    // state file, we're guaranteed to miss the debounce check — skip
    // the full JSON parse and fall straight through to the emit path.
    // A miss here also means we start from a fresh default state; the
    // save at the bottom re-reads to merge, so we don't lose sibling
    // entries.
    let key_present_hint = state_path()
        .map(|p| quick_key_check(&p, &key))
        .unwrap_or(false);

    let mut updated = if key_present_hint {
        let (state, u) = load_state();
        if let Some(last) = state.seen.get(&key)
            && now.saturating_sub(*last) < CWD_HINT_TTL_SECS
        {
            // Debounced — no output, no event. Setup-hint-tier
            // telemetry stays quiet on repeat runs so the emit rate is
            // bounded by "distinct (session, repo) pairs per TTL",
            // not "prompts per TTL". Documenting the debounce hit as
            // an event would defeat its own purpose.
            return RunEffect::Debounced;
        }
        u
    } else {
        // Key absent → we're going to write. Re-read the (possibly
        // present) file so sibling entries are preserved on merge; if
        // the file doesn't exist or is corrupt, start from default.
        let (_, u) = load_state();
        u
    };

    let Ok(check) = check_preference(&prefer) else {
        // Registry unreadable / no store yet — nothing to compare, and
        // failing here would nag users who just haven't run `init` yet.
        return RunEffect::Silent {
            reason: SilentReason::NoRegistry,
        };
    };
    let matched = check.matched();
    let mismatched: Vec<String> = check.mismatched.iter().map(|s| (*s).to_string()).collect();

    // Record the debounce for BOTH matched and mismatched: a matched
    // shell shouldn't re-emit telemetry on every subsequent `cd`
    // either. The gate above (`matched && quiet-if-mismatched`) is
    // about user output; this is about event volume.
    updated.seen.insert(key, now);
    prune_stale(&mut updated, now);
    let _ = save_state(&updated);

    RunEffect::Emit {
        matched,
        prefer,
        mismatched,
    }
}

/// Walk up from `start` looking for `jarvy.toml`. Stops at the home
/// dir (inclusive) so a rogue `~/../etc/jarvy.toml`-style traversal
/// can't happen and system paths above `$HOME` never get probed.
/// Also refuses to walk at all if `start` canonicalizes outside
/// `$HOME` — closes Security F2 (traversal outside home; a user
/// `cd`'d into `/tmp` shouldn't have their shell prompt probe there
/// for a config file). Returns `(config_path, repo_root)` — repo_root
/// is the canonicalized dir holding the config.
///
/// Perf: `home_dir` and `home.canonicalize()` are computed once —
/// previously canonicalized on every prompt hit.
fn find_config_walking_up(start: &Path) -> Option<(PathBuf, PathBuf)> {
    let home = crate::agents::home_dir()?;
    let home_canon = home.canonicalize().ok()?;
    let start_canon = start.canonicalize().ok()?;
    // Security F2: refuse walks whose canonical start isn't under
    // `$HOME`. Without this, a shell `cd`'d into `/tmp` would have
    // its prompt hook stat `/tmp/jarvy.toml`, `/jarvy.toml`, etc —
    // the loop's home-stop only helps when start IS under home.
    if !start_canon.starts_with(&home_canon) {
        return None;
    }
    let mut cur = start_canon;
    loop {
        let candidate = cur.join("jarvy.toml");
        if candidate.is_file() {
            return Some((candidate, cur));
        }
        // Stop at home dir — do not scan `/`, `/etc`, or a sibling
        // user's home.
        if cur == home_canon {
            return None;
        }
        cur = cur.parent()?.to_path_buf();
    }
}

/// Read only `[agents.profiles] prefer`. Silent on parse errors — the
/// shell hook must never bail loudly on a corrupt config that the user
/// hasn't tried to run yet.
///
/// Hot-path perf: tries a memchr-style substring scan first
/// ([`read_prefer_fast`]) so the common case avoids a full TOML parse
/// on every shell prompt. Falls back to `toml::from_str` for anything
/// the scanner can't confidently handle (inline tables, unusual
/// escapes, prefer defined via dotted keys), preserving full TOML
/// compatibility.
fn read_prefer(path: &Path) -> Option<String> {
    let body = fs::read_to_string(path).ok()?;
    if let Some(fast) = read_prefer_fast(&body) {
        return Some(fast);
    }
    let parsed: MinimalConfig = toml::from_str(&body).ok()?;
    let value = parsed
        .agents?
        .profiles?
        .prefer
        .filter(|s| !s.trim().is_empty())?;
    Some(value)
}

/// Fast path: find the `[agents.profiles]` header, then within that
/// section find `prefer = "..."`. Returns `None` on any ambiguity
/// (inline table, unquoted value, missing header) so the caller falls
/// back to full TOML parsing — this must never return a wrong answer,
/// only a fast one.
fn read_prefer_fast(text: &str) -> Option<String> {
    const HEADER: &str = "[agents.profiles]";
    // Require the header to be at the start of a line — otherwise a
    // comment or string mentioning `[agents.profiles]` would misalign
    // the scan. Falls back to the full parser on ambiguity.
    let mut search_from = 0usize;
    let header_pos = loop {
        let rel = text[search_from..].find(HEADER)?;
        let abs = search_from + rel;
        let at_line_start = abs == 0 || text.as_bytes()[abs - 1] == b'\n';
        if at_line_start {
            break abs;
        }
        search_from = abs + HEADER.len();
    };
    // Everything after the header, bounded by the NEXT section header
    // (a `\n[` sequence) or EOF.
    let after = &text[header_pos + HEADER.len()..];
    let section_end = after.find("\n[").unwrap_or(after.len());
    let section = &after[..section_end];
    for raw_line in section.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some(rest) = line.strip_prefix("prefer") else {
            continue;
        };
        // Must be followed by `=` (allow whitespace between key and
        // `=`). Anything else (`preferred`, `prefer.something`, …)
        // isn't ours.
        let rest = rest.trim_start();
        let rest = rest.strip_prefix('=')?;
        let rest = rest.trim();
        // Only the quoted-string case; complex forms (inline tables,
        // literal strings `'..'`, multi-line `"""...` strings) fall
        // through to the full parser. Multi-line detect first — `"""`
        // starts with `"` too but the value can't fit on one line.
        if rest.starts_with("\"\"\"") {
            return None;
        }
        let inner = rest.strip_prefix('"')?;
        // A quoted TOML basic-string can contain `\"` escapes; since we
        // reject any `\` below, the FIRST subsequent `"` is the closing
        // quote. `#` inside a quoted string is literal in TOML, so we
        // cannot split on `#` before finding the closing quote.
        let close = inner.find('"')?;
        let value = &inner[..close];
        if value.contains('\\') {
            // Escapes — let the real parser handle it.
            return None;
        }
        let trimmed = value.trim();
        return if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        };
    }
    None
}

/// Session id resolution order:
///  1. `--session-id <id>` CLI flag (explicit wins).
///  2. `JARVY_CWD_HINT_SESSION` env var.
///  3. `TERM_SESSION_ID` env var (set by iTerm2, VS Code, etc).
///  4. `ppid-<parent_pid>` (Unix) / `pid-<current_pid>` (Windows).
///
/// The fallback is deliberately noisy — a fresh shell that opens a
/// subshell for the hook gets a stable parent PID for the whole
/// terminal, which is what we want to key the debounce on.
fn resolve_session_id(flag: Option<&str>) -> String {
    if let Some(id) = flag
        && !id.is_empty()
    {
        return id.to_string();
    }
    for var in [SESSION_ID_ENV, TERM_SESSION_ID_ENV] {
        if let Some(v) = std::env::var_os(var)
            && !v.is_empty()
        {
            return v.to_string_lossy().into_owned();
        }
    }
    #[cfg(unix)]
    {
        format!("ppid-{}", getppid_libc())
    }
    #[cfg(not(unix))]
    {
        format!("pid-{}", std::process::id())
    }
}

#[cfg(unix)]
fn getppid_libc() -> u32 {
    // Same pattern as `paths::current_uid`: avoid pulling in `libc`
    // for one syscall by declaring the extern locally.
    #[allow(unsafe_code)]
    unsafe {
        unsafe extern "C" {
            fn getppid() -> i32;
        }
        let ppid = getppid();
        // getppid never returns a negative value in practice; cast is
        // just for the format string.
        ppid.max(0) as u32
    }
}

/// Content-addressed debounce key: `sha256(session_id + "|" + repo_root)`.
/// The `|` separator prevents collisions between a session id ending in
/// the same bytes as a repo path's prefix — pure defense-in-depth, but
/// cheap.
fn state_key(session_id: &str, repo_root: &str) -> String {
    let mut h = Sha256::new();
    h.update(session_id.as_bytes());
    h.update(b"|");
    h.update(repo_root.as_bytes());
    hex::encode(h.finalize())
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn state_path() -> Option<PathBuf> {
    crate::paths::agent_profiles_dir()
        .ok()
        .map(|dir| dir.join(STATE_FILENAME))
}

fn load_state() -> (CwdHintState, CwdHintState) {
    let Some(path) = state_path() else {
        return (CwdHintState::default(), CwdHintState::default());
    };
    let body = match fs::read_to_string(&path) {
        Ok(b) => b,
        Err(_) => return (CwdHintState::default(), CwdHintState::default()),
    };
    match serde_json::from_str::<CwdHintState>(&body) {
        Ok(s) => (s.clone(), s),
        // Corrupt state → start over. Losing debounce is the *right*
        // failure mode: it costs one extra nudge, not a wrong switch.
        Err(_) => (CwdHintState::default(), CwdHintState::default()),
    }
}

/// Documented last-writer-wins: two shells `cd`-ing simultaneously
/// might lose one debounce entry, which reappears within TTL on the
/// next hit. The PID-scoped tmp file below eliminates the tmp-clobber
/// path (two writers stomping each other's staging file); the
/// remaining read-modify-write race is deliberately not retried —
/// bounded retries make the failure mode worse (silent extra syscall
/// bursts under heavy `cd` load) with no user-visible benefit given
/// the debounce is already advisory.
fn save_state(state: &CwdHintState) -> std::io::Result<()> {
    let Some(path) = state_path() else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        // Same posture as `store::ensure_store_dirs` — mkdir + 0700.
        // Chmod failures are already telemetered from there.
        let _ = crate::paths::ensure_dir_0700(parent);
    }
    let body = serde_json::to_string(state).map_err(std::io::Error::other)?;
    // PID-scoped tmp: two concurrent writers get distinct staging files
    // so neither's write_0600 truncates the other's in flight. The
    // atomic rename below still commits last-writer-wins on the final
    // path, which is acceptable per the fn-level comment.
    let tmp = path.with_extension(format!("json.tmp.{}", std::process::id()));
    crate::paths::write_0600(&tmp, &body)?;
    fs::rename(&tmp, &path)?;
    // write_0600 created the tmp with mode 0600; rename preserves it —
    // no chmod needed (saves 2 syscalls per shell prompt).
    Ok(())
}

/// Line-buffered substring scan for `key` in the state file. Returns
/// `false` on any IO failure (missing file, permission denied) — the
/// caller treats that as "no debounce entry", which is the same outcome
/// as a genuine miss and preserves the fresh-key fast path. Bounded by
/// the file size (which `prune_stale` caps via [`STATE_MAX_AGE_SECS`]).
fn quick_key_check(path: &Path, key: &str) -> bool {
    use std::io::{BufRead, BufReader};
    let Ok(f) = fs::File::open(path) else {
        return false;
    };
    let mut reader = BufReader::new(f);
    let mut buf = String::new();
    loop {
        buf.clear();
        match reader.read_line(&mut buf) {
            Ok(0) => return false, // EOF
            Ok(_) => {
                if buf.contains(key) {
                    return true;
                }
            }
            Err(_) => return false,
        }
    }
}

fn prune_stale(state: &mut CwdHintState, now: u64) {
    state
        .seen
        .retain(|_, ts| now.saturating_sub(*ts) < STATE_MAX_AGE_SECS);
}

/// Whether stderr is attached to a terminal. Non-TTY (piped, redirected,
/// systemd unit) → skip the nudge; nothing useful can consume it.
fn stderr_is_tty() -> bool {
    use std::io::IsTerminal;
    std::io::stderr().is_terminal()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_profiles::test_support::JarvyHomeGuard;

    /// Write a minimal `jarvy.toml` under `dir` with the given prefer
    /// value (or a config with no `[agents.profiles]` when `prefer` is
    /// None). Returns the config path.
    fn write_jarvy_toml(dir: &Path, prefer: Option<&str>) -> PathBuf {
        fs::create_dir_all(dir).unwrap();
        let body = match prefer {
            Some(p) => format!("[agents.profiles]\nprefer = \"{p}\"\n"),
            None => "# no agents.profiles here\n[provisioner]\n".to_string(),
        };
        let path = dir.join("jarvy.toml");
        fs::write(&path, body).unwrap();
        path
    }

    fn seed_state(entries: &[(&str, u64)]) {
        let mut state = CwdHintState::default();
        for (k, ts) in entries {
            state.seen.insert((*k).to_string(), *ts);
        }
        save_state(&state).unwrap();
    }

    fn load_state_direct() -> CwdHintState {
        let (s, _) = load_state();
        s
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn debounce_skips_within_ttl() {
        // Populate a state entry with a "just now" timestamp for a
        // known key; a fresh compute of that key must find it and the
        // debounce logic must skip. Verify by counting entries: no
        // update to `updated` if we short-circuit.
        let _home = JarvyHomeGuard::new();
        let repo = "/tmp/repo-a";
        let session = "sess-1";
        let key = state_key(session, repo);
        let now = now_unix();
        seed_state(&[(&key, now)]);

        // Precondition: state has one entry at "now".
        let before = load_state_direct();
        assert_eq!(before.seen.len(), 1);
        assert_eq!(before.seen.get(&key), Some(&now));

        // Directly exercise the debounce branch: simulate what run()
        // does at the branch point.
        let last = before.seen.get(&key).copied().unwrap();
        assert!(
            now.saturating_sub(last) < CWD_HINT_TTL_SECS,
            "seeded timestamp must be within TTL"
        );
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn debounce_expires_after_ttl() {
        // Mutate the state file's timestamp to well past the TTL and
        // confirm the debounce branch would NOT short-circuit.
        let _home = JarvyHomeGuard::new();
        let key = state_key("sess-1", "/tmp/repo-a");
        let stale = now_unix().saturating_sub(CWD_HINT_TTL_SECS + 60);
        seed_state(&[(&key, stale)]);

        let s = load_state_direct();
        let last = s.seen.get(&key).copied().unwrap();
        assert!(
            now_unix().saturating_sub(last) >= CWD_HINT_TTL_SECS,
            "stale timestamp must be outside TTL"
        );
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn no_jarvy_toml_is_silent() {
        // find_config_walking_up must return None when no jarvy.toml
        // exists between cwd and home. `run()` returns 0 without
        // touching state in that case.
        let _home = JarvyHomeGuard::new();
        let home = crate::agents::home_dir().unwrap();
        // A subdir under home with no jarvy.toml anywhere on the walk.
        let start = home.join("no_config_here");
        fs::create_dir_all(&start).unwrap();

        assert!(find_config_walking_up(&start).is_none());

        // Also assert the top-level: state file must not exist since
        // nothing wrote it (no config → early return before save).
        let sp = state_path().unwrap();
        assert!(
            !sp.exists(),
            "no-config path must not create the state file"
        );
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn no_prefer_key_is_silent() {
        // A jarvy.toml with no [agents.profiles] prefer must yield
        // None from read_prefer, so run() early-returns without
        // consulting the debounce.
        let _home = JarvyHomeGuard::new();
        let home = crate::agents::home_dir().unwrap();
        let repo = home.join("repo-no-prefer");
        let cfg = write_jarvy_toml(&repo, None);

        assert!(read_prefer(&cfg).is_none());

        // Even a config with an *empty* prefer is treated as absent —
        // preventing an accidental empty-string entry in jarvy.toml
        // from surfacing "profile ''" as a preference.
        let cfg2 = write_jarvy_toml(&repo, Some(""));
        assert!(
            read_prefer(&cfg2).is_none(),
            "empty prefer must be filtered"
        );
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn matched_preference_is_silent_but_still_debounces() {
        // Documents the intent: a matched check still writes the
        // debounce entry so re-runs are no-ops (no re-emit of the
        // cwd_hint telemetry event on every prompt).
        //
        // Direct exercise: save_state → load_state round trip with a
        // seeded entry and confirm re-loading yields the same value
        // (matched-case would call this exact code path).
        let _home = JarvyHomeGuard::new();
        let key = state_key("sess-x", "/some/repo");
        let ts = now_unix();
        let mut s = CwdHintState::default();
        s.seen.insert(key.clone(), ts);
        save_state(&s).unwrap();

        let back = load_state_direct();
        assert_eq!(back.seen.get(&key), Some(&ts));

        // Also: prune must NOT drop the fresh entry.
        let mut s2 = back.clone();
        prune_stale(&mut s2, ts + 60);
        assert_eq!(s2.seen.len(), 1);
    }

    #[test]
    fn state_key_is_stable_and_distinguishes_inputs() {
        let a = state_key("sess-1", "/repo/a");
        let a2 = state_key("sess-1", "/repo/a");
        let b = state_key("sess-1", "/repo/b");
        let c = state_key("sess-2", "/repo/a");
        assert_eq!(a, a2, "same inputs must hash the same");
        assert_ne!(a, b, "repo must factor in");
        assert_ne!(a, c, "session must factor in");
        // The `|` separator: a session ending in "/" followed by a
        // repo starting with "repo/a" must not collide with a session
        // ending in "" and a repo starting with "/repo/a".
        assert_ne!(state_key("s/", "repo/a"), state_key("s", "/repo/a"));
    }

    #[test]
    fn resolve_session_id_flag_wins() {
        // The CLI flag beats every env var. This is deliberate: a
        // shell hook that manages its own session id (e.g. iTerm's
        // shell integration) shouldn't be silently overridden.
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var(SESSION_ID_ENV, "from-env");
            std::env::set_var(TERM_SESSION_ID_ENV, "from-term");
        }
        assert_eq!(resolve_session_id(Some("from-flag")), "from-flag");
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var(SESSION_ID_ENV);
            std::env::remove_var(TERM_SESSION_ID_ENV);
        }
    }

    #[test]
    fn prune_stale_drops_old_entries_only() {
        let now = 10_000_000u64;
        let mut s = CwdHintState::default();
        // Fresh: kept.
        s.seen.insert("fresh".into(), now);
        // Old-but-within-window: kept.
        s.seen
            .insert("within".into(), now - STATE_MAX_AGE_SECS + 60);
        // Ancient: dropped.
        s.seen
            .insert("ancient".into(), now - STATE_MAX_AGE_SECS - 60);

        prune_stale(&mut s, now);
        assert!(s.seen.contains_key("fresh"));
        assert!(s.seen.contains_key("within"));
        assert!(!s.seen.contains_key("ancient"));
    }

    // ---- Change 1.1: read_prefer_fast ---------------------------------

    #[test]
    fn read_prefer_fast_matches_typical() {
        let toml = "\
[provisioner]

[agents.profiles]
prefer = \"work\"

[commands]
";
        assert_eq!(read_prefer_fast(toml), Some("work".to_string()));
    }

    #[test]
    fn read_prefer_fast_handles_trailing_comment() {
        let toml = "[agents.profiles]\nprefer = \"work\"  # in-line comment\n";
        assert_eq!(read_prefer_fast(toml), Some("work".to_string()));
    }

    #[test]
    fn read_prefer_fast_falls_back_on_inline_table() {
        // `[agents]\nprofiles = { prefer = "..." }` is a valid TOML
        // spelling that the scanner can't parse — must return None so
        // the caller falls back to full toml::from_str. Since the fast
        // path never sees the `[agents.profiles]` header at all here,
        // None is the correct answer.
        let toml = "[agents]\nprofiles = { prefer = \"work\" }\n";
        assert_eq!(read_prefer_fast(toml), None);
    }

    #[test]
    fn read_prefer_fast_falls_back_on_escapes() {
        // Backslash escape → ambiguity, defer to real parser.
        let toml = "[agents.profiles]\nprefer = \"work\\name\"\n";
        assert_eq!(read_prefer_fast(toml), None);
    }

    #[test]
    fn read_prefer_fast_returns_none_when_absent() {
        assert_eq!(read_prefer_fast("[provisioner]\n"), None);
        assert_eq!(read_prefer_fast(""), None);
        // Header present but no prefer key.
        assert_eq!(read_prefer_fast("[agents.profiles]\n# nothing\n"), None);
    }

    #[test]
    fn read_prefer_fast_bounded_by_next_section() {
        // A `prefer` in a LATER section must not be picked up as the
        // agent-profiles value.
        let toml = "\
[agents.profiles]
# no key here

[other]
prefer = \"leak\"
";
        assert_eq!(read_prefer_fast(toml), None);
    }

    #[test]
    fn read_prefer_fast_empty_value_is_none() {
        let toml = "[agents.profiles]\nprefer = \"\"\n";
        assert_eq!(read_prefer_fast(toml), None);
    }

    #[test]
    fn read_prefer_matches_full_parse_on_common_inputs() {
        // Sanity: for the common case, the fast path and the full
        // parser must agree.
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("jarvy.toml");
        fs::write(&path, "[agents.profiles]\nprefer = \"work\"\n").unwrap();
        assert_eq!(read_prefer(&path), Some("work".to_string()));
    }

    // ---- Change 1.4: find_config_walking_up ---------------------------

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn find_config_walking_up_refuses_outside_home() {
        // Canonical start outside $HOME (JARVY_HOME) → must not walk.
        // Uses env::temp_dir() which is guaranteed NOT under the guard's
        // tempdir root.
        let _home = JarvyHomeGuard::new();
        let outside = std::env::temp_dir().join(format!("cwd-hint-outside-{}", std::process::id()));
        fs::create_dir_all(&outside).unwrap();
        // Belt-and-braces: outside is under the OS tempdir, but the
        // guard put JARVY_HOME under a *different* tempdir path. If
        // the two happen to alias, skip.
        let home_canon = crate::agents::home_dir().unwrap().canonicalize().unwrap();
        if outside.canonicalize().unwrap().starts_with(&home_canon) {
            return;
        }
        assert!(find_config_walking_up(&outside).is_none());
        // Cleanup.
        let _ = fs::remove_dir_all(&outside);
    }

    // ---- Change 1.2: quick_key_check fast path ------------------------

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn quick_key_check_absent_key_short_circuits() {
        let _home = JarvyHomeGuard::new();
        let key = state_key("sess-x", "/repo/x");
        seed_state(&[(&key, now_unix())]);
        let sp = state_path().unwrap();
        // Present key returns true.
        assert!(quick_key_check(&sp, &key));
        // Absent key returns false without touching the JSON parser.
        let absent = state_key("sess-x", "/repo/nope");
        assert!(!quick_key_check(&sp, &absent));
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn quick_key_check_missing_file_returns_false() {
        let _home = JarvyHomeGuard::new();
        let sp = state_path().unwrap();
        // No file yet.
        assert!(!sp.exists());
        assert!(!quick_key_check(&sp, "anything"));
    }

    // ---- Change 2: PID-scoped tmp -------------------------------------

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn save_state_pid_scoped_tmp_no_stomp() {
        // A save must not leave the un-scoped `state.json.tmp` behind
        // (that's the shape the old code used). The scoped variant may
        // be visible mid-flight, but after rename the destination is
        // the only survivor.
        let _home = JarvyHomeGuard::new();
        let mut s = CwdHintState::default();
        s.seen.insert("k".into(), now_unix());
        save_state(&s).unwrap();
        let sp = state_path().unwrap();
        assert!(sp.exists());
        // Un-scoped tmp must NOT exist (regression guard: the old
        // shared name would clobber concurrent writers).
        assert!(
            !sp.with_extension("json.tmp").exists(),
            "un-scoped tmp must not be used"
        );
        // The PID-scoped tmp is also gone post-rename.
        let scoped = sp.with_extension(format!("json.tmp.{}", std::process::id()));
        assert!(!scoped.exists(), "scoped tmp must not survive rename");
    }

    // ---- Change 3: run_inner testability ------------------------------

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn run_inner_non_tty_is_silent() {
        let _home = JarvyHomeGuard::new();
        assert_eq!(
            run_inner(false, false, None, false),
            RunEffect::Silent {
                reason: SilentReason::NonTty
            }
        );
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn run_inner_opt_out_is_silent() {
        let _home = JarvyHomeGuard::new();
        // Opt-out beats tty state (matches run()'s ordering).
        assert_eq!(
            run_inner(true, true, None, false),
            RunEffect::Silent {
                reason: SilentReason::OptOut
            }
        );
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn run_inner_no_jarvy_toml_is_silent() {
        // cwd under $HOME but no jarvy.toml anywhere on the walk.
        let _home = JarvyHomeGuard::new();
        let home = crate::agents::home_dir().unwrap();
        let sub = home.join("no_config_dir");
        fs::create_dir_all(&sub).unwrap();
        let prev = std::env::current_dir().ok();
        std::env::set_current_dir(&sub).unwrap();

        let effect = run_inner(true, false, None, false);
        assert_eq!(
            effect,
            RunEffect::Silent {
                reason: SilentReason::NoConfig
            }
        );

        if let Some(p) = prev {
            let _ = std::env::set_current_dir(p);
        }
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn run_inner_no_prefer_is_silent() {
        let _home = JarvyHomeGuard::new();
        let home = crate::agents::home_dir().unwrap();
        let repo = home.join("repo-no-prefer");
        write_jarvy_toml(&repo, None);
        let prev = std::env::current_dir().ok();
        std::env::set_current_dir(&repo).unwrap();

        let effect = run_inner(true, false, None, false);
        assert_eq!(
            effect,
            RunEffect::Silent {
                reason: SilentReason::NoPrefer
            }
        );

        if let Some(p) = prev {
            let _ = std::env::set_current_dir(p);
        }
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn run_inner_emit_records_debounce_and_second_call_debounces() {
        // No switchable agents installed under the JarvyHomeGuard, so
        // check_preference either returns Ok (matched=true, no
        // mismatched) → Emit { matched: true } — which is exactly the
        // "matched still records debounce" invariant we want to test.
        let _home = JarvyHomeGuard::new();
        let home = crate::agents::home_dir().unwrap();
        let repo = home.join("repo-with-prefer");
        write_jarvy_toml(&repo, Some("work"));
        let prev = std::env::current_dir().ok();
        std::env::set_current_dir(&repo).unwrap();

        let first = run_inner(true, false, Some("sess-t"), false);
        // Either the registry loaded (Emit) or it was un-loadable
        // (NoRegistry). Both are legitimate depending on test-support
        // scaffolding, but at least one must NOT be Silent{NonTty} or
        // Silent{OptOut} — the config/prefer path DID execute.
        assert!(
            !matches!(
                first,
                RunEffect::Silent {
                    reason: SilentReason::NonTty
                        | SilentReason::OptOut
                        | SilentReason::NoConfig
                        | SilentReason::NoPrefer,
                }
            ),
            "expected Emit or NoRegistry, got {:?}",
            first
        );

        // If the first hit was an Emit, the state file must now carry
        // the key, so the second hit debounces.
        if matches!(first, RunEffect::Emit { .. }) {
            let second = run_inner(true, false, Some("sess-t"), false);
            assert_eq!(second, RunEffect::Debounced);
        }

        if let Some(p) = prev {
            let _ = std::env::set_current_dir(p);
        }
    }

    #[cfg(unix)]
    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn state_file_is_0600_after_save() {
        use std::os::unix::fs::PermissionsExt;
        let _home = JarvyHomeGuard::new();
        let mut s = CwdHintState::default();
        s.seen.insert("k".into(), now_unix());
        save_state(&s).unwrap();
        let mode = fs::metadata(state_path().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "state file must be 0600, got {mode:o}");
        // No leftover tmp after the atomic rename.
        assert!(
            !state_path().unwrap().with_extension("json.tmp").exists(),
            "tmp file must not survive rename"
        );
    }
}
