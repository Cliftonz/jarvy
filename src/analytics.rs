// Telemetry OTLP endpoints are hardcoded at compile time for this CLI.
// Build-time env (set when running `cargo build`) can override the defaults:
// - Traces: JARVY_OTLP_TRACES_ENDPOINT (preferred) or JARVY_OTLP_ENDPOINT
// - Logs:   JARVY_OTLP_LOGS_ENDPOINT (preferred) or JARVY_OTLP_ENDPOINT
// If neither is set at build time, we default to the local Alloy instance
// running on port 4318 (HTTP/protobuf):
//   traces -> http://localhost:4318/v1/traces
//   logs   -> http://localhost:4318/v1/logs

use tracing_subscriber::Layer;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::Registry;

use opentelemetry_sdk::trace::SdkTracerProvider;

pub fn init_logging(enable_analytics: bool) {
    if enable_analytics {
        // Traces: always use OTLP exporter configured at compile time.
        let tracer_provider = build_otlp_tracer_provider();
        opentelemetry::global::set_tracer_provider(tracer_provider);

        // Logs: configure OTLP logs exporter and bridge tracing events to OTEL Logs.
        let logger_provider = build_otlp_logger_provider();
        let otel_logs_layer =
            opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new(
                &logger_provider,
            )
            .with_filter(LevelFilter::ERROR); // export only errors to OTEL

        // Layers:
        // - telemetry (traces to OTEL)
        // - fmt (human-readable stdout for all levels)
        // - otel_logs_layer (errors only to OTEL logs)
        let telemetry = tracing_opentelemetry::layer();
        let fmt_layer = tracing_subscriber::fmt::layer();
        let subscriber = Registry::default()
            .with(telemetry)
            .with(fmt_layer)
            .with(otel_logs_layer);
        tracing::subscriber::set_global_default(subscriber)
            .expect("setting tracing default failed");
    } else {
        // Analytics disabled: stdout logs only.
        tracing_subscriber::fmt().init();
    }
}

fn compile_time_otlp_traces_endpoint() -> &'static str {
    option_env!("JARVY_OTLP_TRACES_ENDPOINT")
        .or(option_env!("JARVY_OTLP_ENDPOINT"))
        .unwrap_or("http://localhost:4318/v1/traces")
}

fn compile_time_otlp_logs_endpoint() -> &'static str {
    option_env!("JARVY_OTLP_LOGS_ENDPOINT")
        .or(option_env!("JARVY_OTLP_ENDPOINT"))
        .unwrap_or("http://localhost:4318/v1/logs")
}

fn build_otlp_tracer_provider() -> SdkTracerProvider {
    use opentelemetry_otlp::{Protocol, WithExportConfig};

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary)
        .with_endpoint(compile_time_otlp_traces_endpoint())
        .build()
        .expect("failed to build OTLP span exporter");

    SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .build()
}

fn build_otlp_logger_provider() -> opentelemetry_sdk::logs::SdkLoggerProvider {
    use opentelemetry_otlp::{Protocol, WithExportConfig};

    let exporter = opentelemetry_otlp::LogExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary)
        .with_endpoint(compile_time_otlp_logs_endpoint())
        .build()
        .expect("failed to build OTLP log exporter");

    opentelemetry_sdk::logs::SdkLoggerProvider::builder()
        .with_batch_exporter(exporter)
        .build()
}
