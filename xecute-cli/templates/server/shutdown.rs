//! Portable termination handling: Ctrl-C and, on Unix, SIGTERM.

/// Resolve on Ctrl-C or SIGTERM; listener failures are logged before shutdown.
pub(super) async fn signal() {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::error!(%error, "failed to listen for Ctrl-C");
        }
    };
    #[cfg(unix)]
    {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut terminate) => {
                tokio::select! { _ = ctrl_c => {}, _ = terminate.recv() => {} }
            }
            Err(error) => {
                tracing::error!(%error, "failed to listen for SIGTERM");
                ctrl_c.await;
            }
        }
    }
    #[cfg(not(unix))]
    ctrl_c.await;
}
