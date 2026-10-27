//! DTO extraction and explicit safe HTTP error mapping; no SQL or business workflow here.
use super::{
    commands::{CreateUserCommand, CreateUserError},
    dto::CreateUserDto,
    entity::User,
    pipeline::CorrelationId,
    queries::ListUsersQuery,
};
use axum::Extension;
use std::sync::Arc;
use xecute::{
    cqrs::{CommandBus, DispatchContext, DispatchError, QueryBus},
    exceptions::RequestId,
    prelude::*,
};
#[controller("/users")]
#[derive(Injectable)]
pub struct UserController {
    commands: Arc<CommandBus>,
    queries: Arc<QueryBus>,
}
#[routes]
impl UserController {
    #[post("")]
    #[body(json=CreateUserDto)]
    #[resp(201,"User created",json=User)]
    #[resp(422, "Invalid name")]
    async fn create(
        &self,
        request_id: Extension<RequestId>,
        #[body] dto: CreateUserDto,
    ) -> Result<(HttpStatus, Json<User>), HttpError> {
        let context = DispatchContext::default().with_value(CorrelationId(request_id.0.0));
        let user = self
            .commands
            .execute_with(CreateUserCommand { name: dto.name }, context)
            .await
            .map_err(|error| match error {
                DispatchError::Handler(CreateUserError::InvalidName) => {
                    HttpError::new(HttpStatus::UNPROCESSABLE_ENTITY)
                }
                DispatchError::Handler(CreateUserError::Capacity) => {
                    HttpError::new(HttpStatus::SERVICE_UNAVAILABLE)
                }
                DispatchError::Bus(_) => HttpError::new(HttpStatus::INTERNAL_SERVER_ERROR),
            })?;
        Ok((HttpStatus::CREATED, Json(user)))
    }
    #[get("")]
    #[resp(200,"Users",json=Vec<User>)]
    async fn list(&self) -> Result<Json<Vec<User>>, HttpError> {
        self.queries
            .execute(ListUsersQuery)
            .await
            .map(Json)
            .map_err(|_| HttpError::new(HttpStatus::INTERNAL_SERVER_ERROR))
    }
}
