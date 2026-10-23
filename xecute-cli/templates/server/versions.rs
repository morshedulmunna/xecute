//! Explicit version registration. Keep route assembly and documentation together.

use axum::Router;
use xecute::{ApiOperation, ApiVersion, ApiVersions, VersionError};
use crate::configs::Config;

/// Register v1 resources; unversioned aliases are separate.
pub fn groups(config: &Config, v1: Router, operations: Vec<ApiOperation>) -> Result<ApiVersions, VersionError> {
    if !config.versioning_enabled { return Ok(ApiVersions::new()); }
    ApiVersions::new()
        .with(ApiVersion::new("v1", v1, operations)?)
}
