//! Server startup, socket peer information, and bounded graceful request draining.

use super::{logging, shutdown};
use crate::{app_module, configs::Config};
use std::{future::IntoFuture, sync::Arc};
use tokio::sync::Notify;

/// Start the configured listener and stop accepting work on a termination signal.
/// Active requests drain until the configured grace period expires.
pub async fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Load environment values for secrets; ordinary settings come from config/AppConfig.toml.
    crate::configs::load_env()?;
    let config = Config::from_file()?;
    super::static_files::validate(&config.static_files)?;
    let _logging = logging::init(&config)?;

    // Resolve application dependencies and apply the shared HTTP policies.
    let container = Arc::new(app_module::AppModule::container()?);
    let app = app_module::AppModule::router_with_container(container)?;
    let operations = app_module::AppModule::api_operations();
    let versions = super::versions::groups(&config, app.clone(), operations.clone())?;
    let app = versions.mount(app)
        .merge(super::docs::router(&config, operations)?)
        .merge(super::docs::versioned_router(&config, &versions)?);

    // Bind before announcing readiness; report the actual address assigned by the OS.
    let listener = tokio::net::TcpListener::bind(config.bind_address).await?;
    let address = listener.local_addr()?;
    let mut config = config;
    config.bind_address = address;
    let app = super::router(app, &config);
    let url = local_url(address);
    tracing::info!(address = %address, url = %url, "server listening");
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

// Wildcard listener addresses are not browser destinations. Keep the bound port,
// including an OS-assigned port when server.bind_address uses port zero.
pub(super) fn local_url(mut address: std::net::SocketAddr) -> String {
    if address.ip().is_unspecified() {
        address.set_ip(if address.is_ipv4() {
            std::net::Ipv4Addr::LOCALHOST.into()
        } else {
            std::net::Ipv6Addr::LOCALHOST.into()
        });
    }
    format!("http://{address}")
}

#[cfg(test)]
mod tests {
    #[test]
    fn startup_urls_use_accessible_hosts_and_preserve_ports() {
        for (address, expected) in [
            ("0.0.0.0:3000", "http://127.0.0.1:3000"),
            ("127.0.0.1:4100", "http://127.0.0.1:4100"),
            ("[::]:3000", "http://[::1]:3000"),
            ("[::1]:4100", "http://[::1]:4100"),
        ] {
            assert_eq!(super::local_url(address.parse().unwrap()), expected);
        }
    }
}
