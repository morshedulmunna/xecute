//! Deterministic memory-cache semantics, serialization, eviction and lifecycle.
#![cfg(feature = "cache")]
use std::{sync::Arc, time::Duration};
use xecute::{
    Container,
    cache::{Cache, CacheError},
};
const TTL: Duration = Duration::from_secs(10);

#[tokio::test(start_paused = true)]
async fn ttl_expiry_is_exact_and_reads_do_not_extend_it() {
    let cache = Cache::memory(2, 1024).unwrap();
    cache.set("key", &42, TTL).await.unwrap();
    tokio::time::advance(Duration::from_secs(9)).await;
    assert_eq!(cache.get::<u32>("key").await.unwrap(), Some(42));
    tokio::time::advance(Duration::from_secs(1)).await;
    assert_eq!(cache.get::<u32>("key").await.unwrap(), None);
    assert!(!cache.delete("key").await.unwrap());
}
#[tokio::test(start_paused = true)]
async fn overwrite_resets_ttl_and_expired_entries_do_not_evict_live_values() {
    let c = Cache::memory(2, 1024).unwrap();
    c.set("a", &1, Duration::from_secs(1)).await.unwrap();
    c.set("b", &2, TTL).await.unwrap();
    tokio::time::advance(Duration::from_secs(2)).await;
    c.set("c", &3, TTL).await.unwrap();
    assert_eq!(c.get::<u32>("b").await.unwrap(), Some(2));
    assert_eq!(c.get::<u32>("a").await.unwrap(), None);
    c.set("b", &4, TTL).await.unwrap();
    tokio::time::advance(Duration::from_secs(9)).await;
    assert_eq!(c.get::<u32>("b").await.unwrap(), Some(4));
}
#[tokio::test]
async fn lru_capacity_is_bounded_and_clones_share_invalidation() {
    let c = Cache::memory(2, 1024).unwrap();
    c.set("a", &1, TTL).await.unwrap();
    c.set("b", &2, TTL).await.unwrap();
    assert_eq!(c.get::<u32>("a").await.unwrap(), Some(1));
    c.set("c", &3, TTL).await.unwrap();
    assert_eq!(c.get::<u32>("b").await.unwrap(), None);
    let clone = c.clone();
    assert!(clone.delete("a").await.unwrap());
    assert_eq!(c.get::<u32>("a").await.unwrap(), None);
    assert!(!clone.delete("a").await.unwrap());
}
#[tokio::test]
async fn configuration_key_ttl_size_and_json_errors_do_not_destroy_old_values() {
    assert!(Cache::memory(0, 1).is_err());
    assert!(Cache::memory(1, 0).is_err());
    let c = Cache::memory(2, 8).unwrap();
    c.set("key", &1, TTL).await.unwrap();
    assert_eq!(
        c.set_bytes("key", b"123456789", TTL).await,
        Err(CacheError::TooLarge)
    );
    assert_eq!(c.get::<u32>("key").await.unwrap(), Some(1));
    assert_eq!(
        c.set("key", &2, Duration::ZERO).await,
        Err(CacheError::InvalidTtl)
    );
    assert_eq!(
        c.set("key", &2, Duration::MAX).await,
        Err(CacheError::InvalidTtl)
    );
    assert_eq!(c.set("", &2, TTL).await, Err(CacheError::InvalidKey));
    assert_eq!(
        c.get_bytes(&"x".repeat(513)).await,
        Err(CacheError::InvalidKey)
    );
    let invalid = std::collections::BTreeMap::from([(vec![1_u8, 2], 3)]);
    assert_eq!(
        c.set("invalid", &invalid, TTL).await,
        Err(CacheError::Serialization)
    );
    c.set_bytes("raw", b"not json", TTL).await.unwrap();
    assert_eq!(c.get::<u32>("raw").await, Err(CacheError::Serialization));
    c.set("key", &"text", TTL).await.unwrap();
    assert_eq!(c.get::<u32>("key").await, Err(CacheError::Serialization));
}
#[tokio::test]
async fn concurrent_writers_and_shutdown_are_safe_across_handles() {
    let cache = Arc::new(Cache::memory(64, 1024).unwrap());
    let mut tasks = Vec::new();
    for i in 0..64 {
        let c = cache.clone();
        tasks.push(tokio::spawn(async move {
            c.set(&format!("key-{i}"), &i, TTL).await.unwrap();
        }));
    }
    for task in tasks {
        task.await.unwrap();
    }
    for i in 0..64 {
        assert_eq!(
            cache.get::<u32>(&format!("key-{i}")).await.unwrap(),
            Some(i)
        );
    }
    cache.close().await.unwrap();
    cache.close().await.unwrap();
    assert_eq!(cache.get::<u32>("key-1").await, Err(CacheError::Closed));
    assert_eq!(cache.set("key", &1, TTL).await, Err(CacheError::Closed));
    assert_eq!(cache.delete("key").await, Err(CacheError::Closed));
}
#[tokio::test]
async fn provider_cleanup_closes_memory_cache() {
    let c = Container::builder()
        .provider(Cache::memory_provider(16, 1024).unwrap())
        .build()
        .unwrap();
    c.initialize_async().await.unwrap();
    let cache = c.resolve::<Cache>().unwrap();
    cache.set("key", &42, TTL).await.unwrap();
    c.shutdown_async().await.unwrap();
    assert_eq!(cache.get::<u32>("key").await, Err(CacheError::Closed));
}
