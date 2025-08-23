use tracing_subscriber::Registry;
use tracing_subscriber::layer::SubscriberExt;

use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_stdout as stdout;
use tracing::{error, span};

pub fn init_logging(enable_analytics: bool) {
    if enable_analytics {
        // Set up a basic stdout tracer provider for spans
        let provider = SdkTracerProvider::builder()
            .with_simple_exporter(stdout::SpanExporter::default())
            .build();
        let tracer = provider.tracer("jarvy");

        let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

        let subscriber = Registry::default().with(telemetry);
        tracing::subscriber::set_global_default(subscriber)
            .expect("setting tracing default failed");
    } else {
        tracing_subscriber::fmt().init();
    }
}
