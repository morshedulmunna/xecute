//! Request inputs supported by controllers:
//! - #[body]: JSON DTO with automatic deserialization and validator validation.
//! - #[param]: URL path value, tuple, or named struct (the marker is singular).
//! - #[query]: query-string DTO with automatic validation.
//! - HeaderMap: headers, including Authorization and Cookie; parse/authenticate explicitly.
//! - Form<T>: application/x-www-form-urlencoded body (no automatic validator validation).
//! - String / Bytes: raw text / binary request body.
//! - Extension<T>: data installed by middleware or an Extension router layer.
//! - Multipart: file/form uploads; see ../file_upload.rs and enable feature `upload`.
//!
//! Use at most one body-consuming extractor per handler and put it last.
//! Controller State is managed by xecute; inject services through controller fields.
//! ConnectInfo<SocketAddr> is also available when the server installs connection info.

use super::modules::GreetingService;
use axum::{body::Bytes, extract::Form, http::HeaderMap};
use schemars::JsonSchema;
use serde::Deserialize;
use std::sync::Arc;
use validator::Validate;
use xecute::prelude::*;

#[derive(Deserialize, Validate, JsonSchema)]
struct CreateDto {
    #[validate(length(min = 1, max = 100))]
    name: String,
}

#[derive(Deserialize, Validate, JsonSchema)]
struct SearchQuery {
    #[validate(range(min = 1, max = 100))]
    limit: Option<u32>,
}

#[derive(Deserialize)]
struct RecordParams {
    id: u64,
}

#[derive(Deserialize)]
struct ContactForm {
    name: String,
}

#[controller("/examples")]
#[derive(Injectable)]
pub struct ExampleController {
    service: Arc<GreetingService>,
}

#[routes]
impl ExampleController {
    /// curl -X POST localhost:3000/examples -H 'Content-Type: application/json' -d '{"name":"Munna"}'
    #[post("")]
    #[body(json = CreateDto)]
    async fn create(&self, #[body] body: CreateDto) -> Json<String> {
        Json(self.service.greet(&body.name))
    }

    /// curl 'localhost:3000/examples/42?limit=5' -H 'X-Request-Id: demo'
    /// Multiple path segments can use a named struct or a tuple in one #[param].
    #[get("/{id}")]
    async fn details(
        &self,
        #[param] params: RecordParams,
        #[query] query: SearchQuery,
        headers: HeaderMap,
    ) -> Json<serde_json::Value> {
        Json(serde_json::json!({
            "id": params.id,
            "limit": query.limit.unwrap_or(10),
            "request_id": headers.get("x-request-id").and_then(|v| v.to_str().ok()),
        }))
    }

    /// curl -X POST localhost:3000/examples/form -d 'name=Munna'
    #[post("/form")]
    async fn form(&self, form: Form<ContactForm>) -> Json<String> {
        Json(self.service.greet(&form.0.name))
    }

    /// curl -X POST localhost:3000/examples/text -H 'Content-Type: text/plain' -d 'hello'
    #[post("/text")]
    async fn text(&self, body: String) -> String {
        body
    }

    /// curl -X POST localhost:3000/examples/bytes --data-binary @Cargo.toml
    #[post("/bytes")]
    async fn bytes(&self, body: Bytes) -> Json<usize> {
        Json(body.len())
    }
}
