//! Write workflow: normalize/validate, persist, then explicitly publish a domain event.
use super::{entity::User, events::UserCreated, repository::UserRepository};
use std::sync::Arc;
use xecute::{
    Injectable,
    cqrs::{Command, CommandHandler, DispatchContext, EventBus},
};
pub struct CreateUserCommand {
    pub name: String,
}
impl Command for CreateUserCommand {
    type Output = User;
    type Error = CreateUserError;
}
#[derive(Debug)]
pub enum CreateUserError {
    InvalidName,
    Capacity,
}
#[derive(Injectable)]
pub struct CreateUserHandler {
    repository: Arc<UserRepository>,
    events: Arc<EventBus>,
}
impl CommandHandler<CreateUserCommand> for CreateUserHandler {
    async fn handle(
        &self,
        command: CreateUserCommand,
        context: DispatchContext,
    ) -> Result<User, CreateUserError> {
        let name = command.name.trim();
        if !(3..=100).contains(&name.chars().count()) {
            return Err(CreateUserError::InvalidName);
        }
        let user = self
            .repository
            .create(name.to_owned())
            .await
            .ok_or(CreateUserError::Capacity)?;
        let report = self
            .events
            .publish_with(
                UserCreated {
                    id: user.id.clone(),
                },
                context,
            )
            .await;
        if !matches!(report,Ok(ref report) if report.is_success()) {
            // Persistence already succeeded. Report delivery separately; do not pretend
            // that returning a handler error would roll back the committed user.
            eprintln!("user event delivery incomplete");
        }
        Ok(user)
    }
}
