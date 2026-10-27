//! HTTP-safe entity representation.
#[derive(Clone, serde::Serialize, schemars::JsonSchema)]
pub struct User {
    pub id: String,
    pub name: String,
}
