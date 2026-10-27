//! Bounded queue, retry/timeout, ownership, scheduling and DI lifecycle contracts.
#![cfg(feature = "jobs")]
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use xecute::{
    Container, ProviderDefinition, ProviderKey,
    jobs::{JobContext, JobError, JobFailure, JobQueue, QueueConfig, QueueError},
};

fn config() -> QueueConfig {
    QueueConfig::new(8, 2).unwrap()
}

#[test]
fn invalid_limits_and_missing_runtime_are_explicit_errors() {
    assert!(QueueConfig::new(0, 1).is_err());
    assert!(QueueConfig::new(1, 0).is_err());
    assert!(QueueConfig::new(1, 1025).is_err());
    assert!(QueueConfig::new(usize::MAX, 1).is_err());
    assert!(config().retry(0, Duration::ZERO).is_err());
    assert!(config().retry(101, Duration::ZERO).is_err());
    assert!(config().attempt_timeout(Duration::ZERO).is_err());
    assert!(config().attempt_timeout(Duration::MAX).is_err());
    assert!(matches!(
        JobQueue::new(config(), |_: JobContext<()>| async { Ok(()) }),
        Err(QueueError::NoRuntime)
    ));
}

#[tokio::test(start_paused = true)]
async fn retry_keeps_identity_payload_and_obeys_delay_then_delivers_outcome() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let events = seen.clone();
    let q = JobQueue::new(
        config().retry(3, Duration::from_secs(2)).unwrap(),
        move |job: JobContext<String>| {
            let events = events.clone();
            async move {
                events
                    .lock()
                    .unwrap()
                    .push((job.id, job.attempt, job.payload.as_str().to_owned()));
                if job.attempt < 3 {
                    Err(JobError::Retryable("temporary"))
                } else {
                    Ok(())
                }
            }
        },
    )
    .unwrap();
    let before = tokio::time::Instant::now();
    let receipt = q.enqueue("email".to_owned()).await.unwrap();
    let id = receipt.id();
    let result = receipt.wait().await.unwrap();
    assert_eq!(result.id, id);
    assert_eq!(result.attempts, 3);
    assert_eq!(result.result, Ok(()));
    assert_eq!(tokio::time::Instant::now() - before, Duration::from_secs(4));
    assert_eq!(
        *seen.lock().unwrap(),
        [
            (id, 1, "email".into()),
            (id, 2, "email".into()),
            (id, 3, "email".into())
        ]
    );
    q.shutdown().await.unwrap();
    assert_eq!(q.stats().retried, 2);
    assert_eq!(q.stats().succeeded, 1);
}

#[tokio::test(start_paused = true)]
async fn permanent_and_exhausted_errors_are_observable_without_killing_workers() {
    let q = JobQueue::new(
        config().retry(2, Duration::ZERO).unwrap(),
        |job: JobContext<u8>| async move {
            match *job.payload {
                0 => Err(JobError::Permanent("invalid")),
                1 => Err(JobError::Retryable("unavailable")),
                _ => Ok(()),
            }
        },
    )
    .unwrap();
    let permanent = q.enqueue(0).await.unwrap().wait().await.unwrap();
    assert_eq!(permanent.attempts, 1);
    assert_eq!(permanent.result, Err(JobFailure::Permanent("invalid")));
    let exhausted = q.enqueue(1).await.unwrap().wait().await.unwrap();
    assert_eq!(exhausted.attempts, 2);
    assert_eq!(
        exhausted.result,
        Err(JobFailure::RetryExhausted("unavailable"))
    );
    assert!(
        q.enqueue(2)
            .await
            .unwrap()
            .wait()
            .await
            .unwrap()
            .result
            .is_ok()
    );
    q.shutdown().await.unwrap();
    assert_eq!(q.stats().failed, 2);
    assert_eq!(q.stats().submitted, 3);
}

#[tokio::test]
async fn handler_panics_are_terminal_but_other_jobs_continue() {
    let q = JobQueue::new(config(), |job: JobContext<bool>| async move {
        assert!(!*job.payload, "test handler panic");
        Ok(())
    })
    .unwrap();
    let panic = q.enqueue(true).await.unwrap().wait().await.unwrap();
    assert_eq!(panic.result, Err(JobFailure::Panicked));
    assert_eq!(panic.attempts, 1);
    assert!(
        q.enqueue(false)
            .await
            .unwrap()
            .wait()
            .await
            .unwrap()
            .result
            .is_ok()
    );
    q.shutdown().await.unwrap();
}

struct DropCounter(Arc<AtomicUsize>);
impl Drop for DropCounter {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
#[tokio::test(start_paused = true)]
async fn timeout_cancels_each_attempt_before_retrying() {
    let drops = Arc::new(AtomicUsize::new(0));
    let counter = drops.clone();
    let q = JobQueue::new(
        config()
            .retry(2, Duration::ZERO)
            .unwrap()
            .attempt_timeout(Duration::from_secs(1))
            .unwrap(),
        move |_: JobContext<()>| {
            let counter = counter.clone();
            async move {
                let _guard = DropCounter(counter);
                std::future::pending().await
            }
        },
    )
    .unwrap();
    let outcome = q.enqueue(()).await.unwrap().wait().await.unwrap();
    assert_eq!(outcome.result, Err(JobFailure::TimedOut));
    assert_eq!(outcome.attempts, 2);
    assert_eq!(drops.load(Ordering::SeqCst), 2);
    q.shutdown().await.unwrap();
}

#[tokio::test]
async fn bounded_buffer_backpressures_and_shutdown_rejects_waiting_producers_then_drains() {
    let entered = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Semaphore::new(0));
    let signal = entered.clone();
    let gate = release.clone();
    let q = JobQueue::new(QueueConfig::new(1, 1).unwrap(), move |_: JobContext<u8>| {
        let signal = signal.clone();
        let gate = gate.clone();
        async move {
            signal.notify_one();
            gate.acquire().await.unwrap().forget();
            Ok(())
        }
    })
    .unwrap();
    let first = q.enqueue(1).await.unwrap();
    entered.notified().await;
    let second = q.try_enqueue(2).unwrap();
    assert!(matches!(q.try_enqueue(3), Err(QueueError::Full)));
    let producer = q.clone();
    let waiting = tokio::spawn(async move { producer.enqueue(4).await });
    tokio::task::yield_now().await;
    let shutdown = q.shutdown();
    tokio::pin!(shutdown);
    tokio::select! {result=&mut shutdown=>panic!("completed before drain: {result:?}"),_=tokio::task::yield_now()=>{}}
    assert!(q.is_closed());
    assert!(matches!(q.try_enqueue(5), Err(QueueError::Closed)));
    assert!(matches!(waiting.await.unwrap(), Err(QueueError::Closed)));
    release.add_permits(2);
    shutdown.await.unwrap();
    assert!(first.wait().await.unwrap().result.is_ok());
    assert!(second.wait().await.unwrap().result.is_ok());
    q.shutdown().await.unwrap();
    assert_eq!(q.stats().submitted, 2);
}

#[tokio::test]
async fn concurrency_is_limited_and_dropped_receipts_do_not_cancel_work() {
    let entered = Arc::new(tokio::sync::Semaphore::new(0));
    let gate = Arc::new(tokio::sync::Semaphore::new(0));
    let active = Arc::new(AtomicUsize::new(0));
    let maximum = Arc::new(AtomicUsize::new(0));
    let (signal, release, now, max) = (
        entered.clone(),
        gate.clone(),
        active.clone(),
        maximum.clone(),
    );
    let q = JobQueue::new(config(), move |_: JobContext<()>| {
        let (signal, release, now, max) =
            (signal.clone(), release.clone(), now.clone(), max.clone());
        async move {
            let count = now.fetch_add(1, Ordering::SeqCst) + 1;
            max.fetch_max(count, Ordering::SeqCst);
            signal.add_permits(1);
            release.acquire().await.unwrap().forget();
            now.fetch_sub(1, Ordering::SeqCst);
            Ok(())
        }
    })
    .unwrap();
    for _ in 0..4 {
        drop(q.enqueue(()).await.unwrap());
    }
    entered.acquire_many(2).await.unwrap().forget();
    assert_eq!(maximum.load(Ordering::SeqCst), 2);
    gate.add_permits(4);
    q.shutdown().await.unwrap();
    assert_eq!(q.stats().succeeded, 4);
    assert!(maximum.load(Ordering::SeqCst) <= 2);
}

#[tokio::test]
async fn cancelled_shutdown_can_resume_without_detaching_worker_tasks() {
    let gate = Arc::new(tokio::sync::Notify::new());
    let waiting = gate.clone();
    let q = JobQueue::new(config(), move |_: JobContext<()>| {
        let waiting = waiting.clone();
        async move {
            waiting.notified().await;
            Ok(())
        }
    })
    .unwrap();
    let receipt = q.enqueue(()).await.unwrap();
    {
        let shutdown = q.shutdown();
        tokio::pin!(shutdown);
        tokio::select! {_=&mut shutdown=>panic!("job still blocked"),_=tokio::task::yield_now()=>{}}
    }
    assert!(q.is_closed());
    gate.notify_one();
    q.shutdown().await.unwrap();
    assert!(receipt.wait().await.unwrap().result.is_ok());
}

#[tokio::test]
async fn dropping_queue_aborts_inflight_attempt_instead_of_detaching_it() {
    let entered = Arc::new(tokio::sync::Notify::new());
    let signal = entered.clone();
    let dropped = Arc::new(tokio::sync::Notify::new());
    let drop_signal = dropped.clone();
    struct NotifyDrop(Arc<tokio::sync::Notify>);
    impl Drop for NotifyDrop {
        fn drop(&mut self) {
            self.0.notify_one();
        }
    }
    let q = JobQueue::new(config(), move |_: JobContext<()>| {
        let (signal, dropped) = (signal.clone(), drop_signal.clone());
        async move {
            let _guard = NotifyDrop(dropped);
            signal.notify_one();
            std::future::pending().await
        }
    })
    .unwrap();
    let receipt = q.enqueue(()).await.unwrap();
    entered.notified().await;
    drop(q);
    dropped.notified().await;
    assert_eq!(receipt.wait().await, Err(QueueError::WorkerStopped));
}

#[tokio::test(start_paused = true)]
async fn interval_schedules_delay_first_tick_and_stop_idempotently() {
    let q = JobQueue::new(config(), |_: JobContext<()>| async { Ok(()) }).unwrap();
    let mut schedule = q.schedule_every(Duration::from_secs(10), || ()).unwrap();
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
    assert_eq!(q.stats().submitted, 0);
    tokio::time::advance(Duration::from_secs(10)).await;
    for _ in 0..20 {
        tokio::task::yield_now().await;
    }
    assert_eq!(q.stats().succeeded, 1);
    schedule.stop().await.unwrap();
    schedule.stop().await.unwrap();
    tokio::time::advance(Duration::from_secs(100)).await;
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
    assert_eq!(q.stats().submitted, 1);
    q.shutdown().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn schedule_skips_full_queue_without_unbounded_backlog() {
    let gate = Arc::new(tokio::sync::Semaphore::new(0));
    let signal = Arc::new(tokio::sync::Notify::new());
    let (release, entered) = (gate.clone(), signal.clone());
    let q = JobQueue::new(
        QueueConfig::new(1, 1)
            .unwrap()
            .attempt_timeout(Duration::from_secs(1000))
            .unwrap(),
        move |_: JobContext<()>| {
            let (release, entered) = (release.clone(), entered.clone());
            async move {
                entered.notify_one();
                release.acquire().await.unwrap().forget();
                Ok(())
            }
        },
    )
    .unwrap();
    drop(q.enqueue(()).await.unwrap());
    signal.notified().await;
    drop(q.enqueue(()).await.unwrap());
    let mut schedule = q.schedule_every(Duration::from_secs(1), || ()).unwrap();
    tokio::time::advance(Duration::from_secs(1)).await;
    for _ in 0..20 {
        tokio::task::yield_now().await;
    }
    assert_eq!(schedule.skipped(), 1);
    assert_eq!(q.stats().submitted, 2);
    schedule.stop().await.unwrap();
    gate.add_permits(2);
    q.shutdown().await.unwrap();
}

#[tokio::test]
async fn lifecycle_drains_queue_before_closing_its_dependency() {
    struct Resource(Arc<Mutex<Vec<&'static str>>>);
    let events = Arc::new(Mutex::new(Vec::new()));
    let log = events.clone();
    let resource = ProviderDefinition::value(Resource(events.clone()))
        .on_shutdown::<Resource, _, _>(|resource| async move {
            resource.0.lock().unwrap().push("resource.close");
            Ok(())
        });
    let queue = ProviderDefinition::singleton_async::<JobQueue<()>, _, _>(
        vec![ProviderKey::of::<Resource>()],
        |c| async move {
            let resource = c.resolve::<Resource>()?;
            JobQueue::new(config(), move |_: JobContext<()>| {
                let resource = resource.clone();
                async move {
                    resource.0.lock().unwrap().push("job");
                    Ok(())
                }
            })
            .map_err(|_| xecute::InjectError::ConstructionFailed("queue startup failed"))
        },
    )
    .on_shutdown::<JobQueue<()>, _, _>(|queue| async move {
        queue
            .shutdown()
            .await
            .map_err(|_| xecute::InjectError::ConstructionFailed("queue shutdown failed"))
    });
    let c = Container::builder()
        .provider(resource)
        .provider(queue)
        .build()
        .unwrap();
    c.initialize_async().await.unwrap();
    let queue = c.resolve::<JobQueue<()>>().unwrap();
    let receipt = queue.enqueue(()).await.unwrap();
    c.shutdown_async().await.unwrap();
    assert!(receipt.wait().await.unwrap().result.is_ok());
    assert_eq!(*log.lock().unwrap(), ["job", "resource.close"]);
}

#[cfg(feature = "testing")]
#[tokio::test]
async fn queue_provider_integrates_with_async_testing_module() {
    let c = xecute::testing::TestingModule::new()
        .provider(JobQueue::provider(config(), |_: JobContext<u32>| async {
            Ok(())
        }))
        .compile_async()
        .await
        .unwrap();
    let q = c.resolve::<JobQueue<u32>>().unwrap();
    assert!(
        q.enqueue(42)
            .await
            .unwrap()
            .wait()
            .await
            .unwrap()
            .result
            .is_ok()
    );
    c.shutdown_async().await.unwrap();
    assert!(q.is_closed());
}

#[tokio::test]
async fn cancelled_backpressured_submission_does_not_enqueue_a_ghost_job() {
    let entered = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Semaphore::new(0));
    let (signal, gate) = (entered.clone(), release.clone());
    let q = JobQueue::new(QueueConfig::new(1, 1).unwrap(), move |_: JobContext<u8>| {
        let (signal, gate) = (signal.clone(), gate.clone());
        async move {
            signal.notify_one();
            gate.acquire().await.unwrap().forget();
            Ok(())
        }
    })
    .unwrap();
    let one = q.enqueue(1).await.unwrap();
    entered.notified().await;
    let two = q.enqueue(2).await.unwrap();
    let producer = q.clone();
    let pending = tokio::spawn(async move { producer.enqueue(3).await });
    tokio::task::yield_now().await;
    pending.abort();
    assert!(pending.await.err().unwrap().is_cancelled());
    release.add_permits(2);
    q.shutdown().await.unwrap();
    one.wait().await.unwrap();
    two.wait().await.unwrap();
    assert_eq!(q.stats().submitted, 2);
    assert_eq!(q.stats().succeeded, 2);
}

#[tokio::test(start_paused = true)]
async fn payload_factory_panic_is_observable_from_schedule_handle() {
    let q = JobQueue::new(config(), |_: JobContext<()>| async { Ok(()) }).unwrap();
    let mut schedule = q
        .schedule_every(Duration::from_secs(1), || {
            panic!("test schedule factory panic")
        })
        .unwrap();
    tokio::time::advance(Duration::from_secs(1)).await;
    for _ in 0..20 {
        tokio::task::yield_now().await;
    }
    assert!(schedule.is_finished());
    assert_eq!(schedule.stop().await, Err(QueueError::WorkerStopped));
    schedule.stop().await.unwrap();
    q.shutdown().await.unwrap();
}
