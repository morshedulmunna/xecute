//! Core NATS transport for the __FEATURE__ worker.
//! Core NATS is at-most-once: messages are not retained when no consumer is
//! connected, and handler failures are logged without redelivery. Use JetStream
//! when durable delivery is required.

use std::{sync::Arc, time::Duration};
use futures_util::StreamExt;
use uuid::Uuid;
use xecute::{InjectError, jobs::JobContext};

use super::job::{__TYPE__Job, handle};

const SUBJECT: &str = "__FEATURE__.jobs";
const QUEUE_GROUP: &str = "__FEATURE__-workers";
const ATTEMPT_TIMEOUT: Duration = Duration::from_secs(30);

/// Failure while enqueueing a __FEATURE__ job.
#[derive(Debug)]
pub enum __TYPE__QueueError {
    /// The payload could not be serialized.
    Serialization,
    /// The broker rejected the publish.
    Publish,
}

impl std::fmt::Display for __TYPE__QueueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Serialization => f.write_str("__FEATURE__ job payload serialization failed"),
            Self::Publish => f.write_str("__FEATURE__ NATS publish failed"),
        }
    }
}

impl std::error::Error for __TYPE__QueueError {}

/// NATS client and queue-group consumer for __FEATURE__ jobs.
#[derive(Clone)]
pub struct __TYPE__Queue {
    client: async_nats::Client,
    consumer: Arc<Consumer>,
}

impl __TYPE__Queue {
    /// Publish a job to the queue group.
    pub async fn enqueue(&self, payload: __TYPE__Job) -> Result<(), __TYPE__QueueError> {
        let body = serde_json::to_vec(&payload).map_err(|_| __TYPE__QueueError::Serialization)?;
        self.client
            .publish(SUBJECT, body.into())
            .await
            .map_err(|_| __TYPE__QueueError::Publish)?;
        self.client
            .flush()
            .await
            .map_err(|_| __TYPE__QueueError::Publish)?;
        Ok(())
    }

    /// Stop the queue-group consumer.
    pub async fn shutdown(&self) {
        self.consumer.stop().await;
    }
}

struct Consumer {
    stop: Arc<tokio::sync::Notify>,
    task: tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl Consumer {
    async fn stop(&self) {
        self.stop.notify_one();
        let task = self.task.lock().await.take();
        if let Some(task) = task {
            let _ = task.await;
        }
    }
}

pub(crate) async fn start() -> Result<__TYPE__Queue, InjectError> {
    let url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_owned());
    let client = async_nats::connect(&url)
        .await
        .map_err(|_| InjectError::ConstructionFailed("__FEATURE__ NATS connection failed"))?;
    let mut subscriber = client
        .queue_subscribe(SUBJECT, QUEUE_GROUP.to_owned())
        .await
        .map_err(|_| InjectError::ConstructionFailed("__FEATURE__ NATS subscription failed"))?;

    let stop = Arc::new(tokio::sync::Notify::new());
    let task_stop = stop.clone();
    let task = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = task_stop.notified() => break,
                message = subscriber.next() => {
                    let Some(message) = message else { break; };
                    process(&message).await;
                }
            }
        }
    });
    Ok(__TYPE__Queue {
        client,
        consumer: Arc::new(Consumer {
            stop,
            task: tokio::sync::Mutex::new(Some(task)),
        }),
    })
}

async fn process(message: &async_nats::Message) {
    let payload: __TYPE__Job = match serde_json::from_slice(&message.payload) {
        Ok(payload) => payload,
        Err(_) => {
            tracing::warn!("__FEATURE__ NATS payload was invalid");
            return;
        }
    };
    let context = JobContext {
        id: Uuid::new_v4(),
        attempt: 1,
        payload: Arc::new(payload),
    };
    match tokio::time::timeout(ATTEMPT_TIMEOUT, handle(context)).await {
        Ok(Ok(())) => {}
        _ => tracing::warn!("__FEATURE__ NATS job failed"),
    }
}
