//! cargo run -p xecute --example cqrs --features cqrs
//! curl -X POST localhost:3000/users -H 'Content-Type: application/json' -d '{"name":"Munna"}'
//! curl localhost:3000/users
mod commands;
mod controller;
mod dto;
mod entity;
mod events;
mod module;
mod pipeline;
mod queries;
mod repository;
use std::sync::Arc;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let container = Arc::new(module::container()?);
    container.initialize_async().await?;
    let work: Result<(), Box<dyn std::error::Error + Send + Sync>> = async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
        println!("CQRS example: http://127.0.0.1:3000/users");
        axum::serve(listener, module::router(container.clone())).await?;
        Ok(())
    }
    .await;
    let cleanup = container.shutdown_async().await;
    work?;
    cleanup?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use tower::ServiceExt;
    #[tokio::test]
    async fn http_controller_dispatches_through_buses_repository_and_two_event_subscribers() {
        let c = Arc::new(module::container().unwrap());
        c.initialize_async().await.unwrap();
        let router = module::router(c.clone());
        for (body, status) in [
            (r#"{"name":"  Munna  "}"#, 201),
            (r#"{"name":"x"}"#, 422),
            (r#"{"name":"    "}"#, 422),
        ] {
            assert_eq!(
                router
                    .clone()
                    .oneshot(
                        Request::builder()
                            .method("POST")
                            .uri("/users")
                            .header("content-type", "application/json")
                            .body(Body::from(body))
                            .unwrap()
                    )
                    .await
                    .unwrap()
                    .status()
                    .as_u16(),
                status
            );
        }
        let response = router
            .oneshot(
                Request::builder()
                    .uri("/users")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let users: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
        assert_eq!(users.as_array().unwrap().len(), 1);
        assert_eq!(users[0]["name"], "Munna");
        let audit = c.resolve::<events::AuditLog>().unwrap();
        let entries = audit.entries();
        assert_eq!(entries.len(), 2);
        assert!(entries[0].starts_with("created:"));
        assert!(entries[1].starts_with("welcome:"));
        c.shutdown_async().await.unwrap();
    }
    #[tokio::test]
    async fn direct_command_call_also_requires_pipeline_context() {
        let c = module::container().unwrap();
        c.initialize_async().await.unwrap();
        let commands = c.resolve::<xecute::cqrs::CommandBus>().unwrap();
        let result = commands
            .execute(commands::CreateUserCommand {
                name: "Munna".into(),
            })
            .await;
        assert!(matches!(
            result,
            Err(xecute::cqrs::DispatchError::Bus(
                xecute::cqrs::BusError::Rejected("correlation_required")
            ))
        ));
        assert!(
            c.resolve::<repository::UserRepository>()
                .unwrap()
                .list()
                .await
                .is_empty()
        );
        c.shutdown_async().await.unwrap();
    }
}
