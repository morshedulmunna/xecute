//! Application entrypoint; the server module owns startup and shutdown.

mod app_module;
mod configs;
mod server;

#[cfg(test)]
mod tests;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    server::run().await
}
