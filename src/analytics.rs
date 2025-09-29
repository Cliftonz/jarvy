// Telemetry OTLP endpoints are hardcoded at compile time for this CLI.
// Build-time env (set when running `cargo build`) can override the defaults:
// - Logs:   JARVY_OTLP_LOGS_ENDPOINT (preferred) or JARVY_OTLP_ENDPOINT
// If neither is set at build time, we default to the local Alloy instance
// running on port 4318 (HTTP/protobuf):
//   logs   -> http://localhost:4318/v1/logs

use std::env;
use tracing::field::Visit;
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::filter::{FilterFn, LevelFilter};
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::registry::Registry;

// Layer that forwards ERROR events to PostHog
struct PosthogErrorLayer;

struct EventVisitor {
    fields: Vec<(String, String)>,
}

impl Visit for EventVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.fields
            .push((field.name().to_string(), format!("{:?}", value)));
    }
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.fields
            .push((field.name().to_string(), value.to_string()));
    }
}

impl<S> Layer<S> for PosthogErrorLayer
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        if event.metadata().level() == &Level::ERROR {
            let mut visitor = EventVisitor { fields: Vec::new() };
            event.record(&mut visitor);
            let fields_for_msg = visitor.fields.clone();

            // Prefer the `message` field if present
            let mut message = None;
            for (k, v) in &visitor.fields {
                if k == "message" {
                    message = Some(v.clone());
                    break;
                }
            }
            let msg = message.unwrap_or_else(|| {
                // Fallback: join k=v pairs
                let parts: Vec<String> = fields_for_msg
                    .into_iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect();
                if parts.is_empty() {
                    "unknown error".to_string()
                } else {
                    parts.join(", ")
                }
            });

            // Build context from metadata and fields
            let meta = event.metadata();
            let mut ctx = serde_json::Map::new();
            ctx.insert(
                "level".to_string(),
                serde_json::Value::String(meta.level().to_string()),
            );
            ctx.insert(
                "target".to_string(),
                serde_json::Value::String(meta.target().to_string()),
            );
            if let Some(m) = meta.module_path() {
                ctx.insert(
                    "module".to_string(),
                    serde_json::Value::String(m.to_string()),
                );
            }
            if let Some(f) = meta.file() {
                ctx.insert("file".to_string(), serde_json::Value::String(f.to_string()));
            }
            if let Some(l) = meta.line() {
                ctx.insert("line".to_string(), serde_json::Value::from(l as u64));
            }
            let fields_obj: serde_json::Map<String, serde_json::Value> = visitor
                .fields
                .into_iter()
                .map(|(k, v)| (k, serde_json::Value::String(v)))
                .collect();
            if !fields_obj.is_empty() {
                ctx.insert("fields".to_string(), serde_json::Value::Object(fields_obj));
            }

            // Send to PostHog (no-op if client disabled)
            crate::posthog::capture_exception(&msg, "tracing_error", None, ctx);
        }
    }
}

pub fn init_logging(enable_analytics: bool) {
    // Always log to console: stdout for non-errors, stderr for errors
    let stdout_non_error = tracing_subscriber::fmt::layer()
        .with_filter(FilterFn::new(|meta| meta.level() < &Level::ERROR));

    let stderr_errors = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_filter(LevelFilter::ERROR);

    // Only if analytics enabled, export errors to OTLP logs
    let otel_layer_opt = if enable_analytics {
        let logger_provider = build_otlp_logger_provider();
        let layer = opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new(
            &logger_provider,
        )
        .with_filter(LevelFilter::ERROR); // export only errors to OTEL
        Some(layer)
    } else {
        None
    };

    let subscriber = Registry::default()
        .with(stdout_non_error)
        .with(stderr_errors)
        .with(PosthogErrorLayer)
        .with(otel_layer_opt);

    tracing::subscriber::set_global_default(subscriber).expect("setting tracing default failed");
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
    // Fallback to compile-time overrides or default
    option_env!("JARVY_OTLP_LOGS_ENDPOINT")
        .or(option_env!("JARVY_OTLP_ENDPOINT"))
        .unwrap_or("http://localhost:4318/v1/logs")
        .to_string()
}

fn build_otlp_logger_provider() -> opentelemetry_sdk::logs::SdkLoggerProvider {
    use opentelemetry_otlp::{Protocol, WithExportConfig};

    let endpoint = otlp_logs_endpoint();
    let exporter = opentelemetry_otlp::LogExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary)
        .with_endpoint(endpoint.as_str())
        .build()
        .expect("failed to build OTLP log exporter");

    let mut logger_builder = opentelemetry_sdk::logs::SdkLoggerProvider::builder();
    if env::var("JARVY_TELEMETRY_SMOKE").as_deref() == Ok("1") {
        logger_builder = logger_builder.with_simple_exporter(exporter);
    } else {
        logger_builder = logger_builder.with_batch_exporter(exporter);
    }
    logger_builder.build()
}
