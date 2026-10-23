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
    /// Parse a strict boolean without changing the process environment.
    pub(crate) fn from_lookup(
        lookup: &impl Fn(&str) -> Result<Option<String>, String>,
    ) -> Result<Self, String> {
        let enabled = match lookup("SWAGGER_ENABLED")?.as_deref() {
            None | Some("true") => true,
            Some("false") => false,
            Some(_) => return Err("SWAGGER_ENABLED must be true or false".into()),
        };
        Ok(Self { enabled })
    }
}
