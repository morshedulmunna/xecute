//! Request payload contracts and validation rules.

use validator::Validate;
use schemars::JsonSchema;
use serde::Deserialize;

/// Payload accepted when creating a resource. Unknown fields are rejected.
#[derive(Deserialize, Validate, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Create__TYPE__Dto {
    /// Display name: 3–100 characters, containing at least one non-space character.
    #[schemars(length(min = 3, max = 100))]
    #[validate(length(min = 3, max = 100), custom(function = "non_blank"))]
    pub name: String,
}

/// Reject whitespace-only values before business logic or persistence runs.
fn non_blank(value: &str) -> Result<(), validator::ValidationError> {
    if value.trim().is_empty() {
        return Err(validator::ValidationError::new("name must not be blank"));
    }
    Ok(())
}
