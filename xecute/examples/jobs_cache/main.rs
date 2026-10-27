//! cargo run -p xecute --example jobs_cache --features jobs,cache
//! A lifecycle-managed worker depends on a TTL cache and finishes before cache closure.
use std::time::Duration;
use xecute::{
    Container, ProviderDefinition, ProviderKey,
    cache::Cache,
    jobs::{JobContext, JobError, JobQueue, QueueConfig},
};

struct BuildReport {
    user_id: u64,
}

async fn example() -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
    let queue = ProviderDefinition::singleton_async::<JobQueue<BuildReport>, _, _>(
        vec![ProviderKey::of::<Cache>()],
        |container| async move {
            let cache = container.resolve::<Cache>()?;
            let config = QueueConfig::new(32, 2)
                .and_then(|config| config.retry(3, Duration::from_millis(10)))
                .map_err(|_| xecute::InjectError::ConstructionFailed("queue config failed"))?;
            JobQueue::new(config, move |job: JobContext<BuildReport>| {
                let cache = cache.clone();
                async move {
                    // Demonstrate a transient failure. Real handlers should use job.id as an idempotency key.
                    if job.attempt == 1 {
                        return Err(JobError::Retryable("temporary_report_error"));
                    }
                    let report =
                        serde_json::json!({"user_id":job.payload.user_id,"status":"ready"});
                    cache
                        .set(
                            &format!("report:user:{}", job.payload.user_id),
                            &report,
                            Duration::from_secs(60),
                        )
                        .await
                        .map_err(|_| JobError::Retryable("cache_unavailable"))?;
                    Ok(())
                }
            })
            .map_err(|_| xecute::InjectError::ConstructionFailed("worker startup failed"))
        },
    )
    .on_shutdown::<JobQueue<BuildReport>, _, _>(|queue| async move {
        queue
            .shutdown()
            .await
            .map_err(|_| xecute::InjectError::ConstructionFailed("worker shutdown failed"))
    });
    let container = Container::builder()
        .provider(Cache::memory_provider(64, 1_048_576)?)
        .provider(queue)
        .build()?;
    container.initialize_async().await?;
    let work: Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> = async {
        let queue = container.resolve::<JobQueue<BuildReport>>()?;
        let receipt = queue.enqueue(BuildReport { user_id: 42 }).await?;
        let id = receipt.id();
        let outcome = receipt.wait().await?;
        if outcome.result.is_err() {
            return Err("report job failed".into());
        }
        println!("Job {id} completed after {} attempts", outcome.attempts);
        let cache = container.resolve::<Cache>()?;
        cache
            .get::<serde_json::Value>("report:user:42")
            .await?
            .ok_or_else(|| "report cache miss".into())
    }
    .await;
    let cleanup = container.shutdown_async().await;
    let report = work?;
    cleanup?;
    Ok(report)
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("Cached report: {}", example().await?);
    Ok(())
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn worker_retries_then_caches_report_and_shuts_down() {
        let report = super::example().await.unwrap();
        assert_eq!(report["user_id"], 42);
        assert_eq!(report["status"], "ready");
    }
}
