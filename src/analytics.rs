
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::Registry;

use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::trace::TracerProvider;
use opentelemetry_stdout as stdout;
use tracing::{error, span};

pub fn init_logging(enable_analytics: bool) {
    if enable_analytics {

        let tracer = opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(stdout::SpanExporter::default())
            .install_simple()
            .unwrap();

        let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

        let subscriber = Registry::default().with(telemetry);
        tracing::subscriber::set_global_default(subscriber).expect("setting tracing default failed");
    } else {
        tracing_subscriber::fmt().init();
    }
}