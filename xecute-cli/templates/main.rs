//! Application entrypoint; the server module owns startup and shutdown.

pub mod app_module;
pub mod configs;
mod middleware;
mod server;

#[cfg(test)]
mod tests;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    server::run().await
}
