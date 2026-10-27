//! Multiple event subscribers with shared correlation metadata and observable state.
use super::pipeline::CorrelationId;
use std::sync::{Arc, Mutex};
use xecute::{
    Injectable,
    cqrs::{DispatchContext, Event, EventError, EventHandler},
};
pub struct UserCreated {
    pub id: String,
}
impl Event for UserCreated {}
#[derive(Injectable)]
pub struct AuditLog {
    #[inject(default)]
    entries: Mutex<Vec<String>>,
}
impl AuditLog {
    #[cfg(test)]
    pub fn entries(&self) -> Vec<String> {
        self.entries.lock().unwrap().clone()
    }
}
#[derive(Injectable)]
pub struct AuditUserCreated {
    log: Arc<AuditLog>,
}
impl EventHandler<UserCreated> for AuditUserCreated {
    async fn handle(
        &self,
        event: Arc<UserCreated>,
        context: DispatchContext,
    ) -> Result<(), EventError> {
        let correlation = context
            .value::<CorrelationId>()
            .ok_or(EventError::Handler("missing_correlation"))?;
        let mut entries = self.log.entries.lock().unwrap();
        // Bound demonstration audit state rather than accumulating an unbounded history.
        if entries.len() >= 1000 {
            entries.remove(0);
        }
        entries.push(format!("created:{}:{}", event.id, correlation.0));
        Ok(())
    }
}
#[derive(Injectable)]
pub struct WelcomeUserCreated {
    log: Arc<AuditLog>,
}
impl EventHandler<UserCreated> for WelcomeUserCreated {
    async fn handle(&self, event: Arc<UserCreated>, _: DispatchContext) -> Result<(), EventError> {
        let mut entries = self.log.entries.lock().unwrap();
        if entries.len() >= 1000 {
            entries.remove(0);
        }
        // This example records an action; it does not actually send email.
        entries.push(format!("welcome:{}", event.id));
        Ok(())
    }
}
