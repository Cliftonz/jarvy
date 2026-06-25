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

/// Initialize the global tracing subscriber + OTLP logger provider.
///
/// `cfg` is the fully-merged effective telemetry config (env > project >
/// global file). Both the master switch (`cfg.enabled` && `cfg.logs`)
/// and the endpoint must come from this single source — earlier
/// versions gated the OTLP layer on the global file flag while
/// `telemetry::init` gated metrics/traces on the merged config,
/// producing the bug where `JARVY_TELEMETRY=1` env-only override left
/// the logger layer permanently disabled.
pub fn init_logging(cfg: &crate::telemetry::TelemetryConfig) {
    let enable_analytics = cfg.enabled && cfg.logs;
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
        match build_otlp_logger_provider(cfg) {
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

/// Resolve the per-signal OTLP/HTTP endpoint.
///
/// Precedence: `JARVY_OTLP_{SIGNAL}_ENDPOINT` env (full URL, used verbatim) >
/// `JARVY_OTLP_ENDPOINT` env (base) > caller-supplied base (from
/// `TelemetryConfig.endpoint`, which is itself sourced from the file
/// config or env). When the resolved value is a base URL (no `/v1/`
/// path segment), the signal path is appended — `opentelemetry-otlp`
/// 0.31's `with_endpoint()` is treated as the full URL and does NOT
/// auto-append, so a bare base produces a `POST /` and the collector
/// 404s every batch.
pub(crate) fn resolve_otlp_endpoint(base: &str, signal: &str) -> String {
    let signal_env = match signal {
        "logs" => "JARVY_OTLP_LOGS_ENDPOINT",
        "metrics" => "JARVY_OTLP_METRICS_ENDPOINT",
        "traces" => "JARVY_OTLP_TRACES_ENDPOINT",
        _ => "",
    };
    let candidate = env::var(signal_env)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .or_else(|| {
            env::var("JARVY_OTLP_ENDPOINT")
                .ok()
                .filter(|v| !v.trim().is_empty())
        })
        .unwrap_or_else(|| base.to_string());
    append_signal_path(&candidate, signal)
}

/// Append `/v1/{signal}` to `endpoint` unless it already terminates in
/// an OTLP signal path. Keeps the helper idempotent so an operator who
/// explicitly sets `JARVY_OTLP_LOGS_ENDPOINT=https://host/v1/logs`
/// doesn't get a double-pathed `https://host/v1/logs/v1/logs`.
fn append_signal_path(endpoint: &str, signal: &str) -> String {
    let trimmed = endpoint.trim_end_matches('/');
    let suffix = format!("/v1/{}", signal);
    if trimmed.ends_with(&suffix) || trimmed.contains("/v1/") {
        trimmed.to_string()
    } else {
        format!("{}{}", trimmed, suffix)
    }
}

pub fn send_otlp_smoke_probe() {
    if env::var("JARVY_TELEMETRY_SMOKE").as_deref() != Ok("1") {
        return;
    }

    // Resolve the smoke target from the same env-var the OTEL exporter
    // honors so the test can pass a random port via
    // `JARVY_OTLP_ENDPOINT=http://127.0.0.1:<port>` instead of fighting
    // for the hardcoded 4318. Falls back to 4318 when no env is set.
    let (host, port) = env::var("JARVY_OTLP_LOGS_ENDPOINT")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .or_else(|| {
            env::var("JARVY_OTLP_ENDPOINT")
                .ok()
                .filter(|v| !v.trim().is_empty())
        })
        .and_then(|url| parse_host_port(&url))
        .unwrap_or_else(|| ("127.0.0.1".to_string(), 4318));

    // Best-effort: 3 retries with 100ms backoff to absorb test-harness
    // jitter (cold-start, GC pause, busy CI runner). Each attempt tries
    // IPv4 then IPv6.
    let req = b"POST /v1/logs HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
    for attempt in 0..3 {
        if let Ok(mut s) = std::net::TcpStream::connect((host.as_str(), port))
            && s.write_all(req).is_ok()
        {
            let _ = s.flush();
            return;
        }
        // Only try IPv6 if no explicit host was given (the default 4318 path).
        if host == "127.0.0.1"
            && let Ok(mut s) = std::net::TcpStream::connect(("::1", port))
            && s.write_all(req).is_ok()
        {
            let _ = s.flush();
            return;
        }
        if attempt < 2 {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
}

/// Extract `(host, port)` from an OTLP endpoint URL. Returns `None`
/// when the URL is malformed or the port isn't explicit — the smoke
/// path needs a precise port to dial and shouldn't guess.
fn parse_host_port(url: &str) -> Option<(String, u16)> {
    // Strip scheme.
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    // Strip path (`/v1/logs`, etc.).
    let host_port = rest.split('/').next().unwrap_or(rest);
    // Split host:port.
    let (host, port_str) = host_port.rsplit_once(':')?;
    let port = port_str.parse().ok()?;
    Some((host.to_string(), port))
}

/// Build the shared OTLP resource — emitted as `service.*`/`host.*`
/// **resource attributes** on every signal so the forwarder's
/// `transform/anonymize` (which only operates on `context: resource`)
/// applies. Identical Resource is reused by both LoggerProvider here
/// and MeterProvider in `telemetry.rs` so logs and metrics carry
/// matching resource identity in Grafana.
///
/// `host.name` is emitted RAW. The forwarder hashes it with the rotating
/// salt before egress — local-only sinks (file logger, stderr) keep the
/// plaintext for the operator's own debugging. Emitting hashed-locally
/// would gain nothing (the forwarder already controls fan-out) and
/// would break `jarvy logs view` for the user.
pub(crate) fn build_resource() -> opentelemetry_sdk::Resource {
    use opentelemetry::KeyValue;

    let hostname = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    opentelemetry_sdk::Resource::builder()
        .with_attributes(vec![
            KeyValue::new("service.name", "jarvy"),
            KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
            KeyValue::new("host.name", hostname),
            KeyValue::new("os.type", env::consts::OS),
            KeyValue::new("os.description", env::consts::OS),
        ])
        .build()
}

fn build_otlp_logger_provider(
    cfg: &crate::telemetry::TelemetryConfig,
) -> Result<opentelemetry_sdk::logs::SdkLoggerProvider, Box<dyn std::error::Error>> {
    use opentelemetry_otlp::{Protocol, WithExportConfig};

    let endpoint = resolve_otlp_endpoint(&cfg.endpoint, "logs");
    let exporter = opentelemetry_otlp::LogExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary)
        .with_endpoint(endpoint.as_str())
        .build()?;

    let mut logger_builder =
        opentelemetry_sdk::logs::SdkLoggerProvider::builder().with_resource(build_resource());
    if env::var("JARVY_TELEMETRY_SMOKE").as_deref() == Ok("1") {
        logger_builder = logger_builder.with_simple_exporter(exporter);
    } else {
        logger_builder = logger_builder.with_batch_exporter(exporter);
    }
    Ok(logger_builder.build())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_host_port_handles_canonical_otlp_url() {
        assert_eq!(
            parse_host_port("http://127.0.0.1:4318"),
            Some(("127.0.0.1".to_string(), 4318))
        );
        assert_eq!(
            parse_host_port("https://otel.corp:443"),
            Some(("otel.corp".to_string(), 443))
        );
    }

    #[test]
    fn parse_host_port_strips_signal_path() {
        assert_eq!(
            parse_host_port("http://127.0.0.1:4318/v1/logs"),
            Some(("127.0.0.1".to_string(), 4318))
        );
    }

    #[test]
    fn parse_host_port_handles_bare_host_port() {
        // No scheme prefix — still parse.
        assert_eq!(
            parse_host_port("127.0.0.1:9999"),
            Some(("127.0.0.1".to_string(), 9999))
        );
    }

    #[test]
    fn parse_host_port_returns_none_when_port_missing() {
        assert_eq!(parse_host_port("http://127.0.0.1"), None);
        assert_eq!(parse_host_port("not a url"), None);
    }

    #[test]
    fn parse_host_port_returns_none_when_port_not_numeric() {
        assert_eq!(parse_host_port("http://127.0.0.1:abc"), None);
    }
}
