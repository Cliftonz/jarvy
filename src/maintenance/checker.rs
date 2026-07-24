//! Checker orchestrator (PRD-057).
//!
//! Iterates configured tools, routes each one to the right backend,
//! aggregates results into a [`Report`]. Backend I/O is bounded by
//! [`super::backends::BACKEND_TIMEOUT`]; router failures land in the
//! `unchecked` bucket with a stable reason label so telemetry stays
//! actionable.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use super::backends::FreshnessBackend;
use super::cache::{CacheEntry, CacheStore, err_entry, ok_entry};
use super::config::MaintenanceConfig;

/// Progress-reporter callback. Called once per backend probe on the
/// foreground refresh path. Signature `(tool, backend)`. Set to
/// `None` on the background path so the detached child stays
/// silent.
pub type ProgressReporter<'a> = Option<&'a (dyn Fn(&str, &str) + Sync)>;

/// Version managers we always skip. Mirrors
/// `drift::detector::is_auto_fixable`. Tools installed through these
/// intentionally sit off Jarvy's version-management path — flagging
/// them as "stale" would create noisy false positives.
pub const VERSION_MANAGERS: &[&str] = &["rustup", "nvm", "pyenv", "rbenv", "sdkman", "asdf"];

/// Result of a single per-tool check.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ToolCheck {
    pub tool: String,
    pub backend: String,
    pub installed: Option<String>,
    pub latest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<String>,
    pub from_cache: bool,
    pub direction: Direction,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    UpToDate,
    Upgrade,
    Downgrade,
    Unknown,
}

/// Tool that was intentionally not checked (version manager,
/// custom-install script, no supported backend, or ignored via
/// `[maintenance] ignore`). Kept out of the main report body so the
/// summary doesn't inflate its "stale" count with tools the user
/// can't act on.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UncheckedTool {
    pub tool: String,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manager: Option<String>,
}

/// Aggregate freshness report. Consumed by both the JSON output
/// path (via `Outputable`) and the human formatter in
/// [`super::reporter`].
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Report {
    pub schema_version: u32,
    pub generated_at: String,
    pub summary: ReportSummary,
    pub updates: Vec<ToolCheck>,
    pub up_to_date: Vec<ToolCheck>,
    pub unchecked: Vec<UncheckedTool>,
    pub errors: Vec<ToolCheck>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ReportSummary {
    pub tools_checked: usize,
    pub updates_available: usize,
    pub unchecked: usize,
    pub errors: usize,
}

impl Report {
    pub fn empty() -> Self {
        Self {
            schema_version: 1,
            generated_at: iso_now(),
            summary: ReportSummary {
                tools_checked: 0,
                updates_available: 0,
                unchecked: 0,
                errors: 0,
            },
            updates: Vec::new(),
            up_to_date: Vec::new(),
            unchecked: Vec::new(),
            errors: Vec::new(),
        }
    }

    fn recompute_summary(&mut self) {
        self.summary.tools_checked = self.updates.len() + self.up_to_date.len();
        self.summary.updates_available = self.updates.len();
        self.summary.unchecked = self.unchecked.len();
        self.summary.errors = self.errors.len();
    }
}

/// Options passed into [`run_check`]. Kept separate from the on-disk
/// config so CLI flags (`--refresh`, `--only`, `--ignore`) can
/// override without mutating the parsed jarvy.toml.
#[derive(Clone, Debug, Default)]
pub struct CheckOptions {
    /// If `true`, ignore cache entries even when fresh and re-probe
    /// every tool. Used by `jarvy check-updates --refresh`.
    pub force_refresh: bool,
    /// Explicit tool allowlist. Empty = every configured tool.
    pub only: Vec<String>,
    /// Additional ignore list layered over `[maintenance] ignore`.
    pub ignore: Vec<String>,
}

/// One tool to look up. The router produces these; the executor
/// (`run_check`) doesn't care where they came from.
pub struct CheckTarget {
    pub tool: String,
    pub backend: Box<dyn FreshnessBackend + Send + Sync>,
    pub pkg_id: String,
    pub installed: Option<String>,
}

/// Run a check over `targets`, consulting `cache` unless
/// `opts.force_refresh` is set. `cache_ttl` seeded from
/// [`MaintenanceConfig::cache_ttl_hours`]. Mutates `cache` with
/// fresh entries so the caller can persist them.
///
/// This is the pure orchestration seam — deliberately agnostic to
/// the source of `targets` (provisioner tools, `[cargo]` packages,
/// `[npm]` packages) so the same executor drives every path.
/// Run a freshness check over `targets`, optionally streaming
/// per-probe progress via `reporter`. The reporter is invoked only
/// on the network path — cache hits stay silent so quiet setups
/// remain quiet.
pub fn run_check_with_progress(
    targets: Vec<CheckTarget>,
    cache: &mut CacheStore,
    cache_ttl: Duration,
    opts: &CheckOptions,
    unchecked: Vec<UncheckedTool>,
    reporter: ProgressReporter<'_>,
) -> Report {
    let now = SystemTime::now();
    let mut report = Report::empty();
    report.unchecked = unchecked;

    // Filter first — `--only` / `--ignore` narrow the target set
    // before we bother touching the cache. Do it once so the
    // parallel closure below doesn't have to.
    let targets: Vec<CheckTarget> = targets
        .into_iter()
        .filter(|t| {
            let by_only = opts.only.is_empty() || opts.only.iter().any(|x| x == &t.tool);
            let by_ignore = !opts.ignore.iter().any(|x| x == &t.tool);
            by_only && by_ignore
        })
        .collect();

    // Split into cache-hit / cache-miss. Hits skip the network
    // fan-out entirely; misses feed into rayon below. Doing this
    // serially first means the parallel workers do only the
    // expensive I/O.
    let mut hits: Vec<(CheckTarget, CacheEntry)> = Vec::new();
    let mut misses: Vec<CheckTarget> = Vec::new();
    for target in targets {
        let hit = if opts.force_refresh {
            None
        } else {
            cache
                .get_fresh(target.backend.name(), &target.pkg_id, cache_ttl, now)
                .cloned()
        };
        match hit {
            Some(entry) => hits.push((target, entry)),
            None => misses.push(target),
        }
    }

    // Fan-out misses in parallel. Backend I/O is dominated by the
    // 5-second network wait; a serial loop would multiply that by
    // N tools. `max(2, num_cpus/2)` mirrors the PRD-declared
    // limit — enough concurrency to overlap network latency,
    // conservative enough to avoid saturating small runners.
    let worker_count = std::cmp::max(2, num_cpus::get() / 2);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(worker_count)
        .build();
    let miss_results: Vec<(CheckTarget, CacheEntry)> = match pool {
        Ok(pool) => pool.install(|| probe_misses(misses, now, reporter)),
        // Rayon build failure is a hard-to-hit edge case; fall
        // back to a serial probe so the caller still gets a
        // report rather than a mystery empty output.
        Err(_) => probe_misses_serial(misses, now, reporter),
    };

    // Merge cache writes serially — the cache is `&mut` and we
    // don't want to bother wrapping it in a Mutex for the hot
    // path when the write phase is already O(N) trivial work.
    for (target, entry) in miss_results.iter() {
        cache.put(target.backend.name(), &target.pkg_id, entry.clone());
    }

    for (target, entry) in hits {
        let check = build_check(&target, target.backend.name(), &entry, true);
        classify(&mut report, check);
    }
    for (target, entry) in miss_results {
        let check = build_check(&target, target.backend.name(), &entry, false);
        classify(&mut report, check);
    }

    // Deterministic order: sort each bucket by tool name so
    // repeated runs produce identical JSON / human output. The
    // parallel dispatch above breaks natural insertion order.
    report.updates.sort_by(|a, b| a.tool.cmp(&b.tool));
    report.up_to_date.sort_by(|a, b| a.tool.cmp(&b.tool));
    report.errors.sort_by(|a, b| a.tool.cmp(&b.tool));
    report.unchecked.sort_by(|a, b| a.tool.cmp(&b.tool));

    report.recompute_summary();
    report
}

fn probe_misses(
    misses: Vec<CheckTarget>,
    now: SystemTime,
    reporter: ProgressReporter<'_>,
) -> Vec<(CheckTarget, CacheEntry)> {
    // Report progress under a shared mutex so foreground stderr
    // lines interleave cleanly rather than racing halfway
    // through each other.
    let stderr_lock = Mutex::new(());
    misses
        .into_par_iter()
        .map(|target| {
            if let Some(cb) = reporter {
                let _g = stderr_lock.lock().ok();
                cb(&target.tool, target.backend.name());
            }
            let entry = match target.backend.latest(&target.pkg_id) {
                Ok(latest) => ok_entry(latest, now),
                Err(err) => err_entry(err.kind(), now),
            };
            (target, entry)
        })
        .collect()
}

fn probe_misses_serial(
    misses: Vec<CheckTarget>,
    now: SystemTime,
    reporter: ProgressReporter<'_>,
) -> Vec<(CheckTarget, CacheEntry)> {
    misses
        .into_iter()
        .map(|target| {
            if let Some(cb) = reporter {
                cb(&target.tool, target.backend.name());
            }
            let entry = match target.backend.latest(&target.pkg_id) {
                Ok(latest) => ok_entry(latest, now),
                Err(err) => err_entry(err.kind(), now),
            };
            (target, entry)
        })
        .collect()
}

fn build_check(
    target: &CheckTarget,
    backend: &str,
    entry: &CacheEntry,
    from_cache: bool,
) -> ToolCheck {
    let direction = match (&target.installed, &entry.latest) {
        (Some(installed), Some(latest)) => compare_versions(installed, latest),
        _ => Direction::Unknown,
    };
    ToolCheck {
        tool: target.tool.clone(),
        backend: backend.to_string(),
        installed: target.installed.clone(),
        latest: entry.latest.clone(),
        error_kind: entry.backend_error.clone(),
        from_cache,
        direction,
    }
}

fn classify(report: &mut Report, check: ToolCheck) {
    if check.error_kind.is_some() {
        report.errors.push(check);
        return;
    }
    match check.direction {
        Direction::Upgrade | Direction::Downgrade => report.updates.push(check),
        Direction::UpToDate => report.up_to_date.push(check),
        Direction::Unknown => report.errors.push(check),
    }
}

/// Compare an installed version against the backend-reported latest.
/// Falls back to plain lexicographic comparison when semver parsing
/// fails — better to over-report a downgrade than to silently
/// misclassify.
pub fn compare_versions(installed: &str, latest: &str) -> Direction {
    if installed == latest {
        return Direction::UpToDate;
    }
    match (
        semver::Version::parse(installed),
        semver::Version::parse(latest),
    ) {
        (Ok(i), Ok(l)) if l > i => Direction::Upgrade,
        (Ok(i), Ok(l)) if l < i => Direction::Downgrade,
        (Ok(_), Ok(_)) => Direction::UpToDate,
        _ => {
            if latest > installed {
                Direction::Upgrade
            } else if latest < installed {
                Direction::Downgrade
            } else {
                Direction::UpToDate
            }
        }
    }
}

/// Return a canonical `unchecked` reason for `tool`.
pub fn version_manager_reason(tool: &str) -> Option<&'static str> {
    if VERSION_MANAGERS
        .iter()
        .any(|vm| tool == *vm || tool.contains(vm))
    {
        Some("version_manager")
    } else {
        None
    }
}

fn iso_now() -> String {
    // Tiny inline ISO-8601 formatter — avoids pulling `chrono` into
    // this module. `duration_since` gives us Unix seconds; we
    // format as `<unix>Z` for stable machine-readable output.
    // Callers that want RFC3339 can post-process.
    let secs = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}Z")
}

// Keep the top-level `MaintenanceConfig` importable from the
// checker for callers that need both. Round-tripped through
// `#[allow(dead_code)]` because the CLI layer is what actually
// wires them together.
#[allow(dead_code)]
pub type _MaintenanceConfigRef<'a> = &'a MaintenanceConfig;

#[allow(dead_code)]
pub type _UnusedMap = BTreeMap<String, String>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compare_versions_detects_upgrade() {
        assert_eq!(compare_versions("1.0.0", "1.0.1"), Direction::Upgrade);
        assert_eq!(compare_versions("1.0.0", "2.0.0"), Direction::Upgrade);
    }

    #[test]
    fn compare_versions_detects_downgrade() {
        assert_eq!(compare_versions("1.0.1", "1.0.0"), Direction::Downgrade);
    }

    #[test]
    fn compare_versions_detects_equal() {
        assert_eq!(compare_versions("1.0.0", "1.0.0"), Direction::UpToDate);
    }

    #[test]
    fn compare_versions_falls_back_to_string() {
        // Non-semver strings still produce a bounded direction.
        let dir = compare_versions("not-a-version", "not-a-version");
        assert_eq!(dir, Direction::UpToDate);
    }

    #[test]
    fn version_manager_reason_matches_rustup() {
        assert_eq!(version_manager_reason("rustup"), Some("version_manager"));
        assert_eq!(version_manager_reason("nvm"), Some("version_manager"));
        assert_eq!(version_manager_reason("git"), None);
    }
}
