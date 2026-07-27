//! Checker orchestrator (PRD-057).
//!
//! Iterates configured tools, routes each one to the right backend,
//! aggregates results into a [`Report`]. Backend I/O is bounded by
//! [`super::backends::BACKEND_TIMEOUT`]; router failures land in the
//! `unchecked` bucket with a stable reason label so telemetry stays
//! actionable.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use super::backends::{BackendError, FreshnessBackend};
use super::cache::{CacheEntry, CacheStore, err_entry, ok_entry};
use super::config::MaintenanceConfig;
use crate::observability::telemetry_gate;

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

impl Direction {
    /// Bounded telemetry label. Use in preference to `?direction`
    /// (`Debug`) so OTLP labels don't inherit the compiler's
    /// idiosyncratic `Debug` formatting (which round-tripped through
    /// `serde` produces `"upgrade"` today but is not a stability
    /// guarantee).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UpToDate => "up_to_date",
            Self::Upgrade => "upgrade",
            Self::Downgrade => "downgrade",
            Self::Unknown => "unknown",
        }
    }
}

/// Coarse severity bucket for a stale-tool report — how far the
/// installed version is behind `latest`. Emitted on
/// `maintenance.stale_tool` in place of the raw version strings so
/// dashboards can graph "how many tools are at least a minor
/// behind?" without leaking private/internal pin identifiers
/// (`foo-cli@0.4.7-acmeco`) or blowing OTLP label cardinality.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LagBucket {
    Patch,
    Minor,
    Major,
    Downgrade,
    Unknown,
}

impl LagBucket {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Patch => "patch",
            Self::Minor => "minor",
            Self::Major => "major",
            Self::Downgrade => "downgrade",
            Self::Unknown => "unknown",
        }
    }

    /// Bucket the gap between `installed` and `latest` semver
    /// strings. Non-semver or absent inputs fall through to
    /// [`LagBucket::Unknown`].
    pub fn classify(installed: Option<&str>, latest: Option<&str>, direction: Direction) -> Self {
        if direction == Direction::Downgrade {
            return Self::Downgrade;
        }
        let (Some(installed), Some(latest)) = (installed, latest) else {
            return Self::Unknown;
        };
        let (Ok(i), Ok(l)) = (
            semver::Version::parse(installed),
            semver::Version::parse(latest),
        ) else {
            return Self::Unknown;
        };
        if l.major > i.major {
            Self::Major
        } else if l.minor > i.minor {
            Self::Minor
        } else if l.patch > i.patch {
            Self::Patch
        } else {
            Self::Unknown
        }
    }
}

/// Map a backend name to the top-level TOML section a tool was
/// declared under. Bounded to 7 labels for stable OTLP faceting.
/// This is the observable-side counterpart of the resolver's
/// per-section routing.
pub fn tool_kind_for_backend(backend: &str) -> &'static str {
    match backend {
        "cargo" => "cargo",
        "npm" => "npm",
        "pip" => "pip",
        "gem" => "gem",
        "go" => "go",
        "nuget" => "nuget",
        // Every native package manager (brew / apt / dnf / pacman
        // / apk / winget / choco / scoop) is reached via the
        // `[provisioner]` section — see `resolver::provisioner_backend`.
        _ => "provisioner",
    }
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

/// Cache-only run: classify every `target` against the on-disk
/// cache without ever spawning a backend probe. Absent entries land
/// in `report.unchecked` as `cache_miss` so the setup summary can
/// tell first-ever runs (no cache) from steady-state runs (cache
/// present, some targets not yet warmed).
///
/// This is the seam that fixes PRD-057 review items maint F2
/// (setup re-implements checker classification) and perf F1 (setup
/// summary spawns N subprocesses). Callers hand the same
/// `CheckTarget`s the full path uses — with `installed: None`
/// courtesy of [`super::resolver::resolve_targets_cheap`] — and
/// share the same classify/sort/summary code as the CLI's
/// foreground refresh.
pub fn run_check_cache_only(
    targets: Vec<CheckTarget>,
    cache: &CacheStore,
    mut unchecked: Vec<UncheckedTool>,
) -> Report {
    let mut report = Report::empty();

    for target in targets {
        let key = CacheStore::key(target.backend.name(), &target.pkg_id);
        let Some(entry) = cache.entries.get(&key) else {
            unchecked.push(UncheckedTool {
                tool: target.tool.clone(),
                reason: "cache_miss".to_string(),
                manager: None,
            });
            continue;
        };
        let check = build_check(&target, target.backend.name(), entry, true);
        classify(&mut report, check);
    }

    report.unchecked = unchecked;
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
            probe_one(target, now)
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
            probe_one(target, now)
        })
        .collect()
}

/// Single-target probe with observability wiring (PRD-057 obs F1 +
/// F2). Records per-backend probe duration and emits a bounded
/// `maintenance.backend_failed` tracing event on `Err(_)` so
/// on-call can graph "brew is flaking" without pivoting on report
/// stdout.
fn probe_one(target: CheckTarget, now: SystemTime) -> (CheckTarget, CacheEntry) {
    let backend_name = target.backend.name();
    let started = Instant::now();
    let outcome = target.backend.latest(&target.pkg_id);
    let elapsed = started.elapsed();
    let entry = match &outcome {
        Ok(latest) => {
            crate::observability::maintenance_metrics::backend_ok(backend_name, elapsed);
            ok_entry(latest.clone(), now)
        }
        Err(err) => {
            let error_kind = err.kind();
            // `error_kind` is a &'static str by BackendError contract;
            // safe as a bounded tracing/OTLP label.
            telemetry_gate::emit(|| {
                emit_backend_failed(backend_name, &target.tool, err, elapsed);
            });
            crate::observability::maintenance_metrics::backend_failed(
                backend_name,
                error_kind,
                elapsed,
            );
            err_entry(error_kind, now)
        }
    };
    (target, entry)
}

fn emit_backend_failed(backend: &'static str, tool: &str, err: &BackendError, elapsed: Duration) {
    let duration_ms = elapsed.as_millis() as u64;
    // Level discipline: `ManagerMissing` and `NotFound` are expected
    // conditions on multi-OS fleets — dropping them to `debug!` keeps
    // the warn stream signal-strong. Everything else is a genuine
    // backend anomaly (timeout, parse drift, permission denial).
    match err {
        BackendError::ManagerMissing | BackendError::NotFound => {
            tracing::debug!(
                event = "maintenance.backend_failed",
                backend = %backend,
                tool = %tool,
                error_kind = err.kind(),
                duration_ms = duration_ms,
            );
        }
        _ => {
            tracing::warn!(
                event = "maintenance.backend_failed",
                backend = %backend,
                tool = %tool,
                error_kind = err.kind(),
                duration_ms = duration_ms,
            );
        }
    }
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

    #[test]
    fn direction_as_str_covers_every_variant() {
        assert_eq!(Direction::UpToDate.as_str(), "up_to_date");
        assert_eq!(Direction::Upgrade.as_str(), "upgrade");
        assert_eq!(Direction::Downgrade.as_str(), "downgrade");
        assert_eq!(Direction::Unknown.as_str(), "unknown");
    }

    #[test]
    fn lag_bucket_classify_patch_minor_major() {
        assert_eq!(
            LagBucket::classify(Some("1.0.0"), Some("1.0.1"), Direction::Upgrade),
            LagBucket::Patch
        );
        assert_eq!(
            LagBucket::classify(Some("1.0.0"), Some("1.1.0"), Direction::Upgrade),
            LagBucket::Minor
        );
        assert_eq!(
            LagBucket::classify(Some("1.0.0"), Some("2.0.0"), Direction::Upgrade),
            LagBucket::Major
        );
    }

    #[test]
    fn lag_bucket_downgrade_takes_precedence() {
        // Downgrade direction wins even if the numeric gap looks
        // like a patch.
        assert_eq!(
            LagBucket::classify(Some("1.0.1"), Some("1.0.0"), Direction::Downgrade),
            LagBucket::Downgrade
        );
    }

    #[test]
    fn lag_bucket_non_semver_falls_back_to_unknown() {
        assert_eq!(
            LagBucket::classify(Some("latest"), Some("1.0.0"), Direction::Upgrade),
            LagBucket::Unknown
        );
        assert_eq!(
            LagBucket::classify(None, Some("1.0.0"), Direction::Upgrade),
            LagBucket::Unknown
        );
        assert_eq!(
            LagBucket::classify(Some("1.0.0"), None, Direction::Upgrade),
            LagBucket::Unknown
        );
    }

    #[test]
    fn tool_kind_covers_every_backend() {
        assert_eq!(tool_kind_for_backend("cargo"), "cargo");
        assert_eq!(tool_kind_for_backend("npm"), "npm");
        assert_eq!(tool_kind_for_backend("pip"), "pip");
        assert_eq!(tool_kind_for_backend("gem"), "gem");
        assert_eq!(tool_kind_for_backend("go"), "go");
        assert_eq!(tool_kind_for_backend("nuget"), "nuget");
        assert_eq!(tool_kind_for_backend("brew"), "provisioner");
        assert_eq!(tool_kind_for_backend("apt"), "provisioner");
        assert_eq!(tool_kind_for_backend("dnf"), "provisioner");
        assert_eq!(tool_kind_for_backend("winget"), "provisioner");
        assert_eq!(tool_kind_for_backend("choco"), "provisioner");
        assert_eq!(tool_kind_for_backend("scoop"), "provisioner");
        assert_eq!(tool_kind_for_backend("apk"), "provisioner");
        assert_eq!(tool_kind_for_backend("pacman"), "provisioner");
    }
}
