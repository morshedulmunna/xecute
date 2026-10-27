//! HTTP payload validation; command handlers also validate non-HTTP entry points.
#[derive(serde::Deserialize, validator::Validate, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateUserDto {
    #[validate(length(min = 3, max = 100))]
    pub name: String,
}
