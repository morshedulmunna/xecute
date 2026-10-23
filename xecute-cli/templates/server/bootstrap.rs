//! Server startup, socket peer information, and bounded graceful request draining.

use super::{logging, shutdown};
use crate::{app_module, configs::Config};
use std::{future::IntoFuture, sync::Arc};
use tokio::sync::Notify;

/// Start the configured listener and stop accepting work on a termination signal.
/// Active requests drain until the configured grace period expires.
pub async fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Load environment values before initializing logging or reading server settings.
    crate::configs::load_env()?;
    logging::init()?;
    let config = Config::from_env()?;

    // Resolve application dependencies and apply the shared HTTP policies.
    let app = app_module::AppModule::router()?.merge(super::docs::router(
        &config,
        app_module::AppModule::api_operations(),
    )?);
    let app = super::router(app, &config);

    // Bind before announcing readiness; report the actual address assigned by the OS.
    let listener = tokio::net::TcpListener::bind(config.bind_address).await?;
    tracing::info!(address = %listener.local_addr()?, "server listening");

    // Notify the serving task when shutdown begins so it can enforce a drain deadline.
    let draining = Arc::new(Notify::new());
    let shutdown_started = draining.clone();

    // Attach the socket peer address to requests for per-IP rate limiting.
    let server = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        shutdown::signal().await;

        // Axum stops accepting connections once this shutdown future completes.
        tracing::info!("shutdown requested; draining requests");
        shutdown_started.notify_one();
    })
    .into_future();

    // Keep one serving future alive while awaiting either completion or shutdown.
    tokio::pin!(server);

    tokio::select! {
        result = &mut server => result?,
        _ = draining.notified() => {
            // Allow in-flight requests to finish, but never wait beyond the grace period.
            match tokio::time::timeout(config.shutdown_grace, &mut server).await {
                Ok(result) => result?,
                Err(error) => {
                    tracing::error!("shutdown drain deadline exceeded");
                    return Err(error.into());
                }
            }
        }
    }

    tracing::info!("server stopped");

    Ok(())
}
