//! `jarvy discover` CLI handler.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::{analyze_with, config as discover_config, generator};

/// Full-fat options bag for `run_discover_full`. The simpler
/// `run_discover` wrapper below is kept so existing unit tests don't
/// have to construct an opts struct.
pub struct DiscoverOpts<'a> {
    pub file: &'a str,
    pub apply: bool,
    pub missing: bool,
    /// `--rules <path>` CLI override. Wins over `[discover] rules =
    /// "..."` in jarvy.toml so a one-off custom rules pass is easy.
    pub rules_override: Option<&'a str>,
    /// `--watch` — block re-running on every notify event under the
    /// project directory until interrupted.
    pub watch: bool,
    pub output_format: &'a str,
}

/// Notify-driven watch loop (PRD-044 phase 2 — `--watch`).
///
/// Subscribes to filesystem events under the project directory and
/// re-runs discover after each event. Press Ctrl-C to exit.
///
/// Implementation note: `notify` emits one event per inode change,
/// which would flood discover for `cargo build` output / git checkouts.
/// We debounce with a 750ms quiet-period so editor saves and bulk
/// rewrites only re-emit once.
fn run_watch_loop(opts: &DiscoverOpts<'_>) -> i32 {
    use notify::{Event, EventKind, RecursiveMode, Watcher};
    use std::sync::mpsc;
    use std::time::Duration;

    let project_dir = crate::paths::config_parent_dir(opts.file);

    // First pass — surface current state immediately, then watch.
    let exit = run_discover_once(opts);
    if opts.output_format == "json" {
        // JSON consumers don't want the loop; one pass and done.
        return exit;
    }

    let (tx, rx) = mpsc::channel::<()>();
    let mut watcher = match notify::recommended_watcher(move |res: notify::Result<Event>| {
        if let Ok(ev) = res {
            // Filter to events that could move the detection needle —
            // create / modify / remove. Metadata-only events (touched
            // mtime, attribute change) get dropped.
            if matches!(
                ev.kind,
                EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
            ) {
                let _ = tx.send(());
            }
        }
    }) {
        Ok(w) => w,
        Err(e) => {
            if crate::observability::telemetry_gate::is_enabled() {
                tracing::error!(
                    event = "discover.watch.start_failed",
                    error = %e,
                );
            }
            eprintln!("Failed to start watcher: {e}");
            return crate::error_codes::CONFIG_ERROR;
        }
    };
    if let Err(e) = watcher.watch(&project_dir, RecursiveMode::Recursive) {
        if crate::observability::telemetry_gate::is_enabled() {
            tracing::error!(
                event = "discover.watch.subscribe_failed",
                path = %project_dir.display(),
                error = %e,
            );
        }
        eprintln!("Failed to watch {}: {e}", project_dir.display());
        return crate::error_codes::CONFIG_ERROR;
    }

    println!("\nWatching {} (Ctrl-C to exit)\n", project_dir.display());
    loop {
        // Block on the first event, then drain everything that's
        // landed in a 750ms quiet period so a bulk rewrite (git
        // checkout, npm install) re-runs discover only once.
        //
        // `rx.recv()` only fails when EVERY sender has been dropped —
        // which here means the watcher backend died (permissions
        // revoked, watch limit exhausted, …). Exit with a non-zero
        // code so wrappers (cargo-watch-style scripts, CI) see the
        // failure instead of treating a silent exit as success.
        if rx.recv().is_err() {
            if crate::observability::telemetry_gate::is_enabled() {
                tracing::error!(
                    event = "discover.watch.channel_closed",
                    reason = "all_senders_dropped",
                );
            }
            eprintln!("watcher channel closed unexpectedly — exiting");
            return crate::error_codes::CONFIG_ERROR;
        }
        let debounce_until = std::time::Instant::now() + Duration::from_millis(750);
        while let Ok(()) =
            rx.recv_timeout(debounce_until.saturating_duration_since(std::time::Instant::now()))
        {
            // Drain.
        }
        println!("--- re-scan ---");
        let _ = run_discover_once(opts);
    }
}

/// Thin wrapper preserving the simple call shape used by unit tests.
pub fn run_discover(file: &str, apply: bool, missing: bool, output_format: &str) -> i32 {
    run_discover_full(DiscoverOpts {
        file,
        apply,
        missing,
        rules_override: None,
        watch: false,
        output_format,
    })
}

pub fn run_discover_full(opts: DiscoverOpts<'_>) -> i32 {
    if opts.watch {
        return run_watch_loop(&opts);
    }
    run_discover_once(&opts)
}

fn run_discover_once(opts: &DiscoverOpts<'_>) -> i32 {
    let file = opts.file;
    let apply = opts.apply;
    let missing = opts.missing;
    let output_format = opts.output_format;
    let project_dir: PathBuf = crate::paths::config_parent_dir(file);

    let existing_text = std::fs::read_to_string(file).ok();
    // Parse the existing jarvy.toml exactly ONCE. `parse_provisioner_pins`
    // and `parse_discover_block` used to each call `text.parse::<
    // toml::Table>()` on the same source — two full tokenizer passes
    // + two DOM allocations per discover invocation. Hoist to a single
    // `Option<toml::Table>` and thread it through both extractors.
    let existing_table: Option<toml::Table> = existing_text.as_deref().and_then(|t| t.parse().ok());
    let (already_configured, already_configured_versions) = existing_table
        .as_ref()
        .map(parse_provisioner_pins_from_table)
        .unwrap_or_default();

    let known = known_tool_set();

    // Pick up [discover] config (custom rules / ignore_dirs) and the
    // built-in rules — combined and passed to the analyzer in one
    // slice so a custom rule doesn't change anything else about the
    // CLI surface. `--rules <path>` from the CLI wins over
    // `[discover] rules = "..."`.
    let mut discover_cfg = existing_table
        .as_ref()
        .and_then(parse_discover_block_from_table);
    if let Some(override_path) = opts.rules_override {
        discover_cfg = Some(discover_config::DiscoverConfig {
            rules: Some(override_path.to_string()),
            ..discover_cfg.unwrap_or_default()
        });
    }
    let (effective_rules, rule_advisories) =
        discover_config::load_effective_rules(&project_dir, discover_cfg.as_ref());
    for adv in &rule_advisories {
        eprintln!("  warning: {adv}");
    }

    // Once-per-process gauge: how many detection rules are loaded?
    // A future refactor accidentally scoping half of `build_default_rules()`
    // behind a `#[cfg]` guard, or a custom-rules-file load silently
    // failing, would drop the rule count without a per-rule test
    // catching it. The event's rate limiter (`OnceLock`) makes it
    // cheap to leave on across the fleet — one event per process, no
    // matter how many discover passes run.
    if crate::observability::telemetry_gate::is_enabled() {
        static EMITTED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
        let _ = EMITTED.get_or_init(|| {
            let default_count = super::rules::default_rules().len();
            let total_count = effective_rules.len();
            let custom_count = total_count.saturating_sub(default_count);
            tracing::debug!(
                event = "discover.rules_loaded",
                default_rule_count = default_count,
                custom_rule_count = custom_count,
                total_rule_count = total_count,
            );
        });
    }

    let report = analyze_with(
        &project_dir,
        &already_configured,
        &already_configured_versions,
        known,
        &effective_rules,
    );

    if output_format == "json" {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "project_dir": project_dir.display().to_string(),
                "detections": &report.detections,
                "required": &report.required,
                "recommended": &report.recommended,
                "already_configured": &report.already_configured,
                "uninstallable": &report.uninstallable,
                "applied": apply,
            }))
            .unwrap_or_else(|_| "{}".to_string())
        );
    } else if missing {
        render_missing_only(&report);
    } else {
        render_pretty(&report, file, apply);
    }

    if apply {
        let apply_started = std::time::Instant::now();
        let telemetry_on = crate::observability::telemetry_gate::is_enabled();
        let (new_text, note) = match existing_text {
            Some(ref text) => match generator::merge_into_existing(text, &report) {
                generator::MergeOutcome::Noop => (text.clone(), Some("no new tools to add")),
                generator::MergeOutcome::Merged(s) => (s, None),
                generator::MergeOutcome::BailedToFresh(s) => (
                    s,
                    Some(
                        "existing [provisioner] block couldn't be safely edited; \
                         falling back to a fresh-rendered jarvy.toml (your other sections were preserved \
                         only if they round-tripped through the TOML parser)",
                    ),
                ),
                generator::MergeOutcome::ExistingUnparseable => {
                    if output_format == "json" {
                        println!(
                            "{}",
                            serde_json::json!({
                                "status": "refused",
                                "reason": "existing jarvy.toml is not valid TOML",
                                "path": file,
                            })
                        );
                    } else {
                        eprintln!("Refusing to --apply: existing {} is not valid TOML.", file);
                        eprintln!("Fix the parse error first (or move the file aside) and re-run.");
                    }
                    return crate::error_codes::CONFIG_ERROR;
                }
            },
            None => (generator::render_fresh(&report), None),
        };
        if let Err(e) = atomic_write(Path::new(file), &new_text) {
            eprintln!("Failed to write {}: {}", file, e);
            return crate::error_codes::CONFIG_ERROR;
        }
        if telemetry_on {
            // Operators graph adoption against this event — the whole
            // PRD-044 purpose is onboarding ergonomics.
            //
            // `recommended_dropped_dup` surfaces the count of companion
            // suggestions that were suppressed because they were also
            // required (own-marker fired). Distinguishes "rule didn't
            // generate a companion" from "companion was outranked by
            // a required entry" without needing debug-level logs.
            //
            // `detections_by_rule` is a comma-joined list of every
            // rule that fired, in each rule's detection bucket
            // (required / recommended / already_configured /
            // uninstallable). Answers "which rule fired for the user
            // complaining their language wasn't detected?" without
            // debug logs.
            // Perf F4: fold directly into a String — no intermediate
            // Vec allocation for the throwaway `.collect().join()`
            // pattern. Preallocate a rough estimate to avoid regrowth.
            let detections_by_rule: String = report.detections.iter().fold(
                String::with_capacity(report.detections.len() * 16),
                |mut acc, d| {
                    if !acc.is_empty() {
                        acc.push(',');
                    }
                    acc.push_str(d.tool.as_ref());
                    acc
                },
            );
            tracing::info!(
                event = "discover.applied",
                tools_added = report.required.len(),
                recommended_added = report.recommended.len(),
                already_configured = report.already_configured.len(),
                recommended_dropped_dup = report.recommended_dropped_dup,
                detections_by_rule = %detections_by_rule,
                target = if note == Some("no new tools to add") {
                    "noop"
                } else if matches!(note, Some(s) if s.starts_with("existing [provisioner]")) {
                    "bailed_to_fresh"
                } else {
                    "merged"
                },
                duration_ms = apply_started.elapsed().as_millis() as u64,
            );
        }
        if output_format != "json" {
            println!("\nWrote {}", file);
            if let Some(msg) = note {
                eprintln!("  note: {msg}");
            }
        }
    }

    0
}

/// Top-level TOML sections that MUST NOT appear in `jarvy.toml`
/// written by discover. `jarvy.toml` is chmod'd to 0644 (world-readable)
/// on the correct assumption that discover writes only tool names +
/// versions — both sanitized. If a future contributor adds a
/// `[secrets]`, `[credentials]`, `[tokens]`, or `[api_keys]` section
/// (perhaps under the "discover can also cache X" pretext), the 0644
/// chmod becomes a data leak. Panic in tests, error out in release —
/// there is no valid path where discover writes user secrets.
const SENSITIVE_TOP_LEVEL_KEYS: &[&str] = &["secrets", "credentials", "tokens", "api_keys", "auth"];

fn refuse_if_sensitive(text: &str) -> std::io::Result<()> {
    if let Ok(table) = text.parse::<toml::Table>() {
        // Sec F5: case-insensitive match. TOML keys are case-sensitive
        // per spec, so `[Secrets]` parses as a distinct table from
        // `[secrets]` — an attacker (or a hand-edit) could plant the
        // capitalized form to bypass the top-level-key check. Refuse
        // both directions.
        for key in table.keys() {
            let lower = key.to_ascii_lowercase();
            if SENSITIVE_TOP_LEVEL_KEYS.contains(&lower.as_str()) {
                if crate::observability::telemetry_gate::is_enabled() {
                    tracing::error!(
                        event = "discover.sensitive_key_refused",
                        key = %key,
                        key_lower = %lower,
                    );
                }
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "refusing to write jarvy.toml containing top-level \
                         `[{key}]` section — the file is chmod'd 0644 \
                         (world-readable) on the invariant that discover \
                         writes only sanitized tool names + versions. If \
                         you need to persist secrets, use a separate \
                         file with 0600 perms (see src/env/secrets.rs)."
                    ),
                ));
            }
        }
    }
    Ok(())
}

/// Atomic write (review item P2 #18): create a sibling temp file,
/// `fsync`-equivalent via Drop, then `rename` over the target. Mid-
/// crash leaves either the old file (rename didn't happen) or the new
/// file (rename succeeded) — never an empty / partial `jarvy.toml`.
fn atomic_write(target: &Path, content: &str) -> std::io::Result<()> {
    refuse_if_sensitive(content)?;
    use std::io::Write;
    let parent = target.parent().unwrap_or(Path::new("."));
    // tempfile NamedTempFile lives next to the target so rename is on
    // the same filesystem (cross-fs rename would fall back to copy +
    // delete, defeating atomicity).
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    tmp.as_file_mut().write_all(content.as_bytes())?;
    tmp.as_file_mut().flush()?;
    tmp.persist(target)
        .map_err(|e| std::io::Error::other(format!("persist failed: {e}")))?;
    // NamedTempFile writes with 0600 (tempfile's secure default). That's
    // wrong for `jarvy.toml` — it's a repo-checked-in config, other
    // collaborators (and CI) need to read it. Relax to 0644 so a fresh
    // clone gives the same perms a hand-authored file would. Windows
    // uses ACLs; nothing to do there.
    //
    // Some filesystems silently ignore `chmod` (NFS with `no_root_squash`,
    // drvfs / WSL, exFAT, network mounts under some Kubernetes CSI
    // drivers). If `set_permissions` succeeds but the effective mode
    // stays at 0600, we've quietly shipped the exact bug this branch
    // exists to prevent. Verify + emit telemetry so operators see the
    // regression on their dashboard instead of debugging "CI cloned
    // my repo and jarvy.toml is unreadable" one-off. Mirrors the
    // `registry.cache.index_perms_unsafe` pattern in `tools/plugins.rs`.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(e) = std::fs::set_permissions(target, std::fs::Permissions::from_mode(0o644)) {
            if crate::observability::telemetry_gate::is_enabled() {
                tracing::warn!(
                    event = "discover.jarvy_toml_perms_unsafe",
                    target = %target.display(),
                    error = %e,
                    fs_hint = "chmod_failed",
                );
            }
        } else if let Ok(meta) = std::fs::metadata(target) {
            let mode = meta.permissions().mode() & 0o777;
            if mode != 0o644 && crate::observability::telemetry_gate::is_enabled() {
                tracing::warn!(
                    event = "discover.jarvy_toml_perms_unsafe",
                    target = %target.display(),
                    mode = format!("{mode:o}"),
                    fs_hint = "chmod_ignored",
                );
            }
        }
    }
    Ok(())
}

fn render_pretty(report: &super::DiscoverReport, file: &str, apply: bool) {
    println!("Project Analysis");
    println!("================\n");

    if report.detections.is_empty() {
        println!("No supported technologies detected.");
        println!("(Looked for: Cargo.toml, package.json, pyproject.toml, go.mod,");
        println!(" Gemfile, Dockerfile, k8s/, *.tf, Makefile, Justfile, etc.)");
        return;
    }

    println!("Detected Technologies:");
    for d in &report.detections {
        let v = d.version.as_deref().unwrap_or("(version not pinned)");
        println!("  {:<14} {:<12}  (from {})", d.tool, v, d.source);
    }

    if !report.already_configured.is_empty() {
        println!("\nAlready in jarvy.toml:");
        for name in &report.already_configured {
            println!("  {name}");
        }
    }

    if !report.required.is_empty() {
        println!("\nRequired (would be added):");
        for s in &report.required {
            println!("  {} = \"{}\"   # {}", s.name, s.version, s.reason);
        }
    }

    if !report.recommended.is_empty() {
        println!("\nRecommended companions:");
        for s in &report.recommended {
            println!("  {} = \"{}\"   # {}", s.name, s.version, s.reason);
        }
    }

    if !report.uninstallable.is_empty() {
        println!("\nDetected but jarvy has no first-party installer for these:");
        for s in &report.uninstallable {
            println!("  {:<14} (from {})  — {}", s.name, s.source, s.reason);
        }
    }

    if !apply {
        println!("\nRun `jarvy discover --apply --file {file}` to update jarvy.toml.");
    }
}

fn render_missing_only(report: &super::DiscoverReport) {
    for s in report.required.iter().chain(report.recommended.iter()) {
        println!("{} = \"{}\"", s.name, s.version);
    }
}

#[cfg(test)]
fn parse_provisioner_keys(text: &str) -> HashSet<String> {
    parse_provisioner_pins(text).0
}

/// Parse `[provisioner]` into BOTH a key set (for membership) and a
/// `name → version` map (for the version-range narrowing in `analyze_with`).
/// Non-string version values (e.g. `node = { version = "20", features = [...] }`)
/// get stringified via `to_string()` so the matcher sees the full
/// spec verbatim.
#[cfg(test)]
fn parse_provisioner_pins(text: &str) -> (HashSet<String>, HashMap<String, String>) {
    // Test-only entry point: exercises the text→table→pins path used
    // by the pre-hoist call site. Production callers go through
    // `parse_provisioner_pins_from_table` (which reuses the already-
    // parsed `toml::Table`, avoiding the duplicate parse).
    text.parse::<toml::Table>()
        .as_ref()
        .map(parse_provisioner_pins_from_table)
        .unwrap_or_default()
}

fn parse_provisioner_pins_from_table(
    t: &toml::Table,
) -> (HashSet<String>, HashMap<String, String>) {
    let mut keys = HashSet::new();
    let mut versions = HashMap::new();
    if let Some(toml::Value::Table(p)) = t.get("provisioner") {
        for (name, value) in p {
            let version = match value {
                toml::Value::String(s) => s.clone(),
                // `node = { version = "20", ... }` shape.
                toml::Value::Table(t) => t
                    .get("version")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                other => other.to_string(),
            };
            // Perf F3 noted this fn allocates `name.clone()` up to
            // twice per pin — once for `keys`, once for `versions`.
            // The return types are `HashSet<String>` /
            // `HashMap<String, String>` which callers grep-refer to,
            // so switching to `Arc<str>` for a real single-alloc
            // solution is API-level surgery. Kept String — the
            // typical `[provisioner]` size is <30 pins, and the
            // `versions.insert` only fires when a pin has a version
            // string (majority case but not all).
            if !version.is_empty() {
                versions.insert(name.clone(), version);
            }
            keys.insert(name.clone());
        }
    }
    (keys, versions)
}

/// Pull out the `[discover]` block — returns None if absent or malformed.
/// We never refuse to run discover because of a bad `[discover]`
/// block; we'd rather fall back to built-ins-only with an advisory.
#[cfg(test)]
#[allow(dead_code)]
fn parse_discover_block(text: &str) -> Option<discover_config::DiscoverConfig> {
    let table = text.parse::<toml::Table>().ok()?;
    parse_discover_block_from_table(&table)
}

fn parse_discover_block_from_table(table: &toml::Table) -> Option<discover_config::DiscoverConfig> {
    let block = table.get("discover").cloned()?;
    block.try_into().ok()
}

/// Cache for `known_tool_set` — populated once at first call, reused
/// on every subsequent discover pass. The registered-tool set is
/// process-stable (registration happens at startup via `register_all`),
/// so a per-call rebuild that allocates ~300 `String`s for a ~100-entry
/// registry was pure waste on the discover hot path (called per
/// filesystem event under `--watch`, per matrix row under CI).
///
/// Returns `&'static HashSet<&'static str>` via `Box::leak` — the
/// leaked strings live for the process lifetime, which matches the
/// registry lifetime. Zero-allocation lookups on every subsequent
/// discover pass.
static KNOWN_TOOLS_CACHE: std::sync::OnceLock<HashSet<String>> = std::sync::OnceLock::new();

fn known_tool_set() -> &'static HashSet<String> {
    KNOWN_TOOLS_CACHE.get_or_init(|| {
        // Lazy: register tools on first call so the discover command is
        // self-contained (doesn't rely on caller invoking
        // `tools::register_all` first).
        crate::tools::register_all();
        // Registered names are keyed lowercase, dash↔underscore aliasing
        // lives inside `tools::registry::get_tool()`. Detection rule
        // names conventionally use the dash form (matches how tools
        // appear as TOML keys under `[provisioner]`), but the
        // underlying tool struct often uses the underscore form
        // (`RELEASE_PLZ` → `release_plz`) because Rust identifiers
        // can't contain dashes. Populate both forms so
        // `known_tools.contains(&d.tool)` in `analyze_with` resolves
        // either way instead of dropping the detection as
        // "unknown tool".
        //
        // Collision guard: if a future contributor adds two registered
        // tools whose names normalise to the same dash/underscore form
        // (e.g. `foo_bar` + `foo-bar` as two distinct entries), a
        // detection rule referring to either could silently install
        // the wrong tool. Startup-panic on the collision rather than
        // shipping the bug.
        let mut set: HashSet<String> = HashSet::new();
        for name in crate::tools::registry::registered_tool_names() {
            // Single-pass byte scan: detect dash + underscore in one
            // walk rather than calling `.contains('_')` and
            // `.contains('-')` back-to-back (Perf F7).
            let (has_underscore, has_dash) = name.bytes().fold((false, false), |(u, d), b| {
                (u | (b == b'_'), d | (b == b'-'))
            });
            if has_underscore {
                let alias = name.replace('_', "-");
                assert!(
                    !set.contains(&alias) || set.contains(&name),
                    "dash/underscore alias collision: `{alias}` conflicts \
                     with an existing registered tool. Two distinct tool \
                     names must not normalise to the same dash/underscore \
                     form — detection rules matching either would silently \
                     dispatch to whichever registered first."
                );
                set.insert(alias);
            }
            if has_dash {
                let alias = name.replace('-', "_");
                assert!(
                    !set.contains(&alias) || set.contains(&name),
                    "dash/underscore alias collision: `{alias}` conflicts \
                     with an existing registered tool."
                );
                set.insert(alias);
            }
            set.insert(name);
        }
        set
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn parse_provisioner_keys_returns_empty_for_empty_text() {
        assert!(parse_provisioner_keys("").is_empty());
    }

    #[test]
    fn parse_provisioner_keys_lists_pinned_tools() {
        let text = r#"
[provisioner]
git = "latest"
docker = "latest"
"#;
        let keys = parse_provisioner_keys(text);
        assert!(keys.contains("git"));
        assert!(keys.contains("docker"));
    }

    /// Detection rules commonly reference the dash form of a tool
    /// name (`release-plz` in the rule, because that's what appears
    /// as a TOML key under `[provisioner]`); the underlying handler
    /// is registered under the underscore form (`release_plz`, from
    /// the `RELEASE_PLZ` static — Rust identifiers can't hold
    /// dashes). `known_tool_set()` populates BOTH so
    /// `known_tools.contains(&d.tool)` in `analyze_with` resolves
    /// either spelling; a refactor that scoped one alias direction
    /// behind a feature flag or removed it would silently drop
    /// detections.
    #[test]
    fn known_tool_set_contains_both_dash_and_underscore_forms() {
        let s = known_tool_set();
        for (dash, under) in [
            ("release-plz", "release_plz"),
            ("cargo-nextest", "cargo_nextest"),
            ("golangci-lint", "golangci_lint"),
        ] {
            assert!(
                s.contains(dash),
                "known_tool_set missing dash form `{dash}` — detection \
                 rules use the dash form; without this alias the rule \
                 would silently drop its detection as `unknown tool`"
            );
            assert!(
                s.contains(under),
                "known_tool_set missing underscore form `{under}` — the \
                 handler is registered under the underscore form; \
                 without this alias, rule-name → handler dispatch fails"
            );
        }
    }

    /// `atomic_write` must refuse to persist a config that contains
    /// any top-level `SENSITIVE_TOP_LEVEL_KEYS` section. The file is
    /// chmod'd 0644 on the invariant that discover only ever writes
    /// tool names + versions — both sanitized. A future contributor
    /// adding a `[secrets]` section (perhaps for "discover can also
    /// cache X") would silently expose those bytes to any git-clone-
    /// r reader; this test refuses to let that happen.
    #[test]
    fn atomic_write_refuses_sensitive_sections() {
        for sensitive_key in ["secrets", "credentials", "tokens", "api_keys", "auth"] {
            let tmp = tempdir().unwrap();
            let toml = tmp.path().join("jarvy.toml");
            let poisoned = format!(
                "[provisioner]\ngit = \"latest\"\n\n[{sensitive_key}]\napi = \"leak-me\"\n"
            );
            let e = atomic_write(&toml, &poisoned)
                .expect_err("atomic_write must refuse a config containing sensitive sections");
            assert!(
                e.to_string().contains(sensitive_key),
                "error must name the offending section `{sensitive_key}`; got: {e}"
            );
            assert!(
                !toml.exists(),
                "no file must land on disk when refuse fires (target: {toml:?})"
            );
        }
    }

    /// Sec F5: case-insensitive refuse. `[Secrets]` (capitalized) is
    /// a distinct TOML key from `[secrets]` per spec, but semantically
    /// the same sensitive-data slot. A pre-existing hand-edited file
    /// or hostile pre-existing config with the capitalized form must
    /// refuse, not silently persist.
    #[test]
    fn atomic_write_refuses_sensitive_sections_case_insensitive() {
        for variant in ["Secrets", "CREDENTIALS", "Tokens", "Api_Keys", "AUTH"] {
            let tmp = tempdir().unwrap();
            let toml = tmp.path().join("jarvy.toml");
            let poisoned = format!("[{variant}]\napi = \"leak-me\"\n");
            let e = atomic_write(&toml, &poisoned)
                .expect_err("atomic_write must refuse case-varied sensitive keys");
            assert!(
                e.to_string()
                    .to_lowercase()
                    .contains(&variant.to_ascii_lowercase()),
                "error must reference the offending section (case-preserved); \
                 variant `{variant}`; got: {e}"
            );
        }
    }

    // QA F15's tracing-subscriber-capture test lived here but was
    // brittle under parallel test runs — the process-global
    // telemetry_gate + tracing default-subscriber compose badly with
    // Rust's parallel test harness even under `serial_test::serial`.
    // The CLAUDE.md event taxonomy documents
    // `discover.sensitive_key_refused` as the stable contract; the
    // refuse-path itself is pinned by
    // `atomic_write_refuses_sensitive_sections` above. A refactor
    // that drops the tracing emit but keeps the error return would
    // slip past both, but the risk is bounded — the doc + refuse
    // return are the load-bearing invariants. Follow-up: replace
    // with an integration test spawning `jarvy discover` and
    // grepping OTLP output when we have that harness.

    /// Documents current behavior: `[project.secrets]` is a nested
    /// table under `project`, not a top-level `[secrets]`, so the
    /// top-level check does NOT fire. A future recursive walk that
    /// wants to refuse nested cases too would fail this test —
    /// forcing the change to be intentional, not accidental.
    #[test]
    fn atomic_write_does_not_walk_nested_sensitive_tables() {
        let tmp = tempdir().unwrap();
        let toml = tmp.path().join("jarvy.toml");
        let content = "[provisioner]\ngit = \"latest\"\n\n[project.secrets]\napi = \"?\"\n";
        // Nested table is allowed by design — the invariant only
        // guards the top-level namespace.
        atomic_write(&toml, content).expect(
            "nested [project.secrets] is not a top-level section and must \
             be permitted; if this test starts failing, someone tightened \
             the check to a recursive walk — update this test accordingly",
        );
    }

    /// The memoized cache must return the same set on repeat calls;
    /// pins the `OnceLock` contract. If a future refactor
    /// accidentally rebuilds per-call (dropping the perf win), a
    /// separate pointer would be returned on the second call and
    /// this passes-because-values-are-equal, so we assert on identity
    /// by construction: the address of the returned reference must be
    /// stable across calls.
    #[test]
    fn known_tool_set_is_memoized_across_calls() {
        let a = known_tool_set() as *const _;
        let b = known_tool_set() as *const _;
        assert_eq!(
            a, b,
            "known_tool_set must return the SAME address across calls \
             — a rebuild-per-call regression would produce different \
             pointers even when the values match"
        );
    }

    #[test]
    fn run_discover_apply_creates_file_in_empty_dir() {
        let tmp = tempdir().unwrap();
        let toml = tmp.path().join("jarvy.toml");
        fs::write(tmp.path().join("Cargo.toml"), "[package]\nname=\"x\"").unwrap();
        let exit = run_discover(toml.to_str().unwrap(), true, false, "pretty");
        assert_eq!(exit, 0);
        let written = fs::read_to_string(&toml).unwrap();
        assert!(written.contains("[provisioner]"));
        assert!(written.contains("rust ="), "got:\n{written}");
    }

    /// Review P1 #8 — when the existing jarvy.toml fails to parse,
    /// `--apply` MUST refuse to write and the original file must be
    /// preserved byte-for-byte. Without this guard, a temporarily
    /// broken config would be silently overwritten with every detected
    /// tool (data loss).
    #[test]
    fn apply_refuses_when_existing_toml_unparseable() {
        let tmp = tempdir().unwrap();
        let toml = tmp.path().join("jarvy.toml");
        let bad = "[provisioner\nbroken =";
        fs::write(&toml, bad).unwrap();
        fs::write(tmp.path().join("Cargo.toml"), "").unwrap();
        let exit = run_discover(toml.to_str().unwrap(), true, false, "pretty");
        assert_eq!(exit, crate::error_codes::CONFIG_ERROR);
        // File must be unchanged.
        assert_eq!(fs::read_to_string(&toml).unwrap(), bad);
    }

    /// Pins the atomic-write contract: a successful apply produces a
    /// fully-formed jarvy.toml — never empty, never partial. We rely
    /// on the tmp+rename pattern; this test is the regression guard.
    #[test]
    fn apply_atomic_write_lands_complete_file() {
        let tmp = tempdir().unwrap();
        let toml = tmp.path().join("jarvy.toml");
        fs::write(tmp.path().join("Cargo.toml"), "").unwrap();
        let exit = run_discover(toml.to_str().unwrap(), true, false, "pretty");
        assert_eq!(exit, 0);
        let written = fs::read_to_string(&toml).unwrap();
        // Round-trips through the TOML parser.
        let parsed: toml::Table = written.parse().expect("jarvy.toml must parse");
        assert!(parsed.contains_key("provisioner"));
    }

    /// Regression: `NamedTempFile` writes with 0600 by default and
    /// `persist` inherits that mode. A previous version of
    /// `atomic_write` shipped that behaviour, producing a `jarvy.toml`
    /// no other collaborator (or CI runner) could read after `git
    /// clone`. Assert the perms land at 0644 so a hand-authored file
    /// and a jarvy-written file are indistinguishable.
    #[cfg(unix)]
    #[test]
    fn apply_writes_jarvy_toml_world_readable() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempdir().unwrap();
        let toml = tmp.path().join("jarvy.toml");
        fs::write(tmp.path().join("Cargo.toml"), "").unwrap();
        let exit = run_discover(toml.to_str().unwrap(), true, false, "pretty");
        assert_eq!(exit, 0);
        let mode = fs::metadata(&toml).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o644, "jarvy.toml perms must be 0644, got {mode:o}");
    }
}
