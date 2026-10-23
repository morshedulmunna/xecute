//! Structured logging with independent console/file sinks and exporter composition.

use crate::configs::{Config, LoggingConfig};
use tracing_appender::{non_blocking::{NonBlocking, NonBlockingBuilder, WorkerGuard}, rolling::{RollingFileAppender, Rotation}};
use tracing_subscriber::{Layer, Registry, fmt, layer::SubscriberExt, util::SubscriberInitExt};
use tracing_subscriber::registry::LookupSpan;

type Error = Box<dyn std::error::Error + Send + Sync>;

/// Build a subscriber with an injectable writer for isolated log capture.
#[cfg(test)]
pub(crate) fn subscriber<W>(filter: tracing_subscriber::EnvFilter, writer: W) -> impl tracing::Subscriber + Send + Sync
where W: for<'a> fmt::MakeWriter<'a> + Send + Sync + 'static {
    tracing_subscriber::registry().with(filter).with(json_layer(writer))
}

fn json_layer<S, W>(writer: W) -> impl Layer<S> + Send + Sync
where S: tracing::Subscriber + for<'a> LookupSpan<'a>, W: for<'a> fmt::MakeWriter<'a> + Send + Sync + 'static {
    fmt::layer().json().flatten_event(true).with_current_span(false)
        .with_span_list(false).with_target(false).with_writer(writer)
}

/// Keeps the background file writer alive and flushes queued logs on normal shutdown.
pub struct LoggerGuard {
    _file: Option<WorkerGuard>,
}

/// Install standard console/file sinks. Retain the returned guard until shutdown.
pub fn init(config: &Config) -> Result<LoggerGuard, Error> {
    init_with_layer(config, tracing_subscriber::layer::Identity::new())
}

/// Add an exporter layer (for example Sentry or OpenTelemetry) to the same events.
/// Initialize the SDK separately and retain its own shutdown guard as required.
pub fn init_with_layer<L>(config: &Config, extra: L) -> Result<LoggerGuard, Error>
where L: Layer<Registry> + Send + Sync + 'static {
    let logging = LoggingConfig::from_env()?;
    validate_output_paths(config, &logging)?;
    let (file, guard) = if logging.file_enabled {
        let (writer, guard) = file_writer(&logging)?;
        (Some(writer), Some(guard))
    } else { (None, None) };
    tracing_subscriber::registry().with(extra).with(logging.filter)
        .with(logging.stdout.then(|| json_layer(std::io::stdout)))
        .with(file.map(json_layer)).try_init()?;
    std::panic::set_hook(Box::new(|_| tracing::error!("task panicked")));
    Ok(LoggerGuard { _file: guard })
}

/// Keep configured log storage outside any publicly served tree.
pub(crate) fn validate_output_paths(config: &Config, logging: &LoggingConfig) -> Result<(), Error> {
    if config.static_files.enabled {
        let public = crate::configs::logging::absolute_path(&config.static_files.directory)?;
        let logs = crate::configs::logging::absolute_path(&logging.directory)?;
        if logs.starts_with(&public) { return Err("LOG_FILE_DIR must not be inside the public static directory".into()); }
    }
    Ok(())
}

pub(crate) fn file_writer(config: &LoggingConfig) -> Result<(NonBlocking, WorkerGuard), Error> {
    let rotation = match config.rotation.as_str() { "daily" => Rotation::DAILY, "hourly" => Rotation::HOURLY, _ => Rotation::NEVER };
    let appender = RollingFileAppender::builder().rotation(rotation).filename_prefix(&config.filename)
        .filename_suffix("log").max_log_files(config.max_files).build(&config.directory)?;
    // Bounded queue with backpressure instead of silently dropping events.
    Ok(NonBlockingBuilder::default().buffered_lines_limit(8192).lossy(false).finish(appender))
}
