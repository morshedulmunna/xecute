//! Run: cargo run -p xecute --example async_lifecycle
//! No external database needed: replace Connection::connect with a real async client.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use xecute::{Container, InjectError, ProviderDefinition, ProviderKey};

struct Connection {
    closed: AtomicBool,
}

impl Connection {
    async fn connect() -> Result<Self, InjectError> {
        tokio::task::yield_now().await;
        println!("connection established");
        Ok(Self {
            closed: AtomicBool::new(false),
        })
    }

    async fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
        println!("connection closed");
    }
}

struct Service {
    connection: Arc<Connection>,
}

#[tokio::main]
async fn main() -> Result<(), InjectError> {
    let container = Container::builder()
        .provider(
            ProviderDefinition::singleton_async::<Connection, _, _>(vec![], |_| async {
                Connection::connect().await
            })
            .on_shutdown::<Connection, _, _>(|connection| async move {
                connection.close().await;
                Ok(())
            }),
        )
        .provider(
            ProviderDefinition::singleton::<Service, _>(
                vec![ProviderKey::of::<Connection>()],
                |container| {
                    Ok(Service {
                        connection: container.resolve()?,
                    })
                },
            )
            .on_init::<Service, _, _>(|service| async move {
                assert!(!service.connection.closed.load(Ordering::SeqCst));
                println!("service ready");
                Ok(())
            }),
        )
        .build()?;

    container.initialize_async().await?;
    // Bind/serve HTTP only after initialization; drain requests before shutdown.
    let service = container.resolve::<Service>()?;
    assert!(!service.connection.closed.load(Ordering::SeqCst));
    container.shutdown_async().await?;
    assert!(service.connection.closed.load(Ordering::SeqCst));
    Ok(())
}
