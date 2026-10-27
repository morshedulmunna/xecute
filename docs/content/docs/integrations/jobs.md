---
title: "Jobs and scheduling"
description: "Run bounded process-local workers with retries, receipts and interval schedules."
---

Move suitable work out of a request handler into bounded workers. These jobs run in the current process and are not durable across restart; choose that model before configuring retries.

Enable Xecute's `jobs` feature for bounded, process-local Tokio workers with
receipts, retries, per-attempt deadlines, graceful draining, and interval schedules.

```toml
[dependencies]
xecute = { version = "0.1", features = ["jobs"] }
```

This feature does not require a broker or database. **Accepted jobs are not persisted:** a process crash/restart loses
queued and running work. Use a durable broker or a database outbox/worker system
for work that must survive a crash. This implementation is suitable for bounded
best-effort application work; it does not claim exactly-once delivery, distributed
leases, dead-letter storage, or multi-process queue coordination.

## Create a queue and observe completion

```rust
use std::time::Duration;
use xecute::jobs::{JobContext, JobError, JobQueue, QueueConfig};

struct SendEmail { user_id: u64 }

let config = QueueConfig::new(128, 4)?
    .retry(3, Duration::from_millis(250))?
    .attempt_timeout(Duration::from_secs(10))?;

let queue = JobQueue::new(config, |job: JobContext<SendEmail>| async move {
    // Pass job.id to your delivery API as an idempotency key when supported.
    // Call an async email client using job.payload.user_id here.
    let _ = (job.id, job.attempt, job.payload.user_id);
    Ok::<(), JobError>(())
})?;

let receipt = queue.enqueue(SendEmail { user_id: 42 }).await?;
let job_id = receipt.id();
let outcome = receipt.wait().await?;
assert_eq!(outcome.id, job_id);
assert!(outcome.result.is_ok());
queue.shutdown().await?;
```

`JobQueue::new` must run inside an active Tokio runtime with its time driver
enabled. `JobQueue::provider` defers startup until the container's async startup
and attaches a queue-draining shutdown hook. Queue clones share workers, buffer,
shutdown state, and counters; they do not create additional worker pools.

`JobContext<T>` contains an `Arc<T>` payload, stable UUID, and one-based attempt
number. Payloads need `Send + Sync + 'static`, but do not need `Clone`. A receipt
waits for this particular accepted job; dropping its receipt does not cancel the
job. No unbounded in-memory job history is retained, and an ID alone cannot be
used to look up an old result. Retain/observe receipts or record application job
state when clients need later status checks.

## Backpressure, limits, and retries

`QueueConfig::new(capacity, workers)` requires positive capacity and 1..=1024
workers. Capacity is the maximum **waiting** job count; up to `workers` additional
jobs can be running. Payload size is application-controlled; choose appropriate
payload bounds as well as a buffer count. Retry delay occupies a worker slot,
preventing a failing dependency from creating an unbounded retry backlog.

- `enqueue(payload).await` waits for capacity; cancellation before it returns
  means the payload was not accepted.
- `try_enqueue(payload)` returns `QueueError::Full` immediately when capacity is
  exhausted, and `Closed` after shutdown starts.
- A failed submission consumes its payload; retain a clone/application record
  yourself if you intend to submit again.

Defaults are three total attempts, 100ms fixed delay, and a 30-second deadline per
attempt. `.retry(max_attempts, delay)` supports 1..=100 total attempts; zero delay
is valid. `.attempt_timeout(duration)` requires a positive, representable duration.

Handlers return:

```rust
Err(JobError::Retryable("temporary_delivery_error"))
// or
Err(JobError::Permanent("recipient_invalid"))
```

Only `Retryable` errors and timeouts are retried, up to the configured attempt
count. Permanent errors stop immediately. Handler panics become a terminal
`JobFailure::Panicked`; they do not kill the queue's other workers or cause an
automatic retry. Rust's ordinary panic hook still controls panic logging, so do
not panic with secrets or request payloads. Use static, safe diagnostic codes in
`JobError` values.

`JobOutcome` reports `attempts` and `result`, with permanent, retry-exhausted,
timed-out, and panicked terminal states. A receipt returns `WorkerStopped` if its
worker is aborted before delivering an outcome. `stats()` exposes submitted,
succeeded, failed, and retried counters as process-local approximate snapshots.
Terminal failures are not automatically stored or published; observe outcomes
and emit application logs/metrics deliberately.

On timeout, the attempt task is aborted and joined before retrying. Cancelling a
Rust future does not undo a remote operation that already committed. Use the same
`job.id` across retries as an idempotency key, and apply application/database
uniqueness guarantees where required. CPU-heavy or blocking code prevents prompt
cooperative cancellation; workers are for nonblocking async handlers. `spawn_blocking`
work and external side effects require their own cancellation/lifetime policy.

## Dependency injection and resource ordering

For a handler without DI dependencies:

```rust
let definition = JobQueue::provider(config, |job: JobContext<SendEmail>| async move {
    let _ = job.payload.user_id;
    Ok::<(), JobError>(())
});
let container = xecute::Container::builder().provider(definition).build()?;
container.initialize_async().await?;
let queue = container.resolve::<JobQueue<SendEmail>>()?;
// Submit work.
container.shutdown_async().await?;
```

When handlers depend on a database, cache, or client, use your own
`ProviderDefinition::singleton_async::<JobQueue<YourJob>, _, _>` factory. Declare
all `ProviderKey` dependencies, resolve them from its supplied container, and
capture their Arcs in the handler. Attach
`.on_shutdown::<JobQueue<YourJob>, _, _>(...)` to await `queue.shutdown()`.
This dependency graph makes queues drain **before** databases/caches close.
Do not resolve the queue back into its own handler: that creates an ownership
cycle and can prevent final-drop cancellation.

The runnable [jobs/cache example](https://github.com/morshedulmunna/xecute/blob/main/xecute/examples/jobs_cache/main.rs) implements this
pattern with `Cache` as an explicit queue dependency, a retryable job, and cleanup
even when application work fails:

```sh
cargo run -p xecute --example jobs_cache --features jobs,cache
```

Controllers can inject `Arc<JobQueue<YourJob>>` or extract it with `Inject<T>` in a
scoped router. Prefer small payloads containing stable record IDs. Do not copy
Authorization headers, request scopes, or borrowed HTTP request data into jobs.
Background jobs have application-owned lifetime, not HTTP request lifetime.
Authorize the request before enqueueing, and perform any current record-level
policy checks your job requires when it executes.

Each payload type identifies one concrete DI provider. Use distinct payload/newtype
wrappers for independent queues. Module imports/exports and test overrides follow
the existing container rules. With the `testing` feature, queues work with
`TestingModule::compile_async()`.

## Graceful shutdown and cancellation

`shutdown().await` stops submissions, rejects blocked producers, drains already
accepted jobs including retries, and joins all workers. Calls are safe concurrently
or repeatedly. `is_closed()` reports whether submissions have stopped. Never call
shutdown from one of the same queue's handlers: shutdown waits for that handler.

Stop HTTP submission sources and interval schedules first, then drain requests
and queues, then close their dependencies. Estimate a drain deadline from queue
capacity, worker count, attempt limits, retry delays, and actual handler behavior.
There is no implicit queue-wide shutdown timeout.

If a caller cancels `queue.shutdown()`, join handles remain owned by the queue;
calling it again resumes draining. Dropping all queue handles aborts its workers
and their currently executing attempt tasks. That is cancellation, not graceful
completion. If you put an external timeout around `Container::shutdown_async`,
remember the lifecycle system does not retry a cancelled cleanup hook: retain a
queue handle if you need to resume its drain explicitly.

## Interval schedules

```rust
let mut schedule = queue.schedule_every(Duration::from_secs(60), || {
    SendEmail { user_id: 42 }
})?;
// Keep the handle alive while the schedule is wanted.
let skipped = schedule.skipped();
schedule.stop().await?;
queue.shutdown().await?;
```

The first occurrence is after one full interval, not immediately. Missed ticks use
Tokio's `Skip` policy. A full queue skips that occurrence and increments
`skipped()` rather than accumulating jobs. A closed queue stops the schedule.
Schedule creation does not wait for individual job results; observe queue failure
counters and instrument the handler. The payload factory must be short and
nonblocking. `is_finished()` reveals an exited scheduling task, and `stop()`
reports an unexpected task/factory panic as `WorkerStopped`. Stop is idempotent;
dropping the handle aborts future ticks but does not cancel already accepted jobs.

Schedules are per-process: four replicas run four schedules. Distributed singleton
cron, persisted schedules, cron expressions, time zones, and leader election are
not included. To lifecycle-manage a schedule, put its handle behind a Tokio mutex
in its own singleton provider, declare the queue as its dependency, and stop the
handle in its shutdown hook. This orders schedule stop before queue draining.

## Tests

```sh
cargo test -p xecute --features jobs,cache,testing --test jobs --test cache --example jobs_cache
```

Queue tests cover retry identity/delay, permanent and exhausted failures, timeout
cancellation, panic isolation, bounded concurrency/capacity, cancelled submissions,
dropped receipts, shutdown draining/cancellation/resumption, final-drop task
ownership, scheduling timing/full-queue behavior, schedule factory failures, and
DI cleanup order. Time-dependent unit tests use Tokio's paused clock.

Reference: [Tokio task ownership and cancellation](https://docs.rs/tokio/latest/tokio/task/struct.JoinHandle.html)
and [interval missed-tick behavior](https://docs.rs/tokio/latest/tokio/time/enum.MissedTickBehavior.html).

## Keep learning

[Memory and Redis caching](/docs/integrations/cache). Return to the [learning path](/docs) whenever you need a different topic.

---

[Maintained source](https://github.com/morshedulmunna/xecute/blob/main/xecute/docs/jobs.md).
