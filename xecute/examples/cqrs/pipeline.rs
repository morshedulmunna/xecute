//! A shared policy enforced for every bus caller, not just HTTP controllers.
use xecute::cqrs::{Behavior, BusError, DispatchFuture, Next};
#[derive(Clone)]
pub struct CorrelationId(pub String);
pub struct RequireCorrelation;
impl Behavior for RequireCorrelation {
    fn handle<'a>(&'a self, next: Next<'a>) -> DispatchFuture<'a> {
        Box::pin(async move {
            if next.context().value::<CorrelationId>().is_none() {
                return Err(BusError::Rejected("correlation_required"));
            }
            next.run().await
        })
    }
}
