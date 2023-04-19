//! Setup logging subsystem.

use std::{collections::HashSet, time::Duration};

use opentelemetry::{
    global, runtime,
    sdk::{
        export::metrics::aggregation::cumulative_temporality_selector,
        metrics::{controllers::BasicController, selectors::simple},
        propagation::TraceContextPropagator,
        trace, Resource,
    },
    trace::TraceError,
    KeyValue,
};
use opentelemetry_otlp::{TonicExporterBuilder, WithExportConfig};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, prelude::*, util::SubscriberInitExt, EnvFilter, Layer};

use crate::{config, FormattingLayer, StorageSubscription};

/// Contains guards necessary for logging and metrics collection.
#[derive(Debug)]
pub struct TelemetryGuard {
    _log_guards: Vec<WorkerGuard>,
    _metrics_controller: Option<BasicController>,
}

/// Setup logging sub-system specifying the logging configuration, service (binary) name, and a
/// list of external crates for which a more verbose logging must be enabled. All crates within the
/// current cargo workspace are automatically considered for verbose logging.
pub fn setup(
    config: &config::Log,
    service_name: &'static str,
    crates_to_filter: impl AsRef<[&'static str]>,
) -> Result<TelemetryGuard, opentelemetry::metrics::MetricsError> {
    let mut guards = Vec::new();

    // Setup OpenTelemetry traces and metrics
    let (telemetry_tracer, _metrics_controller) = if config.telemetry.enabled {
        global::set_text_map_propagator(TraceContextPropagator::new());
        (
            setup_tracing_pipeline(&config.telemetry, service_name),
            setup_metrics_pipeline(&config.telemetry),
        )
    } else {
        (None, None)
    };
    let telemetry_layer = match telemetry_tracer {
        Some(Ok(ref tracer)) => Some(tracing_opentelemetry::layer().with_tracer(tracer.clone())),
        _ => None,
    };

    // Setup file logging
    let file_writer = if config.file.enabled {
        let mut path = crate::env::workspace_path();
        // Using an absolute path for file log path would replace workspace path with absolute path,
        // which is the intended behavior for us.
        path.push(&config.file.path);

        let file_appender = tracing_appender::rolling::hourly(&path, &config.file.file_name);
        let (file_writer, guard) = tracing_appender::non_blocking(file_appender);
        guards.push(guard);

        let file_filter = get_envfilter(
            config.file.filtering_directive.as_ref(),
            config::Level(tracing::Level::WARN),
            config.file.level,
            &crates_to_filter,
        );

        Some(FormattingLayer::new(service_name, file_writer).with_filter(file_filter))
    } else {
        None
    };

    let subscriber = tracing_subscriber::registry()
        .with(telemetry_layer)
        .with(StorageSubscription)
        .with(file_writer);

    // Setup console logging
    if config.console.enabled {
        let (console_writer, guard) = tracing_appender::non_blocking(std::io::stdout());
        guards.push(guard);

        let console_filter = get_envfilter(
            config.console.filtering_directive.as_ref(),
            config::Level(tracing::Level::WARN),
            config.console.level,
            &crates_to_filter,
        );

        match config.console.log_format {
            config::LogFormat::Default => {
                let logging_layer = fmt::layer()
                    .with_timer(fmt::time::time())
                    .pretty()
                    .with_writer(console_writer)
                    .with_filter(console_filter);
                subscriber.with(logging_layer).init();
            }
            config::LogFormat::Json => {
                let logging_layer =
                    FormattingLayer::new(service_name, console_writer).with_filter(console_filter);
                subscriber.with(logging_layer).init();
            }
        }
    } else {
        subscriber.init();
    };

    if let Some(Err(err)) = telemetry_tracer {
        tracing::error!("Failed to create an opentelemetry_otlp tracer: {err}");
        eprintln!("Failed to create an opentelemetry_otlp tracer: {err}");
    }

    // Returning the TelemetryGuard for logs to be printed and metrics to be collected until it is
    // dropped
    Ok(TelemetryGuard {
        _log_guards: guards,
        _metrics_controller,
    })
}

fn get_opentelemetry_exporter(config: &config::LogTelemetry) -> TonicExporterBuilder {
    let mut exporter_builder = opentelemetry_otlp::new_exporter().tonic();

    if let Some(ref endpoint) = config.otel_exporter_otlp_endpoint {
        exporter_builder = exporter_builder.with_endpoint(endpoint);
    }
    if let Some(timeout) = config.otel_exporter_otlp_timeout {
        exporter_builder = exporter_builder.with_timeout(Duration::from_millis(timeout));
    }

    exporter_builder
}

fn setup_tracing_pipeline(
    config: &config::LogTelemetry,
    service_name: &'static str,
) -> Option<Result<trace::Tracer, TraceError>> {
    let trace_config = trace::config()
        .with_sampler(trace::Sampler::TraceIdRatioBased(
            config.sampling_rate.unwrap_or(1.0),
        ))
        .with_resource(Resource::new(vec![KeyValue::new(
            "service.name",
            service_name,
        )]));

    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(get_opentelemetry_exporter(config))
        .with_trace_config(trace_config)
        .install_simple();

    Some(tracer)
}

fn setup_metrics_pipeline(config: &config::LogTelemetry) -> Option<BasicController> {
    let histogram_buckets = {
        let mut init = 0.01;
        let mut buckets: [f64; 15] = [0.0; 15];

        for bucket in &mut buckets {
            init *= 2.0;
            *bucket = init;
        }
        buckets
    };

    opentelemetry_otlp::new_pipeline()
        .metrics(
            simple::histogram(histogram_buckets),
            cumulative_temporality_selector(),
            // This would have to be updated if a different web framework is used
            runtime::TokioCurrentThread,
        )
        .with_exporter(get_opentelemetry_exporter(config))
        .with_period(Duration::from_secs(3))
        .with_timeout(Duration::from_secs(10))
        .build()
        .map_err(|err| eprintln!("Failed to setup metrics pipeline: {err:?}"))
        .ok()
}

fn get_envfilter(
    filtering_directive: Option<&String>,
    default_log_level: config::Level,
    filter_log_level: config::Level,
    crates_to_filter: impl AsRef<[&'static str]>,
) -> EnvFilter {
    filtering_directive
        .map(|filter| {
            // Try to create target filter from specified filtering directive, if set

            // Safety: If user is overriding the default filtering directive, then we need to panic
            // for invalid directives.
            #[allow(clippy::expect_used)]
            EnvFilter::builder()
                .with_default_directive(default_log_level.into_level().into())
                .parse(filter)
                .expect("Invalid EnvFilter filtering directive")
        })
        .unwrap_or_else(|| {
            // Construct a default target filter otherwise
            let mut workspace_members = std::env!("CARGO_WORKSPACE_MEMBERS")
                .split(',')
                .collect::<HashSet<_>>();
            workspace_members.extend(crates_to_filter.as_ref());

            workspace_members
                .drain()
                .zip(std::iter::repeat(filter_log_level.into_level()))
                .fold(
                    EnvFilter::default().add_directive(default_log_level.into_level().into()),
                    |env_filter, (target, level)| {
                        // Safety: This is a hardcoded basic filtering directive. If even the basic
                        // filter is wrong, it's better to panic.
                        #[allow(clippy::expect_used)]
                        env_filter.add_directive(
                            format!("{target}={level}")
                                .parse()
                                .expect("Invalid EnvFilter directive format"),
                        )
                    },
                )
        })
}
