//! API documentation settings; enable Swagger by default and allow disabling it.

/// Documentation exposure policy.
#[derive(Clone, Debug)]
pub struct DocsConfig {
    /// Expose /docs and /docs/openapi.json when true.
    pub enabled: bool,
}

impl Default for DocsConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

impl DocsConfig {
    /// Read documentation exposure from the ordinary configuration snapshot.
    pub(crate) fn from_config(file: &super::ConfigFile) -> Result<Self, super::ConfigError> {
        Ok(Self { enabled: file.get_or("docs.enabled", true)? })
    }
}
