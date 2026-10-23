//! Explicit URL version groups with independently validated OpenAPI documents.

use crate::{ApiOperation, DocumentationError, openapi_document};
use axum::Router;
use serde_json::Value;
use std::collections::BTreeMap;

/// Invalid version names, duplicate groups, or conflicting API contracts.
#[derive(Debug)]
pub struct VersionError(String);
impl std::fmt::Display for VersionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for VersionError {}

/// One explicit version and its independently assembled application routes.
#[derive(Clone)]
pub struct ApiVersion {
    name: String,
    router: Router,
    operations: Vec<ApiOperation>,
}
impl ApiVersion {
    /// Group routes under a canonical v1/v2/etc prefix. Metadata paths are relative
    /// to this group and are prefixed exactly once in its OpenAPI document.
    pub fn new(
        name: &str,
        router: Router,
        mut operations: Vec<ApiOperation>,
    ) -> Result<Self, VersionError> {
        let number = name
            .strip_prefix('v')
            .and_then(|value| value.parse::<u32>().ok());
        if !number.is_some_and(|value| value > 0 && name == format!("v{value}")) {
            return Err(VersionError(
                "API version must be canonical v1, v2, etc".into(),
            ));
        }
        for operation in &mut operations {
            if !operation.path.starts_with('/') {
                return Err(VersionError(
                    "version operation paths must start with /".into(),
                ));
            }
            operation.path = format!("/{name}{}", operation.path);
        }
        // Validate even when Swagger is disabled so conflicting contracts fail startup.
        openapi_document("API", name, operations.clone())
            .map_err(|error| VersionError(error.to_string()))?;
        Ok(Self {
            name: name.into(),
            router,
            operations,
        })
    }
    /// Return the canonical version label without a slash.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Build this version's OpenAPI document with fully prefixed paths.
    pub fn document(&self, title: &str) -> Result<Value, DocumentationError> {
        openapi_document(title, &self.name, self.operations.clone())
    }
    /// Return prefixed route metadata for inspection or aggregation.
    pub fn operations(&self) -> &[ApiOperation] {
        &self.operations
    }
}

/// Explicit collection of version groups; no version implicitly includes another.
#[derive(Clone, Default)]
pub struct ApiVersions {
    versions: Vec<ApiVersion>,
}
impl ApiVersions {
    /// Start an empty version collection.
    pub fn new() -> Self {
        Self::default()
    }
    /// Add a group, rejecting duplicate version labels before router assembly.
    pub fn with(mut self, version: ApiVersion) -> Result<Self, VersionError> {
        if self
            .versions
            .iter()
            .any(|existing| existing.name == version.name)
        {
            return Err(VersionError("duplicate API version".into()));
        }
        self.versions.push(version);
        Ok(self)
    }
    /// Inspect registered groups, for example to mount version-specific Swagger routes.
    pub fn versions(&self) -> &[ApiVersion] {
        &self.versions
    }
    /// Mount all groups on an otherwise unversioned router. Add global middleware afterwards.
    pub fn mount(&self, mut router: Router) -> Router {
        for version in &self.versions {
            router = router.nest(&format!("/{}", version.name), version.router.clone());
        }
        router
    }
    /// Build independent documents; operation IDs and schema names are scoped to each version.
    pub fn documents(&self, title: &str) -> Result<BTreeMap<String, Value>, DocumentationError> {
        self.versions
            .iter()
            .map(|version| Ok((version.name.clone(), version.document(title)?)))
            .collect()
    }
}
