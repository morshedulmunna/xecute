//! Flat structured JSON logging; request completion events carry their own context.

use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt};

/// Build a subscriber with an injectable writer for stdout or isolated test capture.
pub(crate) fn subscriber<W>(filter: EnvFilter, writer: W) -> impl tracing::Subscriber + Send + Sync
where
    W: for<'a> fmt::MakeWriter<'a> + Send + Sync + 'static,
{
    fmt()
        .with_env_filter(filter)
        .json()
        .flatten_event(true)
        .with_current_span(false)
        .with_span_list(false)
        .with_target(false)
        .with_writer(writer)
        .finish()
}

/// Install the process logger once and redact panic payloads from the panic hook.
pub fn init() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = crate::configs::LoggingConfig::from_env()?;
    subscriber(config.filter, std::io::stdout).try_init()?;
    // Panic payloads may contain secrets; the HTTP boundary returns a generic 500.
    std::panic::set_hook(Box::new(|_| tracing::error!("task panicked")));
    Ok(())
}
