//! RabbitMQ transport for the __FEATURE__ worker.
//! A durable queue distributes jobs across every connected process. Retryable
//! failures are republished with an incremented attempt header.

use std::{sync::Arc, time::Duration};
use futures_util::StreamExt;
use lapin::{
    BasicProperties, Connection, ConnectionProperties,
    message::Delivery,
    options::{
        BasicAckOptions, BasicConsumeOptions, BasicNackOptions, BasicPublishOptions,
        BasicQosOptions, BasicRejectOptions, QueueDeclareOptions,
    },
    types::{AMQPValue, FieldTable},
};
use uuid::Uuid;
use xecute::{
    InjectError,
    jobs::{JobContext, JobError},
};

use super::job::{__TYPE__Job, handle};

const QUEUE: &str = "__FEATURE___jobs";
const MAX_ATTEMPTS: u32 = 3;
const RETRY_DELAY: Duration = Duration::from_millis(250);
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
            Self::Publish => f.write_str("__FEATURE__ RabbitMQ publish failed"),
        }
    }
}

impl std::error::Error for __TYPE__QueueError {}

/// Durable RabbitMQ queue used to enqueue __FEATURE__ jobs.
#[derive(Clone)]
pub struct __TYPE__Queue {
    channel: lapin::Channel,
    consumer: Arc<Consumer>,
}

impl __TYPE__Queue {
    /// Publish a job for at-least-once processing.
    pub async fn enqueue(&self, payload: __TYPE__Job) -> Result<(), __TYPE__QueueError> {
        let body = serde_json::to_vec(&payload).map_err(|_| __TYPE__QueueError::Serialization)?;
        let properties = BasicProperties::default()
            .with_content_type("application/json".into())
            .with_delivery_mode(2)
            .with_message_id(Uuid::new_v4().to_string().into())
            .with_headers(attempt_headers(1));
        self.channel
            .basic_publish(
                "".into(),
                QUEUE.into(),
                BasicPublishOptions::default(),
                &body,
                properties,
            )
            .await
            .map_err(|_| __TYPE__QueueError::Publish)?
            .await
            .map_err(|_| __TYPE__QueueError::Publish)?;
        Ok(())
    }

    /// Stop the consumer and close the broker connection.
    pub async fn shutdown(&self) {
        self.consumer.stop().await;
    }
}

struct Consumer {
    connection: Connection,
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
        let _ = self.connection.close(200, "OK".into()).await;
    }
}

pub(crate) async fn start() -> Result<__TYPE__Queue, InjectError> {
    let url = std::env::var("RABBITMQ_URL")
        .unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".to_owned());
    let connection = Connection::connect(&url, ConnectionProperties::default())
        .await
        .map_err(|_| InjectError::ConstructionFailed("__FEATURE__ RabbitMQ connection failed"))?;
    let channel = connection
        .create_channel()
        .await
        .map_err(|_| InjectError::ConstructionFailed("__FEATURE__ RabbitMQ channel failed"))?;
    channel
        .queue_declare(
            QUEUE.into(),
            QueueDeclareOptions {
                durable: true,
                ..QueueDeclareOptions::default()
            },
            FieldTable::default(),
        )
        .await
        .map_err(|_| InjectError::ConstructionFailed("__FEATURE__ RabbitMQ queue declaration failed"))?;
    channel
        .basic_qos(4, BasicQosOptions::default())
        .await
        .map_err(|_| InjectError::ConstructionFailed("__FEATURE__ RabbitMQ prefetch failed"))?;
    let mut consumer = channel
        .basic_consume(
            QUEUE.into(),
            "".into(),
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await
        .map_err(|_| InjectError::ConstructionFailed("__FEATURE__ RabbitMQ consumer failed"))?;

    let stop = Arc::new(tokio::sync::Notify::new());
    let task_stop = stop.clone();
    let task_channel = channel.clone();
    let task = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = task_stop.notified() => break,
                delivery = consumer.next() => {
                    let Some(delivery) = delivery else { break; };
                    let Ok(delivery) = delivery else { continue; };
                    process(&task_channel, &delivery).await;
                }
            }
        }
    });
    Ok(__TYPE__Queue {
        channel,
        consumer: Arc::new(Consumer {
            connection,
            stop,
            task: tokio::sync::Mutex::new(Some(task)),
        }),
    })
}

fn attempt_headers(attempt: u32) -> FieldTable {
    let mut headers = FieldTable::default();
    headers.insert("x-attempt".into(), AMQPValue::LongUInt(attempt));
    headers
}

async fn process(channel: &lapin::Channel, delivery: &Delivery) {
    let payload: __TYPE__Job = match serde_json::from_slice(&delivery.data) {
        Ok(payload) => payload,
        Err(_) => {
            let _ = delivery
                .reject(BasicRejectOptions { requeue: false })
                .await;
            return;
        }
    };
    let id = delivery
        .properties
        .message_id()
        .as_ref()
        .and_then(|value| Uuid::parse_str(value.as_str()).ok())
        .unwrap_or_else(Uuid::new_v4);
    let attempt = delivery
        .properties
        .headers()
        .as_ref()
        .and_then(|headers| headers.inner().get("x-attempt"))
        .and_then(|value| match value {
            AMQPValue::ShortShortUInt(value) => Some(u32::from(*value)),
            AMQPValue::ShortUInt(value) => Some(u32::from(*value)),
            AMQPValue::LongUInt(value) => Some(*value),
            AMQPValue::LongInt(value) => u32::try_from(*value).ok(),
            AMQPValue::LongLongInt(value) => u32::try_from(*value).ok(),
            _ => None,
        })
        .unwrap_or(1);
    let retry_payload = payload.clone();
    let context = JobContext {
        id,
        attempt,
        payload: Arc::new(payload),
    };
    match tokio::time::timeout(ATTEMPT_TIMEOUT, handle(context)).await {
        Ok(Ok(())) => {
            let _ = delivery.ack(BasicAckOptions::default()).await;
        }
        Ok(Err(JobError::Permanent(_))) => {
            let _ = delivery
                .reject(BasicRejectOptions { requeue: false })
                .await;
        }
        Ok(Err(JobError::Retryable(_))) | Err(_) => {
            if attempt >= MAX_ATTEMPTS {
                let _ = delivery
                    .reject(BasicRejectOptions { requeue: false })
                    .await;
                return;
            }
            tokio::time::sleep(RETRY_DELAY).await;
            if republish(channel, &retry_payload, id, attempt + 1).await {
                let _ = delivery.ack(BasicAckOptions::default()).await;
            } else {
                let _ = delivery
                    .nack(BasicNackOptions {
                        multiple: false,
                        requeue: true,
                    })
                    .await;
            }
        }
    }
}

async fn republish(
    channel: &lapin::Channel,
    payload: &__TYPE__Job,
    id: Uuid,
    attempt: u32,
) -> bool {
    let Ok(body) = serde_json::to_vec(payload) else {
        return false;
    };
    let properties = BasicProperties::default()
        .with_content_type("application/json".into())
        .with_delivery_mode(2)
        .with_message_id(id.to_string().into())
        .with_headers(attempt_headers(attempt));
    match channel
        .basic_publish(
            "".into(),
            QUEUE.into(),
            BasicPublishOptions::default(),
            &body,
            properties,
        )
        .await
    {
        Ok(confirm) => confirm.await.is_ok(),
        Err(_) => false,
    }
}
