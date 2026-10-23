//! Stored resource and response representation.

use schemars::JsonSchema;
use serde::Serialize;

/// Resource returned by collection and creation endpoints.
#[derive(Clone, Serialize, JsonSchema)]
pub struct __TYPE__ {
    /// Server-generated unique identifier.
    pub id: String,
    /// Validated display name.
    pub name: String,
}
