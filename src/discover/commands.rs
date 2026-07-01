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
    let (already_configured, already_configured_versions) = existing_text
        .as_deref()
        .map(parse_provisioner_pins)
        .unwrap_or_default();

    let known = known_tool_set();

    // Pick up [discover] config (custom rules / ignore_dirs) and the
    // built-in rules — combined and passed to the analyzer in one
    // slice so a custom rule doesn't change anything else about the
    // CLI surface. `--rules <path>` from the CLI wins over
    // `[discover] rules = "..."`.
    let mut discover_cfg = existing_text.as_deref().and_then(parse_discover_block);
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

    let report = analyze_with(
        &project_dir,
        &already_configured,
        &already_configured_versions,
        &known,
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
            tracing::info!(
                event = "discover.applied",
                tools_added = report.required.len(),
                recommended_added = report.recommended.len(),
                already_configured = report.already_configured.len(),
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

/// Atomic write (review item P2 #18): create a sibling temp file,
/// `fsync`-equivalent via Drop, then `rename` over the target. Mid-
/// crash leaves either the old file (rename didn't happen) or the new
/// file (rename succeeded) — never an empty / partial `jarvy.toml`.
fn atomic_write(target: &Path, content: &str) -> std::io::Result<()> {
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
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(target, std::fs::Permissions::from_mode(0o644));
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
fn parse_provisioner_pins(text: &str) -> (HashSet<String>, HashMap<String, String>) {
    let mut keys = HashSet::new();
    let mut versions = HashMap::new();
    if let Ok(t) = text.parse::<toml::Table>() {
        if let Some(toml::Value::Table(p)) = t.get("provisioner") {
            for (name, value) in p {
                keys.insert(name.clone());
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
                if !version.is_empty() {
                    versions.insert(name.clone(), version);
                }
            }
        }
    }
    (keys, versions)
}

/// Pull out the `[discover]` block — returns None if absent or malformed.
/// We never refuse to run discover because of a bad `[discover]`
/// block; we'd rather fall back to built-ins-only with an advisory.
fn parse_discover_block(text: &str) -> Option<discover_config::DiscoverConfig> {
    let table = text.parse::<toml::Table>().ok()?;
    let block = table.get("discover").cloned()?;
    block.try_into().ok()
}

fn known_tool_set() -> HashSet<String> {
    // Lazy: register tools on first call so the discover command is
    // self-contained (doesn't rely on caller invoking
    // `tools::register_all` first).
    crate::tools::register_all();
    // Registered names are keyed lowercase, dash↔underscore aliasing
    // lives inside `tools::registry::get_tool()`. Detection rule names
    // conventionally use the dash form (matches how tools appear as
    // TOML keys under `[provisioner]`), but the underlying tool struct
    // often uses the underscore form (`RELEASE_PLZ` → `release_plz`)
    // because Rust identifiers can't contain dashes. Populate both
    // forms so `known_tools.contains(&d.tool)` in `analyze_with`
    // resolves either way instead of dropping the detection as
    // "unknown tool".
    let mut set: HashSet<String> = HashSet::new();
    for name in crate::tools::registry::registered_tool_names() {
        if name.contains('_') {
            set.insert(name.replace('_', "-"));
        }
        if name.contains('-') {
            set.insert(name.replace('-', "_"));
        }
        set.insert(name);
    }
    set
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
