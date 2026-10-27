//! Explicit, acyclic bus registrations share ordinary DI module validation.
use super::{
    commands::{CreateUserCommand, CreateUserHandler},
    controller::UserController,
    events::{AuditLog, AuditUserCreated, UserCreated, WelcomeUserCreated},
    pipeline::RequireCorrelation,
    queries::{ListUsersHandler, ListUsersQuery},
    repository::UserRepository,
};
use axum::Router;
use std::sync::Arc;
use xecute::{
    Container, ModuleDefinition, ModuleGraph, Provider, ProviderDefinition, Scope,
    cqrs::{CommandBus, EventBus, QueryBus},
};
pub fn container() -> Result<Container, Box<dyn std::error::Error + Send + Sync>> {
    let events = EventBus::builder()
        .handler_provider::<UserCreated, AuditUserCreated>()
        .handler_provider::<UserCreated, WelcomeUserCreated>()
        .into_provider(Scope::Singleton)?;
    let commands = CommandBus::builder()
        .handler_provider::<CreateUserCommand, CreateUserHandler>()
        .behavior_provider::<RequireCorrelation>()
        .into_provider(Scope::Singleton)?;
    let queries = QueryBus::builder()
        .handler_provider::<ListUsersQuery, ListUsersHandler>()
        .into_provider(Scope::Singleton)?;
    let users = ModuleDefinition::new("users")
        .provider(UserRepository::definition())
        .provider(AuditLog::definition())
        .provider(AuditUserCreated::definition())
        .provider(WelcomeUserCreated::definition())
        .provider(CreateUserHandler::definition())
        .provider(ListUsersHandler::definition())
        .provider(ProviderDefinition::value(RequireCorrelation))
        .provider(UserController::definition())
        .provider(events)
        .provider(commands)
        .provider(queries);
    Ok(ModuleGraph::new(vec![users]).build()?)
}
pub fn router(container: Arc<Container>) -> Router {
    xecute::exceptions::exception_boundary(UserController::scoped_router(container))
}
