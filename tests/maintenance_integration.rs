//! Library-level integration for the freshness checker (PRD-057).
//!
//! Exercises `checker::run_check_with_progress` end-to-end with
//! stub backends so the whole pipeline (cache lookup, backend
//! fan-out, classification, sort, cache mutation) runs
//! deterministically without touching the network or any real
//! package manager.
//!
//! The `FreshnessBackend` trait is part of Jarvy's public
//! surface — tests here also serve as executable documentation
//! for third-party backend plugins.

use jarvy::maintenance::backends::{BackendError, FreshnessBackend};
use jarvy::maintenance::cache::CacheStore;
use jarvy::maintenance::checker::{self, CheckOptions, CheckTarget, Direction, UncheckedTool};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

// ---------------------------------------------------------------
// Stub backends
// ---------------------------------------------------------------

/// Backend that always succeeds with a canned version. Counts
/// invocations so we can assert cache hits skip the shell-out.
struct StaticBackend {
    name: &'static str,
    version: String,
    calls: Arc<AtomicUsize>,
}

impl StaticBackend {
    fn new(name: &'static str, version: impl Into<String>) -> Self {
        Self {
            name,
            version: version.into(),
            calls: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl FreshnessBackend for StaticBackend {
    fn name(&self) -> &'static str {
        self.name
    }

    fn latest(&self, _pkg_id: &str) -> Result<String, BackendError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.version.clone())
    }
}

/// Backend that always returns a specific error. Useful for
/// asserting the classifier routes failures into `errors`
/// rather than silently masking them.
struct FailingBackend {
    name: &'static str,
    err: BackendError,
}

impl FreshnessBackend for FailingBackend {
    fn name(&self) -> &'static str {
        self.name
    }

    fn latest(&self, _pkg_id: &str) -> Result<String, BackendError> {
        Err(self.err.clone())
    }
}

// ---------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------

fn target(
    tool: &str,
    backend: Box<dyn FreshnessBackend + Send + Sync>,
    pkg_id: &str,
    installed: Option<&str>,
) -> CheckTarget {
    CheckTarget {
        tool: tool.to_string(),
        backend,
        pkg_id: pkg_id.to_string(),
        installed: installed.map(String::from),
    }
}

fn quick_opts() -> CheckOptions {
    CheckOptions::default()
}

fn force_refresh_opts() -> CheckOptions {
    CheckOptions {
        force_refresh: true,
        ..Default::default()
    }
}

// ---------------------------------------------------------------
// Classification
// ---------------------------------------------------------------

#[test]
fn upgrade_lands_in_updates_bucket() {
    let targets = vec![target(
        "jq",
        Box::new(StaticBackend::new("brew", "1.8.0")),
        "jq",
        Some("1.7.1"),
    )];
    let mut cache = CacheStore::default();
    let report = checker::run_check_with_progress(
        targets,
        &mut cache,
        Duration::from_secs(3600),
        &quick_opts(),
        Vec::new(),
        None,
    );
    assert_eq!(report.updates.len(), 1);
    assert!(report.up_to_date.is_empty());
    assert!(report.errors.is_empty());
    assert_eq!(report.updates[0].direction, Direction::Upgrade);
    assert_eq!(report.summary.updates_available, 1);
}

#[test]
fn same_version_lands_in_up_to_date() {
    let targets = vec![target(
        "jq",
        Box::new(StaticBackend::new("brew", "1.7.1")),
        "jq",
        Some("1.7.1"),
    )];
    let mut cache = CacheStore::default();
    let report = checker::run_check_with_progress(
        targets,
        &mut cache,
        Duration::from_secs(3600),
        &quick_opts(),
        Vec::new(),
        None,
    );
    assert!(report.updates.is_empty());
    assert_eq!(report.up_to_date.len(), 1);
    assert_eq!(report.up_to_date[0].direction, Direction::UpToDate);
}

#[test]
fn downgrade_lands_in_updates_bucket() {
    // Direction::Downgrade classifies as an update — the user
    // still deserves visibility ("the upstream is behind our
    // pin, maybe our pin drifted forward accidentally").
    let targets = vec![target(
        "jq",
        Box::new(StaticBackend::new("brew", "1.5.0")),
        "jq",
        Some("1.7.1"),
    )];
    let mut cache = CacheStore::default();
    let report = checker::run_check_with_progress(
        targets,
        &mut cache,
        Duration::from_secs(3600),
        &quick_opts(),
        Vec::new(),
        None,
    );
    assert_eq!(report.updates.len(), 1);
    assert_eq!(report.updates[0].direction, Direction::Downgrade);
}

#[test]
fn missing_installed_version_lands_in_errors() {
    // Unknown installed → Direction::Unknown → errors bucket.
    let targets = vec![target(
        "jq",
        Box::new(StaticBackend::new("brew", "1.8.0")),
        "jq",
        None,
    )];
    let mut cache = CacheStore::default();
    let report = checker::run_check_with_progress(
        targets,
        &mut cache,
        Duration::from_secs(3600),
        &quick_opts(),
        Vec::new(),
        None,
    );
    assert!(report.updates.is_empty());
    assert_eq!(report.errors.len(), 1);
    assert_eq!(report.errors[0].direction, Direction::Unknown);
}

#[test]
fn backend_error_lands_in_errors_with_kind() {
    let targets = vec![target(
        "jq",
        Box::new(FailingBackend {
            name: "brew",
            err: BackendError::Timeout,
        }),
        "jq",
        Some("1.7.1"),
    )];
    let mut cache = CacheStore::default();
    let report = checker::run_check_with_progress(
        targets,
        &mut cache,
        Duration::from_secs(3600),
        &quick_opts(),
        Vec::new(),
        None,
    );
    assert_eq!(report.errors.len(), 1);
    assert_eq!(
        report.errors[0].error_kind.as_deref(),
        Some("timeout"),
        "error_kind should be the bounded label"
    );
}

// ---------------------------------------------------------------
// Cache interactions
// ---------------------------------------------------------------

#[test]
fn cache_hit_skips_backend_call() {
    let backend = StaticBackend::new("brew", "1.8.0");
    let calls_ref = backend.calls.clone();

    let mut cache = CacheStore::default();
    // Pre-populate a fresh hit so the checker skips the probe.
    cache.put(
        "brew",
        "jq",
        jarvy::maintenance::cache::ok_entry("1.7.1", std::time::SystemTime::now()),
    );

    let targets = vec![target("jq", Box::new(backend), "jq", Some("1.7.1"))];
    let report = checker::run_check_with_progress(
        targets,
        &mut cache,
        Duration::from_secs(3600),
        &quick_opts(),
        Vec::new(),
        None,
    );
    assert_eq!(
        calls_ref.load(Ordering::SeqCst),
        0,
        "backend must not fire on cache hit"
    );
    assert_eq!(report.up_to_date.len(), 1);
    assert!(report.up_to_date[0].from_cache);
}

#[test]
fn force_refresh_bypasses_cache() {
    let backend = StaticBackend::new("brew", "1.8.0");
    let calls_ref = backend.calls.clone();

    let mut cache = CacheStore::default();
    cache.put(
        "brew",
        "jq",
        jarvy::maintenance::cache::ok_entry("1.7.1", std::time::SystemTime::now()),
    );

    let targets = vec![target("jq", Box::new(backend), "jq", Some("1.7.1"))];
    let report = checker::run_check_with_progress(
        targets,
        &mut cache,
        Duration::from_secs(3600),
        &force_refresh_opts(),
        Vec::new(),
        None,
    );
    assert_eq!(calls_ref.load(Ordering::SeqCst), 1);
    assert_eq!(
        report.updates.len(),
        1,
        "refreshed backend returned newer version"
    );
    assert!(!report.updates[0].from_cache);
}

#[test]
fn miss_populates_cache() {
    let backend = StaticBackend::new("brew", "1.8.0");
    let mut cache = CacheStore::default();
    let targets = vec![target("jq", Box::new(backend), "jq", Some("1.7.1"))];
    checker::run_check_with_progress(
        targets,
        &mut cache,
        Duration::from_secs(3600),
        &quick_opts(),
        Vec::new(),
        None,
    );
    let key = CacheStore::key("brew", "jq");
    let entry = cache.entries.get(&key).expect("entry populated");
    assert_eq!(entry.latest.as_deref(), Some("1.8.0"));
}

// ---------------------------------------------------------------
// Filters
// ---------------------------------------------------------------

#[test]
fn only_filter_narrows_to_named_tools() {
    let targets = vec![
        target(
            "jq",
            Box::new(StaticBackend::new("brew", "1.8.0")),
            "jq",
            Some("1.7.1"),
        ),
        target(
            "git",
            Box::new(StaticBackend::new("brew", "2.45.2")),
            "git",
            Some("2.45.2"),
        ),
    ];
    let mut cache = CacheStore::default();
    let opts = CheckOptions {
        only: vec!["jq".to_string()],
        ..Default::default()
    };
    let report = checker::run_check_with_progress(
        targets,
        &mut cache,
        Duration::from_secs(3600),
        &opts,
        Vec::new(),
        None,
    );
    assert_eq!(report.summary.tools_checked, 1);
    assert_eq!(report.updates.len(), 1);
    assert_eq!(report.updates[0].tool, "jq");
}

#[test]
fn ignore_filter_skips_named_tools() {
    let targets = vec![
        target(
            "jq",
            Box::new(StaticBackend::new("brew", "1.8.0")),
            "jq",
            Some("1.7.1"),
        ),
        target(
            "git",
            Box::new(StaticBackend::new("brew", "2.45.3")),
            "git",
            Some("2.45.2"),
        ),
    ];
    let mut cache = CacheStore::default();
    let opts = CheckOptions {
        ignore: vec!["git".to_string()],
        ..Default::default()
    };
    let report = checker::run_check_with_progress(
        targets,
        &mut cache,
        Duration::from_secs(3600),
        &opts,
        Vec::new(),
        None,
    );
    assert_eq!(report.summary.tools_checked, 1);
    assert!(report.updates.iter().all(|c| c.tool != "git"));
}

// ---------------------------------------------------------------
// Sort determinism
// ---------------------------------------------------------------

#[test]
fn report_buckets_sorted_by_tool_name() {
    // Insertion order shuffled — the sort at the end of
    // `run_check_with_progress` guarantees deterministic
    // output regardless of parallel dispatch order.
    let targets = vec![
        target(
            "zsh",
            Box::new(StaticBackend::new("brew", "5.9.1")),
            "zsh",
            Some("5.9.0"),
        ),
        target(
            "bash",
            Box::new(StaticBackend::new("brew", "5.2.32")),
            "bash",
            Some("5.2.31"),
        ),
        target(
            "fish",
            Box::new(StaticBackend::new("brew", "3.7.2")),
            "fish",
            Some("3.7.1"),
        ),
    ];
    let mut cache = CacheStore::default();
    let report = checker::run_check_with_progress(
        targets,
        &mut cache,
        Duration::from_secs(3600),
        &quick_opts(),
        Vec::new(),
        None,
    );
    let names: Vec<_> = report.updates.iter().map(|c| c.tool.clone()).collect();
    assert_eq!(names, vec!["bash", "fish", "zsh"]);
}

// ---------------------------------------------------------------
// Unchecked passthrough
// ---------------------------------------------------------------

#[test]
fn unchecked_passthrough_preserves_reasons() {
    let unchecked = vec![
        UncheckedTool {
            tool: "rust".to_string(),
            reason: "version_manager".to_string(),
            manager: Some("rustup".to_string()),
        },
        UncheckedTool {
            tool: "brew".to_string(),
            reason: "custom_install".to_string(),
            manager: None,
        },
    ];
    let mut cache = CacheStore::default();
    let report = checker::run_check_with_progress(
        Vec::new(),
        &mut cache,
        Duration::from_secs(3600),
        &quick_opts(),
        unchecked,
        None,
    );
    assert_eq!(report.summary.unchecked, 2);
    // Sort by tool name — brew, rust.
    assert_eq!(report.unchecked[0].tool, "brew");
    assert_eq!(report.unchecked[1].tool, "rust");
}

// ---------------------------------------------------------------
// Progress reporter
// ---------------------------------------------------------------

#[test]
fn progress_reporter_called_once_per_probe() {
    let targets = vec![
        target(
            "jq",
            Box::new(StaticBackend::new("brew", "1.8.0")),
            "jq",
            Some("1.7.1"),
        ),
        target(
            "git",
            Box::new(StaticBackend::new("brew", "2.45.3")),
            "git",
            Some("2.45.2"),
        ),
    ];
    let mut cache = CacheStore::default();
    let calls: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());
    let reporter = |tool: &str, backend: &str| {
        calls
            .lock()
            .unwrap()
            .push((tool.to_string(), backend.to_string()));
    };
    checker::run_check_with_progress(
        targets,
        &mut cache,
        Duration::from_secs(3600),
        &quick_opts(),
        Vec::new(),
        Some(&reporter),
    );
    let recorded = calls.lock().unwrap().clone();
    assert_eq!(recorded.len(), 2, "one call per probed target");
    assert!(recorded.iter().any(|(t, _)| t == "jq"));
    assert!(recorded.iter().any(|(t, _)| t == "git"));
}

#[test]
fn progress_reporter_skipped_for_cache_hits() {
    let mut cache = CacheStore::default();
    cache.put(
        "brew",
        "jq",
        jarvy::maintenance::cache::ok_entry("1.8.0", std::time::SystemTime::now()),
    );
    let targets = vec![target(
        "jq",
        Box::new(StaticBackend::new("brew", "1.8.0")),
        "jq",
        Some("1.7.1"),
    )];
    let calls: Mutex<usize> = Mutex::new(0);
    let reporter = |_: &str, _: &str| {
        *calls.lock().unwrap() += 1;
    };
    checker::run_check_with_progress(
        targets,
        &mut cache,
        Duration::from_secs(3600),
        &quick_opts(),
        Vec::new(),
        Some(&reporter),
    );
    assert_eq!(
        *calls.lock().unwrap(),
        0,
        "progress must not fire on cache hits"
    );
}

// ---------------------------------------------------------------
// compare_versions edge cases
// ---------------------------------------------------------------

#[test]
fn compare_versions_semver_covers_prerelease() {
    // Semver considers `1.0.0-beta` < `1.0.0`, and our compare
    // should surface that as Upgrade.
    assert_eq!(
        checker::compare_versions("1.0.0-beta", "1.0.0"),
        Direction::Upgrade
    );
    assert_eq!(
        checker::compare_versions("1.0.0", "1.0.0-beta"),
        Direction::Downgrade
    );
}

#[test]
fn compare_versions_non_semver_falls_back_to_lexicographic() {
    // Some backends (winget, apk) return version strings that
    // don't parse as strict semver. Compare falls back to plain
    // string comparison rather than misclassifying.
    let dir = checker::compare_versions("2024.01", "2024.02");
    assert!(matches!(dir, Direction::Upgrade | Direction::UpToDate));
}
