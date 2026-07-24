//! `jarvy check-updates` command handler (PRD-057).
//!
//! Three run modes:
//!
//! - **cache-read** (setup path, `--format` on but no flags via
//!   this handler): read the on-disk cache and print the summary.
//!   Handled inline by [`crate::commands::setup_cmd`], not this
//!   file.
//! - **foreground refresh** (`jarvy check-updates`,
//!   `--refresh`): probe every backend, write results to the
//!   cache, print the full report.
//! - **background refresh** (`--background`, invoked as the
//!   detached child from setup): probe silently, write cache,
//!   exit. No stdout, minimal stderr.

use std::time::Duration;

use crate::config::Config;
use crate::maintenance::{
    self, MaintenanceConfig,
    backends::BackendError,
    cache::{self, CacheStore},
    checker::{self, CheckOptions},
    lock::{self, LockOutcome},
    reporter,
    resolver::{self, ResolveInput},
};

/// Exit codes documented in PRD-057 § CLI surface.
const EXIT_OK: i32 = 0;
const EXIT_UPDATES_AVAILABLE: i32 = 1;
const EXIT_CONFIG_ERROR: i32 = 2;
const EXIT_BACKEND_UNAVAILABLE: i32 = 3;

/// Entry point.
pub fn run(
    file: &str,
    refresh: bool,
    background: bool,
    only: Option<&str>,
    ignore: Option<&str>,
    include_unchecked: bool,
    output_format: &str,
) -> i32 {
    let config = Config::new(file);
    let maintenance = config.maintenance.clone().unwrap_or_default();

    if let Some(reason) = trust_gate_refuse(&maintenance) {
        if !background {
            eprintln!("check-updates skipped: {}", reason);
        }
        return EXIT_OK;
    }

    if maintenance.remote_refused() {
        if crate::observability::telemetry_gate::is_enabled() {
            tracing::warn!(
                event = "maintenance.remote_refused",
                reason = "allow_remote_not_set",
            );
        }
        if !background {
            eprintln!(
                "check-updates refused: remote config has `check_updates = true` \
                 without `[maintenance] allow_remote = true`."
            );
        }
        return EXIT_OK;
    }

    let input = build_resolve_input(&config);
    let (targets, unchecked) = resolver::resolve_targets(&input, &maintenance);
    if targets.is_empty() && unchecked.is_empty() {
        if !background {
            println!("check-updates: no eligible tools in [provisioner] / [cargo] / [npm].");
        }
        return EXIT_OK;
    }

    let mut cache = match cache::load() {
        Ok(c) => c,
        Err(cache::CacheError::NoHome) => {
            eprintln!("check-updates: cannot resolve home directory; skipping.");
            return EXIT_CONFIG_ERROR;
        }
        Err(e) => {
            if crate::observability::telemetry_gate::is_enabled() {
                tracing::warn!(
                    event = "maintenance.cache_read_failed",
                    error_kind = classify_cache_error(&e),
                );
            }
            CacheStore::default()
        }
    };

    let ttl = Duration::from_secs(maintenance.cache_ttl_hours.saturating_mul(3600));
    let opts = CheckOptions {
        force_refresh: refresh || background,
        only: split_csv(only),
        ignore: split_csv(ignore),
    };

    // Concurrent-refresher guard. Only bother when we're actually
    // going to write cache entries — a pure cache-read path
    // (no `--refresh` / `--background`) is idempotent and safe to
    // race. The guard is held for the lifetime of the check via
    // `_guard`'s RAII drop.
    let _guard = if opts.force_refresh {
        match lock::acquire() {
            LockOutcome::Acquired(g) => Some(g),
            LockOutcome::AlreadyRunning {
                held_by_pid,
                age_seconds,
            } => {
                if crate::observability::telemetry_gate::is_enabled() {
                    tracing::debug!(
                        event = "maintenance.refresh_already_running",
                        held_by_pid = held_by_pid,
                        age_seconds = age_seconds,
                    );
                }
                if !background {
                    eprintln!(
                        "check-updates: another refresher (pid {held_by_pid}) is running; skipping."
                    );
                }
                return EXIT_OK;
            }
            LockOutcome::NoHome => {
                eprintln!("check-updates: cannot resolve home directory; skipping.");
                return EXIT_CONFIG_ERROR;
            }
            LockOutcome::IoError(e) => {
                // Non-fatal — proceed unguarded rather than block
                // the whole check on a lock-file permissions
                // hiccup. Surfaces via telemetry so operators can
                // spot lock-dir corruption.
                if crate::observability::telemetry_gate::is_enabled() {
                    tracing::warn!(
                        event = "maintenance.cache_write_failed",
                        error_kind = "io",
                        stage = "lock_acquire",
                        error = %e,
                    );
                }
                None
            }
        }
    } else {
        None
    };

    let mode_label = if background {
        "cli_background"
    } else {
        "cli_foreground"
    };
    let started = std::time::Instant::now();
    if crate::observability::telemetry_gate::is_enabled() {
        tracing::info!(
            event = "maintenance.phase_started",
            mode = mode_label,
            tool_count = targets.len(),
            cache_ttl_hours = maintenance.cache_ttl_hours,
        );
    }

    // Foreground `--refresh` streams a `Checking <tool>...` line
    // per backend probe so the caller sees progress instead of
    // silence during the (up to 5s × N) network fan-out. Cache
    // hits never trigger the reporter, so quiet setups stay quiet.
    let progress_fn = |tool: &str, backend: &str| {
        eprintln!("Checking {tool} ({backend})...");
    };
    let reporter: checker::ProgressReporter<'_> = if background {
        None
    } else if refresh {
        Some(&progress_fn)
    } else {
        None
    };
    let report =
        checker::run_check_with_progress(targets, &mut cache, ttl, &opts, unchecked, reporter);

    if let Err(e) = cache::save(&cache)
        && crate::observability::telemetry_gate::is_enabled()
    {
        tracing::warn!(
            event = "maintenance.cache_write_failed",
            error_kind = classify_cache_error(&e),
        );
    }

    if crate::observability::telemetry_gate::is_enabled() {
        for check in &report.updates {
            tracing::info!(
                event = "maintenance.stale_tool",
                tool = %check.tool,
                backend = %check.backend,
                installed = %check.installed.as_deref().unwrap_or(""),
                latest = %check.latest.as_deref().unwrap_or(""),
                direction = ?check.direction,
            );
        }
        tracing::info!(
            event = "maintenance.phase_completed",
            mode = mode_label,
            tools_checked = report.summary.tools_checked,
            updates_available = report.summary.updates_available,
            unchecked = report.summary.unchecked,
            errors = report.summary.errors,
            duration_ms = started.elapsed().as_millis() as u64,
        );
    }
    crate::telemetry::maintenance_stale_tools(report.summary.updates_available as u64, None);

    if background {
        return EXIT_OK;
    }

    match output_format {
        "json" => match serde_json::to_string_pretty(&report) {
            Ok(text) => println!("{text}"),
            Err(e) => {
                eprintln!("check-updates: JSON serialize failed: {e}");
                return EXIT_CONFIG_ERROR;
            }
        },
        _ => {
            print!(
                "{}",
                reporter::human_with_options(&report, include_unchecked)
            );
        }
    }

    if report.summary.tools_checked == 0 && !report.errors.is_empty() {
        return EXIT_BACKEND_UNAVAILABLE;
    }
    if report.summary.updates_available > 0 {
        EXIT_UPDATES_AVAILABLE
    } else {
        EXIT_OK
    }
}

fn trust_gate_refuse(cfg: &MaintenanceConfig) -> Option<&'static str> {
    match maintenance::should_run(cfg) {
        Ok(()) => None,
        Err(maintenance::SkipReason::DisabledByConfig) => {
            Some("[maintenance] check_updates = false")
        }
        Err(maintenance::SkipReason::DisabledByEnv) => Some("JARVY_CHECK_UPDATES=0 set"),
        Err(maintenance::SkipReason::Sandbox) => Some("sandbox detected"),
        Err(maintenance::SkipReason::Ci) => Some("CI environment detected"),
        Err(_) => Some("phase skipped"),
    }
}

fn build_resolve_input(config: &Config) -> ResolveInput {
    let cargo_packages: Vec<String> = config
        .cargo
        .as_ref()
        .map(|c| c.packages.keys().cloned().collect())
        .unwrap_or_default();
    let cargo_installed = if cargo_packages.is_empty() {
        Default::default()
    } else {
        resolver::detect_cargo_installed()
    };
    ResolveInput {
        provisioner_tools: config.get_tool_configs().into_keys().collect(),
        cargo_packages,
        npm_packages: config
            .npm
            .as_ref()
            .map(|c| c.packages.keys().cloned().collect())
            .unwrap_or_default(),
        pip_packages: config
            .pip
            .as_ref()
            .map(|c| c.packages.keys().cloned().collect())
            .unwrap_or_default(),
        gem_packages: config
            .gem
            .as_ref()
            .map(|c| c.packages.keys().cloned().collect())
            .unwrap_or_default(),
        go_packages: config
            .go
            .as_ref()
            .map(|c| c.packages.keys().cloned().collect())
            .unwrap_or_default(),
        nuget_packages: config
            .nuget
            .as_ref()
            .map(|c| c.packages.keys().cloned().collect())
            .unwrap_or_default(),
        cargo_installed,
    }
}

fn split_csv(v: Option<&str>) -> Vec<String> {
    v.map(|s| {
        s.split(',')
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .collect()
    })
    .unwrap_or_default()
}

fn classify_cache_error(e: &cache::CacheError) -> &'static str {
    match e {
        cache::CacheError::NoHome => "no_home",
        cache::CacheError::Io(_) => "io",
        cache::CacheError::Json(_) => "json",
    }
}

// Unused BackendError re-export keeps the module surface aligned
// with the taxonomy — new backends land here first, then get
// consumed by the resolver.
#[allow(dead_code)]
pub(crate) type _BackendErrorAlias = BackendError;
