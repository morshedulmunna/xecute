//! Bounded, process-local jobs. Explicitly drain before shutting down dependencies.
//! Accepted jobs are not durable across process crashes. Handlers must be idempotent
//! when retries/timeouts can repeat side effects. No database or broker is implied.

use crate::{InjectError, ProviderDefinition};
use std::{
    fmt,
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};
use uuid::Uuid;

/// Queue setup, submission, or worker lifecycle failure.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum QueueError {
    /// Invalid capacity, worker count, retry count, or duration.
    InvalidConfiguration(&'static str),
    /// No active Tokio runtime exists.
    NoRuntime,
    /// The queue no longer accepts work.
    Closed,
    /// try_enqueue found no remaining buffer capacity.
    Full,
    /// A worker was stopped before delivering a receipt or terminated unexpectedly.
    WorkerStopped,
    /// A synchronization lock was poisoned.
    Poisoned,
}
impl fmt::Display for QueueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "background queue: {self:?}")
    }
}
impl std::error::Error for QueueError {}

/// An application-declared failure. Messages must be static, safe diagnostic codes.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum JobError {
    /// Retry according to the queue policy.
    Retryable(&'static str),
    /// Stop immediately; repeating this operation is not useful.
    Permanent(&'static str),
}

/// Final failure of one accepted job.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum JobFailure {
    /// Handler declared a permanent failure.
    Permanent(&'static str),
    /// Handler exhausted its configured attempts.
    RetryExhausted(&'static str),
    /// The final attempt exceeded its deadline.
    TimedOut,
    /// Handler panicked. Panics are isolated and are not retried.
    Panicked,
}

/// Final outcome delivered to a job receipt.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct JobOutcome {
    /// Stable identifier retained across attempts.
    pub id: Uuid,
    /// Number of attempts executed, including the first.
    pub attempts: u32,
    /// Success or terminal failure.
    pub result: Result<(), JobFailure>,
}

/// Payload and execution metadata passed to a handler.
pub struct JobContext<T> {
    /// Stable identifier suitable for an application idempotency key.
    pub id: Uuid,
    /// One-based current attempt.
    pub attempt: u32,
    /// Shared payload; handlers need not clone T for retries.
    pub payload: Arc<T>,
}

/// One caller's completion receiver. Dropping it does not cancel an accepted job.
pub struct JobReceipt {
    id: Uuid,
    result: oneshot::Receiver<JobOutcome>,
}
impl JobReceipt {
    /// Return the accepted job's stable identifier.
    pub fn id(&self) -> Uuid {
        self.id
    }
    /// Wait for completion; the queue remains responsible if this wait is cancelled.
    pub async fn wait(self) -> Result<JobOutcome, QueueError> {
        self.result.await.map_err(|_| QueueError::WorkerStopped)
    }
}

/// Validated limits for an in-process queue.
#[derive(Debug, Clone)]
pub struct QueueConfig {
    capacity: usize,
    workers: usize,
    max_attempts: u32,
    retry_delay: Duration,
    attempt_timeout: Duration,
}
impl QueueConfig {
    /// Buffer capacity excludes running jobs. Defaults: three attempts, 100ms fixed
    /// retry delay and 30s per attempt. Up to 1024 workers are supported.
    pub fn new(capacity: usize, workers: usize) -> Result<Self, QueueError> {
        if capacity == 0
            || capacity > tokio::sync::Semaphore::MAX_PERMITS
            || workers == 0
            || workers > 1024
        {
            return Err(QueueError::InvalidConfiguration(
                "require positive capacity and 1..=1024 workers",
            ));
        }
        Ok(Self {
            capacity,
            workers,
            max_attempts: 3,
            retry_delay: Duration::from_millis(100),
            attempt_timeout: Duration::from_secs(30),
        })
    }
    /// Set total attempts (1..=100) and fixed delay before a retry. Zero delay is allowed.
    pub fn retry(mut self, max_attempts: u32, delay: Duration) -> Result<Self, QueueError> {
        if !(1..=100).contains(&max_attempts)
            || tokio::time::Instant::now().checked_add(delay).is_none()
        {
            return Err(QueueError::InvalidConfiguration(
                "invalid retry attempts or delay",
            ));
        }
        self.max_attempts = max_attempts;
        self.retry_delay = delay;
        Ok(self)
    }
    /// Bound each handler attempt; the duration must be positive and representable.
    pub fn attempt_timeout(mut self, timeout: Duration) -> Result<Self, QueueError> {
        if timeout.is_zero() || tokio::time::Instant::now().checked_add(timeout).is_none() {
            return Err(QueueError::InvalidConfiguration(
                "attempt timeout must be positive and representable",
            ));
        }
        self.attempt_timeout = timeout;
        Ok(self)
    }
}

/// Counters are process-local snapshots; no per-job history is retained by the queue.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct QueueStats {
    /// Jobs accepted into the buffer.
    pub submitted: usize,
    /// Jobs completed successfully.
    pub succeeded: usize,
    /// Jobs ending in a terminal failure.
    pub failed: usize,
    /// Additional attempts started after a failure or timeout.
    pub retried: usize,
}
#[derive(Default)]
struct Counters {
    submitted: AtomicUsize,
    succeeded: AtomicUsize,
    failed: AtomicUsize,
    retried: AtomicUsize,
}

type JobFuture = Pin<Box<dyn Future<Output = Result<(), JobError>> + Send>>;
type Handler<T> = dyn Fn(JobContext<T>) -> JobFuture + Send + Sync;
struct Envelope<T> {
    id: Uuid,
    payload: Arc<T>,
    result: oneshot::Sender<JobOutcome>,
}
struct Inner<T> {
    sender: Mutex<Option<mpsc::Sender<Envelope<T>>>>,
    receiver: Arc<tokio::sync::Mutex<mpsc::Receiver<Envelope<T>>>>,
    workers: tokio::sync::Mutex<Vec<JoinHandle<()>>>,
    counters: Arc<Counters>,
}
impl<T> Drop for Inner<T> {
    fn drop(&mut self) {
        for worker in self.workers.get_mut() {
            worker.abort();
        }
    }
}

/// Cloneable submission handle and worker owner, shared across controller/service injections.
/// Dropping all handles aborts outstanding jobs; call shutdown for graceful completion.
pub struct JobQueue<T> {
    inner: Arc<Inner<T>>,
}
impl<T> Clone for JobQueue<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

struct AbortOnDrop<T>(JoinHandle<T>);
impl<T> Drop for AbortOnDrop<T> {
    fn drop(&mut self) {
        self.0.abort();
    }
}

impl<T: Send + Sync + 'static> JobQueue<T> {
    /// Start workers on the current Tokio runtime. For DI use provider so startup is deferred.
    pub fn new<F, Fut>(config: QueueConfig, handler: F) -> Result<Self, QueueError>
    where
        F: Fn(JobContext<T>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), JobError>> + Send + 'static,
    {
        Self::start(config, Arc::new(move |context| Box::pin(handler(context))))
    }

    fn start(config: QueueConfig, handler: Arc<Handler<T>>) -> Result<Self, QueueError> {
        let runtime = tokio::runtime::Handle::try_current().map_err(|_| QueueError::NoRuntime)?;
        let (sender, receiver) = mpsc::channel::<Envelope<T>>(config.capacity);
        let receiver = Arc::new(tokio::sync::Mutex::new(receiver));
        let counters = Arc::new(Counters::default());
        let mut workers = Vec::new();
        for _ in 0..config.workers {
            let receiver = receiver.clone();
            let handler = handler.clone();
            let config = config.clone();
            let counters = counters.clone();
            workers.push(runtime.spawn(async move {
                loop {
                    let job = receiver.lock().await.recv().await;
                    let Some(job) = job else {
                        break;
                    };
                    let outcome =
                        execute(job.id, job.payload, &config, handler.clone(), &counters).await;
                    if outcome.result.is_ok() {
                        counters.succeeded.fetch_add(1, Ordering::Relaxed);
                    } else {
                        counters.failed.fetch_add(1, Ordering::Relaxed);
                    }
                    let _ = job.result.send(outcome);
                }
            }));
        }
        Ok(Self {
            inner: Arc::new(Inner {
                sender: Mutex::new(Some(sender)),
                receiver,
                workers: tokio::sync::Mutex::new(workers),
                counters,
            }),
        })
    }

    /// Register a queue that starts at async DI startup and drains on lifecycle shutdown.
    /// Register dependent resources in your own factory when the handler needs DI dependencies.
    pub fn provider<F, Fut>(config: QueueConfig, handler: F) -> ProviderDefinition
    where
        F: Fn(JobContext<T>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), JobError>> + Send + 'static,
    {
        let handler: Arc<Handler<T>> = Arc::new(move |context| Box::pin(handler(context)));
        ProviderDefinition::singleton_async::<Self, _, _>(vec![], move |_| {
            let config = config.clone();
            let handler = handler.clone();
            async move {
                Self::start(config, handler)
                    .map_err(|_| InjectError::ConstructionFailed("job queue startup failed"))
            }
        })
        .on_shutdown::<Self, _, _>(|queue| async move {
            queue
                .shutdown()
                .await
                .map_err(|_| InjectError::ConstructionFailed("job queue shutdown failed"))
        })
    }

    fn sender(&self) -> Result<mpsc::Sender<Envelope<T>>, QueueError> {
        self.inner
            .sender
            .lock()
            .map_err(|_| QueueError::Poisoned)?
            .as_ref()
            .cloned()
            .ok_or(QueueError::Closed)
    }
    fn envelope(payload: T) -> (Envelope<T>, JobReceipt) {
        let id = Uuid::new_v4();
        let (sender, result) = oneshot::channel();
        (
            Envelope {
                id,
                payload: Arc::new(payload),
                result: sender,
            },
            JobReceipt { id, result },
        )
    }
    /// Wait for buffer capacity. Cancellation before this returns means no job was accepted.
    pub async fn enqueue(&self, payload: T) -> Result<JobReceipt, QueueError> {
        let sender = self.sender()?;
        let (job, receipt) = Self::envelope(payload);
        sender.send(job).await.map_err(|_| QueueError::Closed)?;
        self.inner
            .counters
            .submitted
            .fetch_add(1, Ordering::Relaxed);
        Ok(receipt)
    }
    /// Submit without waiting; reject immediately when the bounded buffer is full.
    pub fn try_enqueue(&self, payload: T) -> Result<JobReceipt, QueueError> {
        let sender = self.sender()?;
        let (job, receipt) = Self::envelope(payload);
        sender.try_send(job).map_err(|error| match error {
            mpsc::error::TrySendError::Full(_) => QueueError::Full,
            mpsc::error::TrySendError::Closed(_) => QueueError::Closed,
        })?;
        self.inner
            .counters
            .submitted
            .fetch_add(1, Ordering::Relaxed);
        Ok(receipt)
    }
    /// Return whether shutdown has stopped accepting new submissions.
    pub fn is_closed(&self) -> bool {
        self.inner
            .sender
            .lock()
            .map_or(true, |sender| sender.is_none())
    }

    /// Snapshot queue completion and retry counters.
    pub fn stats(&self) -> QueueStats {
        let c = &self.inner.counters;
        QueueStats {
            submitted: c.submitted.load(Ordering::Relaxed),
            succeeded: c.succeeded.load(Ordering::Relaxed),
            failed: c.failed.load(Ordering::Relaxed),
            retried: c.retried.load(Ordering::Relaxed),
        }
    }
    /// Stop submissions, drain accepted jobs including retries, and join every worker.
    /// Concurrent/repeated calls are safe. Cancellation preserves join handles so a
    /// later shutdown can resume. Never call from one of this queue's own handlers.
    pub async fn shutdown(&self) -> Result<(), QueueError> {
        self.inner
            .sender
            .lock()
            .map_err(|_| QueueError::Poisoned)?
            .take();
        self.inner.receiver.lock().await.close();
        let mut workers = self.inner.workers.lock().await;
        let mut failed = false;
        while let Some(worker) = workers.last_mut() {
            failed |= worker.await.is_err();
            workers.pop();
        }
        if failed {
            Err(QueueError::WorkerStopped)
        } else {
            Ok(())
        }
    }

    /// Enqueue on fixed intervals, first after one interval. Missed ticks are skipped;
    /// a full queue skips that occurrence. Keep the handle alive and stop it before
    /// draining the queue. This is per-process scheduling, not distributed cron.
    pub fn schedule_every<F>(
        &self,
        interval: Duration,
        make_payload: F,
    ) -> Result<ScheduleHandle, QueueError>
    where
        F: Fn() -> T + Send + 'static,
    {
        let first = tokio::time::Instant::now()
            .checked_add(interval)
            .filter(|_| !interval.is_zero())
            .ok_or(QueueError::InvalidConfiguration(
                "interval must be positive and representable",
            ))?;
        let runtime = tokio::runtime::Handle::try_current().map_err(|_| QueueError::NoRuntime)?;
        let queue = self.clone();
        let skipped = Arc::new(AtomicUsize::new(0));
        let missed = skipped.clone();
        let task = runtime.spawn(async move {
            let mut interval = tokio::time::interval_at(first, interval);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                interval.tick().await;
                match queue.try_enqueue(make_payload()) {
                    Ok(_) => {}
                    Err(QueueError::Full) => {
                        missed.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(ScheduleHandle {
            task: Some(task),
            skipped,
        })
    }
}

async fn execute<T: Send + Sync + 'static>(
    id: Uuid,
    payload: Arc<T>,
    config: &QueueConfig,
    handler: Arc<Handler<T>>,
    counters: &Counters,
) -> JobOutcome {
    let mut result = Err(JobFailure::TimedOut);
    let mut attempts = 0;
    for attempt in 1..=config.max_attempts {
        attempts = attempt;
        if attempt > 1 {
            counters.retried.fetch_add(1, Ordering::Relaxed);
        }
        let handler = handler.clone();
        let payload = payload.clone();
        let mut task = AbortOnDrop(tokio::spawn(async move {
            handler(JobContext {
                id,
                attempt,
                payload,
            })
            .await
        }));
        let terminal = match tokio::time::timeout(config.attempt_timeout, &mut task.0).await {
            Ok(Ok(Ok(()))) => {
                result = Ok(());
                true
            }
            Ok(Ok(Err(JobError::Permanent(reason)))) => {
                result = Err(JobFailure::Permanent(reason));
                true
            }
            Ok(Ok(Err(JobError::Retryable(reason)))) => {
                result = Err(JobFailure::RetryExhausted(reason));
                false
            }
            Ok(Err(_)) => {
                result = Err(JobFailure::Panicked);
                true
            }
            Err(_) => {
                task.0.abort();
                let _ = (&mut task.0).await;
                result = Err(JobFailure::TimedOut);
                false
            }
        };
        if terminal || attempt == config.max_attempts {
            break;
        }
        tokio::time::sleep(config.retry_delay).await;
    }
    JobOutcome {
        id,
        attempts,
        result,
    }
}

/// Owned interval schedule. Drop aborts future ticks; accepted jobs remain queued.
pub struct ScheduleHandle {
    task: Option<JoinHandle<()>>,
    skipped: Arc<AtomicUsize>,
}
impl ScheduleHandle {
    /// Count occurrences skipped because the bounded queue was full.
    pub fn skipped(&self) -> usize {
        self.skipped.load(Ordering::Relaxed)
    }
    /// Return whether the scheduling task has exited or panicked.
    pub fn is_finished(&self) -> bool {
        self.task.as_ref().is_none_or(JoinHandle::is_finished)
    }
    /// Stop future ticks and wait for exit. Repeated calls are safe; a factory
    /// panic is reported as WorkerStopped rather than silently hidden.
    pub async fn stop(&mut self) -> Result<(), QueueError> {
        if let Some(task) = self.task.as_mut() {
            task.abort();
            let result = task.await;
            self.task.take();
            if result.is_err_and(|error| !error.is_cancelled()) {
                return Err(QueueError::WorkerStopped);
            }
        }
        Ok(())
    }
}
impl Drop for ScheduleHandle {
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}
