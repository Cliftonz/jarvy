// Telemetry OTLP endpoints are hardcoded at compile time for this CLI.
// Build-time env (set when running `cargo build`) can override the defaults:
// - Logs:   JARVY_OTLP_LOGS_ENDPOINT (preferred) or JARVY_OTLP_ENDPOINT
// If neither is set at build time, we default to the local Alloy instance
// running on port 4318 (HTTP/protobuf). Note: opentelemetry_otlp expects a base URL
// and will append the signal path (e.g., /v1/logs) automatically.
//   base   -> http://localhost:4318

use std::env;
use std::io::Write;
use tracing::Level;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::Layer;
use tracing_subscriber::filter::{FilterFn, LevelFilter};
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::Registry;

/// Records the runtime state of OTEL telemetry initialization. Read by
/// `jarvy telemetry status` so users can see whether their OTEL endpoint
/// actually came up — previously a misconfigured endpoint produced one
/// stderr line at startup with no further signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelemetryBootstrapState {
    /// OTLP exporter active.
    Healthy,
    /// User explicitly disabled telemetry.
    Disabled,
    /// User enabled telemetry but exporter setup failed.
    Degraded,
}

static BOOTSTRAP_STATE: std::sync::OnceLock<std::sync::RwLock<TelemetryBootstrapState>> =
    std::sync::OnceLock::new();

/// Owning handle to the SDK logger provider. Stashed at init so
/// `shutdown_logging()` can flush queued log records before
/// `std::process::exit` kills the worker thread.
static LOGGER_PROVIDER: std::sync::OnceLock<opentelemetry_sdk::logs::SdkLoggerProvider> =
    std::sync::OnceLock::new();

/// `WorkerGuard` returned by `tracing_appender::non_blocking`. Holding
/// this for the lifetime of the process keeps the background-thread
/// writer alive; dropping it flushes pending lines. We stash it in a
/// `Mutex<Option>` so `shutdown_logging()` can drop it deterministically
/// before `std::process::exit`.
static FILE_LOGGER_GUARD: std::sync::Mutex<Option<tracing_appender::non_blocking::WorkerGuard>> =
    std::sync::Mutex::new(None);

fn bootstrap_state_cell() -> &'static std::sync::RwLock<TelemetryBootstrapState> {
    BOOTSTRAP_STATE.get_or_init(|| std::sync::RwLock::new(TelemetryBootstrapState::Disabled))
}

pub fn telemetry_bootstrap_state() -> TelemetryBootstrapState {
    bootstrap_state_cell()
        .read()
        .map(|g| *g)
        .unwrap_or(TelemetryBootstrapState::Degraded)
}

fn set_bootstrap_state(state: TelemetryBootstrapState) {
    if let Ok(mut g) = bootstrap_state_cell().write() {
        *g = state;
    }
}

pub fn init_logging(enable_analytics: bool) {
    // Registry-level EnvFilter so dependency-crate `info!`/`debug!`
    // events from `dirs`, `ureq`, `opentelemetry_sdk`, `rustls`, etc.
    // don't flood `~/.jarvy/logs/jarvy.log` and OTLP exports
    // (round-2 obs P1). Operators get a `RUST_LOG` escape hatch; the
    // default is "warn at the global floor, info inside our own crate."
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn,jarvy=info"));

    // Console output goes to stderr at every level. Stdout is reserved
    // for command output (e.g. `jarvy tools --index --format json`,
    // `jarvy explain --format json`) so downstream consumers can pipe
    // a clean payload. A non-error tracing event arriving on stdout
    // ahead of the `println!` of the JSON body breaks the parser —
    // `scripts/gen-docs.sh` hit exactly this in CI envs where the
    // default `warn,jarvy=info` filter let registry-load `info!()`
    // events fire before the index emit.
    let stderr_non_error = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_filter(FilterFn::new(|meta| meta.level() < &Level::ERROR));

    let stderr_errors = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_filter(LevelFilter::ERROR);

    // File layer at ~/.jarvy/logs/jarvy.log (daily rotation, JSON).
    //
    // Previously the file layer existed in `observability::logging::init_*`
    // but those functions were never called from `main.rs` — so
    // `~/.jarvy/logs/jarvy.log` was always empty, `jarvy logs view` returned
    // nothing, and `jarvy ticket create` shipped a hollow log file to support.
    // Wire it into the same `Registry` as the console + OTLP layers so a
    // single `set_global_default` covers all sinks (observability review #1).
    let file_layer = match ensure_log_dir() {
        Ok(dir) => {
            let appender = RollingFileAppender::new(Rotation::DAILY, dir, "jarvy.log");
            // Wrap in non_blocking so per-event tracing writes are
            // coalesced through an mpsc + dedicated worker thread —
            // upstream tracing-appender's recommended pattern. The
            // returned WorkerGuard is stashed in FILE_LOGGER_GUARD;
            // dropping it (in shutdown_logging) flushes pending lines
            // before process exit.
            let (non_blocking, guard) = tracing_appender::non_blocking(appender);
            if let Ok(mut slot) = FILE_LOGGER_GUARD.lock() {
                *slot = Some(guard);
            }
            let layer = tracing_subscriber::fmt::layer()
                .json()
                .with_writer(non_blocking)
                .with_span_events(FmtSpan::CLOSE)
                .with_current_span(true)
                .with_target(true)
                .with_ansi(false)
                // Capture every level the user might care about during a
                // failed `jarvy setup`. Cheaper than rebuilding telemetry
                // from `eprintln!` after the fact.
                .with_filter(LevelFilter::INFO);
            Some(layer)
        }
        Err(e) => {
            eprintln!("Warning: file logging disabled — could not create log dir: {e}");
            None
        }
    };

    // Only if analytics enabled, export to OTLP logs.
    //
    // Filter is now driven by `JARVY_OTLP_LEVEL` (default `info`) instead of
    // the previous hard-coded `LevelFilter::ERROR`. The old filter dropped
    // every `info!`-level event — `tool.installed`, `setup.inventory`,
    // `hook.completed` — so `logs = true` in TelemetryConfig was a lie at
    // export time (observability review #2).
    let mut bootstrap_error: Option<String> = None;
    let otel_layer_opt = if enable_analytics {
        match build_otlp_logger_provider() {
            Ok(logger_provider) => {
                let layer = opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new(
                    &logger_provider,
                )
                .with_filter(otlp_level_filter());
                // Stash the provider so `shutdown_logging()` (called from
                // `main` before `std::process::exit`) can flush queued
                // batches. Without this, `process::exit` skips Drop and
                // the batch processor's worker thread is killed mid-flight,
                // silently truncating OTLP log exports (round-2 obs P0).
                let _ = LOGGER_PROVIDER.set(logger_provider);
                Some(layer)
            }
            Err(e) => {
                // No subscriber yet — eprintln! is the only channel until the
                // fallback subscriber is up. After that, we emit a structured
                // event so the degradation is visible in any downstream sink
                // and in `jarvy telemetry status`.
                eprintln!("Warning: failed to initialize OTLP telemetry: {e}");
                bootstrap_error = Some(e.to_string());
                None
            }
        }
    } else {
        None
    };

    let subscriber = Registry::default()
        .with(env_filter)
        .with(stderr_non_error)
        .with(stderr_errors)
        .with(file_layer)
        .with(otel_layer_opt);

    let install_result = tracing::subscriber::set_global_default(subscriber);
    if let Err(e) = install_result {
        eprintln!("Failed to set tracing default: {e}");
    }

    if !enable_analytics {
        set_bootstrap_state(TelemetryBootstrapState::Disabled);
    } else if let Some(reason) = bootstrap_error {
        set_bootstrap_state(TelemetryBootstrapState::Degraded);
        // Subscriber is now installed (without OTEL layer); this event reaches
        // the fallback console layer and any downstream consumer.
        tracing::error!(
            event = "telemetry.bootstrap.degraded",
            reason = %reason,
            "OTLP exporter failed to initialize; running with console logs only"
        );
    } else {
        set_bootstrap_state(TelemetryBootstrapState::Healthy);
    }
}

/// Flush any queued OTLP log records and tear down the logger provider.
/// MUST be called before `std::process::exit` — `process::exit` skips
/// every `Drop` impl, including the batch processor's worker-thread
/// shutdown, which means in-flight log batches are silently dropped
/// (round-2 obs P0).
///
/// Safe to call multiple times; safe to call when telemetry was never
/// initialized.
pub fn shutdown_logging() {
    if let Some(provider) = LOGGER_PROVIDER.get() {
        // `force_flush` waits for queued records to export. We
        // intentionally ignore `Err` here because there's nowhere
        // useful to report it — we're on the way out.
        let _ = provider.force_flush();
        let _ = provider.shutdown();
    }
    // Drop the file-logger WorkerGuard so the background-thread writer
    // flushes pending lines to ~/.jarvy/logs/jarvy.log before
    // `std::process::exit` kills it.
    if let Ok(mut slot) = FILE_LOGGER_GUARD.lock() {
        slot.take();
    }
}

/// Resolve the OTLP-bridge level filter. `JARVY_OTLP_LEVEL` (or the legacy
/// `JARVY_OTLP_LOGS` boolean) overrides the default `info`. Setting it to
/// `error` recovers the old behavior for users who explicitly want quiet
/// exports.
fn otlp_level_filter() -> LevelFilter {
    match env::var("JARVY_OTLP_LEVEL")
        .map(|v| v.trim().to_ascii_lowercase())
        .as_deref()
    {
        Ok("error") => LevelFilter::ERROR,
        Ok("warn") => LevelFilter::WARN,
        Ok("info") => LevelFilter::INFO,
        Ok("debug") => LevelFilter::DEBUG,
        Ok("trace") => LevelFilter::TRACE,
        _ => LevelFilter::INFO,
    }
}

/// Returns `~/.jarvy/logs`, creating it if necessary. Path resolution
/// goes through `crate::paths::logs_dir` so a future XDG migration or
/// `JARVY_HOME` override is honored without touching this site.
fn ensure_log_dir() -> std::io::Result<std::path::PathBuf> {
    let dir = crate::paths::logs_dir().map_err(|e| std::io::Error::other(e.to_string()))?;
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn otlp_logs_endpoint() -> String {
    if let Ok(v) = env::var("JARVY_OTLP_LOGS_ENDPOINT") {
        if !v.trim().is_empty() {
            return v;
        }
    }
    if let Ok(v) = env::var("JARVY_OTLP_ENDPOINT") {
        if !v.trim().is_empty() {
            return v;
        }
    }
    // Fallback to compile-time overrides or default (base URL; path is appended by exporter)
    option_env!("JARVY_OTLP_LOGS_ENDPOINT")
        .or(option_env!("JARVY_OTLP_ENDPOINT"))
        .unwrap_or("http://localhost:4318")
        .to_string()
}

pub fn send_otlp_smoke_probe() {
    if env::var("JARVY_TELEMETRY_SMOKE").as_deref() != Ok("1") {
        return;
    }
    // Best-effort: try IPv4 then IPv6. Ignore errors; this is just a smoke trigger.
    let req = b"POST /v1/logs HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
    // IPv4
    if let Ok(mut s) = std::net::TcpStream::connect(("127.0.0.1", 4318)) {
        let _ = s.write_all(req);
        let _ = s.flush();
        return;
    }
    // IPv6
    if let Ok(mut s) = std::net::TcpStream::connect(("::1", 4318)) {
        let _ = s.write_all(req);
        let _ = s.flush();
    }
}

fn build_otlp_logger_provider()
-> Result<opentelemetry_sdk::logs::SdkLoggerProvider, Box<dyn std::error::Error>> {
    use opentelemetry_otlp::{Protocol, WithExportConfig};

    let endpoint = otlp_logs_endpoint();
    let exporter = opentelemetry_otlp::LogExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary)
        .with_endpoint(endpoint.as_str())
        .build()?;

    let mut logger_builder = opentelemetry_sdk::logs::SdkLoggerProvider::builder();
    if env::var("JARVY_TELEMETRY_SMOKE").as_deref() == Ok("1") {
        logger_builder = logger_builder.with_simple_exporter(exporter);
    } else {
        logger_builder = logger_builder.with_batch_exporter(exporter);
    }
    Ok(logger_builder.build())
}
