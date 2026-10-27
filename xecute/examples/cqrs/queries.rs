//! Read workflow, separate from command-side side effects.
use super::{entity::User, repository::UserRepository};
use std::{convert::Infallible, sync::Arc};
use xecute::{
    Injectable,
    cqrs::{DispatchContext, Query, QueryHandler},
};
pub struct ListUsersQuery;
impl Query for ListUsersQuery {
    type Output = Vec<User>;
    type Error = Infallible;
}
#[derive(Injectable)]
pub struct ListUsersHandler {
    repository: Arc<UserRepository>,
}
impl QueryHandler<ListUsersQuery> for ListUsersHandler {
    async fn handle(&self, _: ListUsersQuery, _: DispatchContext) -> Result<Vec<User>, Infallible> {
        Ok(self.repository.list().await)
    }
}
