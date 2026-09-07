use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::WithExportConfig;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

use crate::config::Config;

/// The tracer provider is built once at startup and kept alive for the process's lifetime by
/// returning it to `main` for storage there — a provider dropped early silently stops exporting
/// spans (`assets-web`'s `application/tracing.py` has a comment about the exact same failure
/// mode hitting catalog-api via a misplaced `defer`). Returns `None` when
/// `OTEL_EXPORTER_OTLP_ENDPOINT` is unset, so tracing is a no-op rather than a startup failure in
/// local dev. The Sentry client guard is returned alongside for the same reason - dropping it
/// early shuts down error reporting silently.
pub fn init(
    config: &Config,
) -> (
    Option<opentelemetry_sdk::trace::SdkTracerProvider>,
    Option<sentry::ClientInitGuard>,
) {
    let env_filter =
        EnvFilter::try_new(&config.log_level).unwrap_or_else(|_| EnvFilter::new("info"));

    let provider = config.otlp_endpoint.as_ref().map(|endpoint| {
        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_http()
            .with_endpoint(format!("{endpoint}/v1/traces"))
            .build()
            .expect("failed to build OTLP span exporter");

        opentelemetry_sdk::trace::SdkTracerProvider::builder()
            .with_batch_exporter(exporter)
            .build()
    });

    let otel_layer = provider.as_ref().map(|p| {
        let tracer = p.tracer("sweetrpg-game-systems-web");
        tracing_opentelemetry::layer().with_tracer(tracer)
    });

    // Without a propagator registered, opentelemetry::global::get_text_map_propagator defaults
    // to a no-op - admin_client.rs's outgoing traceparent injection would silently do nothing.
    // W3C TraceContext matches the Go services' otelgin extraction and the Swift frontends'
    // OTelW3CPropagator, so a trace started here links correctly into admin-api's own trace.
    opentelemetry::global::set_text_map_propagator(
        opentelemetry_sdk::propagation::TraceContextPropagator::new(),
    );

    // Guarded on SENTRY_DSN being set - unset means this stays uninitialized and the
    // sentry-tracing layer's captured events simply have nowhere to go, a silent no-op rather
    // than a startup failure (matches every other frontend's own guarded Sentry setup).
    let sentry_guard = config.sentry_dsn.as_ref().map(|dsn| {
        let mut options = sentry::ClientOptions::default();
        options.environment = Some(config.env.clone().into());
        options.release = sentry::release_name!();
        sentry::init((dsn.as_str(), options))
    });

    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer().json())
        .with(otel_layer)
        .with(sentry_tracing::layer())
        .init();

    (provider, sentry_guard)
}
