//! Thin HTTP handlers with automatic DTO extraction and validation.

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
