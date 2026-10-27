//! PostgreSQL outbox transport for the __FEATURE__ worker.
//! Jobs are durable and claimed with `FOR UPDATE SKIP LOCKED`, so every process
//! sharing the database can safely run a consumer.

use std::{sync::Arc, time::Duration};
use uuid::Uuid;
use xecute::{
    InjectError,
    database::sqlx::{self, PgPool, postgres::PgPoolOptions},
    jobs::{JobContext, JobError},
};

use super::job::{__TYPE__Job, handle};

const MAX_ATTEMPTS: u32 = 3;
const RETRY_DELAY_MS: u64 = 250;
const POLL_INTERVAL_MS: u64 = 500;
const ATTEMPT_TIMEOUT: Duration = Duration::from_secs(30);

/// Failure while enqueueing a __FEATURE__ job.
#[derive(Debug)]
pub enum __TYPE__QueueError {
    /// The payload could not be serialized.
    Serialization,
    /// The database rejected the operation.
    Database,
}

impl std::fmt::Display for __TYPE__QueueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Serialization => f.write_str("__FEATURE__ job payload serialization failed"),
            Self::Database => f.write_str("__FEATURE__ outbox database operation failed"),
        }
    }
}

impl std::error::Error for __TYPE__QueueError {}

/// Durable PostgreSQL outbox used to enqueue __FEATURE__ jobs.
#[derive(Clone)]
pub struct __TYPE__Queue {
    pool: PgPool,
    consumer: Arc<Consumer>,
}

impl __TYPE__Queue {
    /// Insert a job for at-least-once processing.
    pub async fn enqueue(&self, payload: __TYPE__Job) -> Result<(), __TYPE__QueueError> {
        let body = serde_json::to_string(&payload).map_err(|_| __TYPE__QueueError::Serialization)?;
        sqlx::query("INSERT INTO xecute___FEATURE___jobs (id, payload) VALUES ($1, $2)")
            .bind(Uuid::new_v4().to_string())
            .bind(body)
            .execute(&self.pool)
            .await
            .map_err(|_| __TYPE__QueueError::Database)?;
        Ok(())
    }

    /// Stop the polling consumer and close the database pool.
    pub async fn shutdown(&self) {
        self.consumer.stop().await;
        self.pool.close().await;
    }
}

struct Consumer {
    stop: tokio::sync::watch::Sender<bool>,
    task: tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl Consumer {
    async fn stop(&self) {
        let _ = self.stop.send(true);
        let task = self.task.lock().await.take();
        if let Some(task) = task {
            let _ = task.await;
        }
    }
}

pub(crate) async fn start() -> Result<__TYPE__Queue, InjectError> {
    let url = std::env::var("DATABASE_URL").map_err(|_| {
        InjectError::ConstructionFailed("DATABASE_URL is required for the __FEATURE__ worker")
    })?;
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .map_err(|_| {
            InjectError::ConstructionFailed("__FEATURE__ outbox database connection failed")
        })?;
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS xecute___FEATURE___jobs (
            id TEXT PRIMARY KEY,
            payload TEXT NOT NULL,
            attempts INTEGER NOT NULL DEFAULT 0,
            available_at TIMESTAMPTZ NOT NULL DEFAULT now(),
            locked_at TIMESTAMPTZ,
            completed_at TIMESTAMPTZ,
            last_error TEXT
        )",
    )
    .execute(&pool)
    .await
    .map_err(|_| InjectError::ConstructionFailed("__FEATURE__ outbox table creation failed"))?;
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS xecute___FEATURE___jobs_available
         ON xecute___FEATURE___jobs (available_at)
         WHERE completed_at IS NULL",
    )
    .execute(&pool)
    .await
    .map_err(|_| InjectError::ConstructionFailed("__FEATURE__ outbox index creation failed"))?;

    let (stop, stop_rx) = tokio::sync::watch::channel(false);
    let task = tokio::spawn(run(pool.clone(), stop_rx));
    Ok(__TYPE__Queue {
        pool,
        consumer: Arc::new(Consumer {
            stop,
            task: tokio::sync::Mutex::new(Some(task)),
        }),
    })
}

async fn run(pool: PgPool, mut stop: tokio::sync::watch::Receiver<bool>) {
    loop {
        tokio::select! {
            changed = stop.changed() => {
                if changed.is_err() || *stop.borrow() {
                    break;
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(POLL_INTERVAL_MS)) => {}
        }
        loop {
            match process_one(&pool).await {
                Ok(true) => {}
                Ok(false) => break,
                Err(_) => {
                    tracing::warn!("__FEATURE__ outbox processing failed");
                    break;
                }
            }
            if *stop.borrow() {
                return;
            }
        }
    }
}

async fn process_one(pool: &PgPool) -> Result<bool, sqlx::Error> {
    let claimed: Option<(String, String, i32)> = sqlx::query_as(
        "WITH claimed AS (
            SELECT id FROM xecute___FEATURE___jobs
            WHERE completed_at IS NULL
              AND available_at <= now()
              AND (locked_at IS NULL OR locked_at < now() - interval '5 minutes')
            ORDER BY available_at
            FOR UPDATE SKIP LOCKED
            LIMIT 1
         )
         UPDATE xecute___FEATURE___jobs AS jobs
         SET attempts = jobs.attempts + 1, locked_at = now()
         FROM claimed
         WHERE jobs.id = claimed.id
         RETURNING jobs.id, jobs.payload, jobs.attempts",
    )
    .fetch_optional(pool)
    .await?;
    let Some((id, payload, attempts)) = claimed else {
        return Ok(false);
    };
    let Ok(payload) = serde_json::from_str::<__TYPE__Job>(&payload) else {
        finish(pool, &id, Some("invalid payload")).await?;
        return Ok(true);
    };
    let context = JobContext {
        id: Uuid::parse_str(&id).unwrap_or_else(|_| Uuid::new_v4()),
        attempt: attempts as u32,
        payload: Arc::new(payload),
    };
    match tokio::time::timeout(ATTEMPT_TIMEOUT, handle(context)).await {
        Ok(Ok(())) => finish(pool, &id, None).await?,
        Ok(Err(JobError::Permanent(code))) => finish(pool, &id, Some(code)).await?,
        Ok(Err(JobError::Retryable(code))) => retry(pool, &id, attempts as u32, code).await?,
        Err(_) => retry(pool, &id, attempts as u32, "attempt timed out").await?,
    }
    Ok(true)
}

async fn finish(pool: &PgPool, id: &str, error: Option<&str>) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE xecute___FEATURE___jobs
         SET completed_at = now(), locked_at = NULL, last_error = $2
         WHERE id = $1",
    )
    .bind(id)
    .bind(error)
    .execute(pool)
    .await?;
    Ok(())
}

async fn retry(pool: &PgPool, id: &str, attempts: u32, error: &str) -> Result<(), sqlx::Error> {
    if attempts >= MAX_ATTEMPTS {
        return finish(pool, id, Some("retry attempts exhausted")).await;
    }
    sqlx::query(
        "UPDATE xecute___FEATURE___jobs
         SET available_at = now() + ($2::double precision * interval '1 millisecond'),
             locked_at = NULL,
             last_error = $3
         WHERE id = $1",
    )
    .bind(id)
    .bind(RETRY_DELAY_MS as f64)
    .bind(error)
    .execute(pool)
    .await?;
    Ok(())
}
