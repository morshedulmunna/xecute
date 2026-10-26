//! Thin HTTP handlers with automatic DTO extraction and validation.
//!
//! Request input guide (applies to both MVC and CQRS controllers):
//! - `#[body] dto: Create__TYPE__Dto`: JSON body, decoded and validated automatically.
//!   Example: POST /__FEATURE__ with {"name":"Munna"} and Content-Type: application/json.
//! - `#[param] id: u64`: path parameter, e.g. #[get("/{id}")] for /__FEATURE__/42.
//!   For multiple segments use one tuple or Deserialize struct with matching field names.
//! - `#[query] query: SearchDto`: /__FEATURE__?limit=10; DTO must derive
//!   Deserialize, Validate and JsonSchema. Use Option<T> for optional fields.
//! - `headers: axum::http::HeaderMap`: request headers, including Authorization
//!   and Cookie. Read with headers.get("authorization"); authentication and cookie
//!   parsing require your own logic/middleware. There is no #[headers] marker.
//! - `form: axum::extract::Form<FormDto>`: URL-encoded form body; FormDto derives
//!   Deserialize. Call validator explicitly if needed (unlike #[body]/#[query]).
//! - `body: String`: raw UTF-8 text; `body: axum::body::Bytes`: raw binary body.
//! - `context: axum::Extension<MyContext>`: middleware/shared request data;
//!   install a matching Extension layer first and derive Clone on MyContext.
//! - `peer: axum::extract::ConnectInfo<std::net::SocketAddr>`: client socket address;
//!   serve with into_make_service_with_connect_info::<std::net::SocketAddr>().
//! - `multipart: xecute::upload::Multipart`: file/form uploads; enable xecute's
//!   `upload` feature and use #[multipart] on the handler for OpenAPI metadata.
//!
//! Example handler signatures to adapt inside this #[routes] impl:
//! ```ignore
//! #[get("/{id}")]
//! async fn find(&self, #[param] id: u64, #[query] query: SearchDto,
//!               headers: axum::http::HeaderMap) -> Json<u64> {
//!     // Use id, query and headers in your service or query handler.
//!     Json(id)
//! }
//!
//! #[post("/form")]
//! async fn submit(&self, form: axum::extract::Form<FormDto>) -> String {
//!     // Pass form.0 to your service or command handler.
//!     "Accepted".into()
//! }
//! ```
//! Use at most one body-consuming extractor (JSON, form, text, bytes, multipart,
//! or Request), placed last. Do not combine #[body] with Form or Multipart.
//! Controller state is managed by xecute; inject services through controller fields.
//! #[api], #[body(json = Dto)] and #[resp] describe OpenAPI; parameter #[body]
//! performs extraction. Other HTTP verbs: #[put], #[patch], #[delete], #[head], #[options].

use std::sync::Arc;
use xecute::HttpStatus;
use xecute::prelude::*;

use super::dto::*;
use super::entity::__TYPE__;
use super::service::__TYPE__Service;

/// Injectable controller owning the resource route prefix.
#[controller("/__FEATURE__")]
#[derive(Injectable)]
pub struct __TYPE__Controller {
    service: Arc<__TYPE__Service>,
}

#[routes]
impl __TYPE__Controller {
    /// Retrieve the current collection.
    #[get("")]
    #[api(
        summary = "List __FEATURE__ records",
        tag = "__TYPE__",
        operation_id = "list__TYPE__"
    )]
    #[resp(200, "Current collection", json = Vec<__TYPE__>)]
    async fn list(&self) -> Json<Vec<__TYPE__>> {
        Json(self.service.list().await)
    }

    /// Create a resource after automatic JSON decoding and validator validation.
    #[post("")]
    #[api(
        summary = "Create a __FEATURE__ record",
        tag = "__TYPE__",
        operation_id = "create__TYPE__"
    )]
    #[body(json = Create__TYPE__Dto)]
    #[resp(201, "Resource created", json = __TYPE__)]
    #[resp(422, "Invalid name or payload")]
    #[resp(503, "Repository capacity exhausted")]
    async fn create(
        &self,
        #[body] request: Create__TYPE__Dto,
    ) -> Result<(HttpStatus, Json<__TYPE__>), HttpError> {
        let entity = self
            .service
            .create(request)
            .await?;

        Ok((HttpStatus::CREATED, Json(entity)))
    }
}
