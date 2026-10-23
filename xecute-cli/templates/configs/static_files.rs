//! Static asset settings for a dedicated public directory.

use std::path::PathBuf;

/// Static file exposure policy.
#[derive(Clone, Debug)]
pub struct StaticConfig {
    /// Enable public file serving.
    pub enabled: bool,
    /// Dedicated directory containing only public assets.
    pub directory: PathBuf,
    /// Non-root URL prefix for public assets.
    pub mount: String,
}
impl Default for StaticConfig {
    fn default() -> Self {
        Self { enabled: true, directory: "public".into(), mount: "/static".into() }
    }
}
