//! Explicit live Redis checks use dedicated namespaces and never FLUSHDB/FLUSHALL.
#![cfg(feature = "cache-redis")]
use std::time::Duration;
use xecute::{
    Container,
    cache::{Cache, CacheError, RedisCacheConfig},
};

#[test]
fn redis_config_validates_limits_and_redacts_credentials() {
    for (url, namespace) in [
        ("http://host", "test"),
        ("redis://localhost", ""),
        ("redis://localhost", "bad:*"),
        ("redis://localhost:bad", "test"),
    ] {
        assert!(RedisCacheConfig::new(url, namespace).is_err());
    }
    let config = RedisCacheConfig::new(
        "redis://private_user:private_password@private_host:6379",
        "test",
    )
    .unwrap();
    let debug = format!("{config:?}");
    for secret in ["private_user", "private_password", "private_host"] {
        assert!(!debug.contains(secret));
    }
    assert!(config.clone().max_value_bytes(0).is_err());
    assert!(config.clone().timeout(Duration::ZERO).is_err());
    assert!(config.timeout(Duration::MAX).is_err());
}
#[tokio::test]
async fn stalled_redis_connection_is_bounded_and_redacted() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("redis://:test_secret@{}", listener.local_addr().unwrap());
    let config = RedisCacheConfig::new(&url, "timeout")
        .unwrap()
        .timeout(Duration::from_millis(50))
        .unwrap();
    let result = tokio::time::timeout(Duration::from_secs(3), Cache::redis(config))
        .await
        .unwrap();
    assert!(matches!(result, Err(CacheError::Unavailable)));
    assert!(!format!("{}", result.err().unwrap()).contains("test_secret"));
}
fn url() -> String {
    std::env::var("TEST_REDIS_URL").expect("set TEST_REDIS_URL to a dedicated Redis instance")
}
fn namespace() -> String {
    format!("xecute_test_{}", uuid::Uuid::new_v4().simple())
}
fn config(namespace: &str) -> RedisCacheConfig {
    RedisCacheConfig::new(&url(), namespace).unwrap()
}

#[tokio::test]
#[ignore = "requires Redis via TEST_REDIS_URL"]
async fn independent_clients_share_entries_but_namespaces_are_isolated() {
    let ns = namespace();
    let one = Cache::redis(config(&ns)).await.unwrap();
    let two = Cache::redis(config(&ns)).await.unwrap();
    let other = Cache::redis(config(&namespace())).await.unwrap();
    one.set(
        "user:42",
        &serde_json::json!({"name":"Munna"}),
        Duration::from_secs(30),
    )
    .await
    .unwrap();
    let value: serde_json::Value = two.get("user:42").await.unwrap().unwrap();
    assert_eq!(value["name"], "Munna");
    assert_eq!(
        other.get::<serde_json::Value>("user:42").await.unwrap(),
        None
    );
    assert!(two.delete("user:42").await.unwrap());
    assert!(!one.delete("user:42").await.unwrap());
    one.close().await.unwrap();
    two.close().await.unwrap();
    other.close().await.unwrap();
}
#[tokio::test]
#[ignore = "requires Redis via TEST_REDIS_URL"]
async fn redis_ttl_is_atomic_positive_and_overwrite_replaces_expiration() {
    let ns = namespace();
    let cache = Cache::redis(config(&ns)).await.unwrap();
    cache
        .set_bytes("ttl", b"short", Duration::from_millis(100))
        .await
        .unwrap();
    let client = redis::Client::open(url()).unwrap();
    let mut con = client.get_multiplexed_async_connection().await.unwrap();
    let pttl: i64 = redis::cmd("PTTL")
        .arg(format!("{ns}:ttl"))
        .query_async(&mut con)
        .await
        .unwrap();
    assert!(pttl > 0 && pttl <= 100);
    cache
        .set_bytes("ttl", b"long", Duration::from_secs(30))
        .await
        .unwrap();
    let pttl: i64 = redis::cmd("PTTL")
        .arg(format!("{ns}:ttl"))
        .query_async(&mut con)
        .await
        .unwrap();
    assert!(pttl > 1000);
    cache
        .set_bytes("expires", b"value", Duration::from_millis(25))
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        while cache.get_bytes("expires").await.unwrap().is_some() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert!(!cache.delete("expires").await.unwrap());
    assert_eq!(
        cache.set_bytes("ttl", b"bad", Duration::ZERO).await,
        Err(CacheError::InvalidTtl)
    );
    assert_eq!(
        cache.get_bytes("ttl").await.unwrap(),
        Some(b"long".to_vec())
    );
    cache.delete("ttl").await.unwrap();
    cache.close().await.unwrap();
}
#[tokio::test]
#[ignore = "requires Redis via TEST_REDIS_URL"]
async fn lifecycle_close_does_not_delete_shared_entries_and_size_type_errors_are_explicit() {
    let ns = namespace();
    let c = Container::builder()
        .provider(Cache::redis_provider(
            config(&ns).max_value_bytes(8).unwrap(),
        ))
        .build()
        .unwrap();
    c.initialize_async().await.unwrap();
    let cache = c.resolve::<Cache>().unwrap();
    cache
        .set("number", &42, Duration::from_secs(30))
        .await
        .unwrap();
    assert_eq!(
        cache
            .set_bytes("number", b"123456789", Duration::from_secs(1))
            .await,
        Err(CacheError::TooLarge)
    );
    cache
        .set_bytes("raw", b"not-json", Duration::from_secs(30))
        .await
        .unwrap();
    assert_eq!(
        cache.get::<u32>("raw").await,
        Err(CacheError::Serialization)
    );
    c.shutdown_async().await.unwrap();
    assert_eq!(cache.get::<u32>("number").await, Err(CacheError::Closed));
    let reopened = Cache::redis(config(&ns)).await.unwrap();
    assert_eq!(reopened.get::<u32>("number").await.unwrap(), Some(42));
    reopened.delete("number").await.unwrap();
    reopened.delete("raw").await.unwrap();
    reopened.close().await.unwrap();
}
#[tokio::test]
#[ignore = "requires Redis via TEST_REDIS_URL"]
async fn redis_driver_errors_and_external_oversized_entries_are_safe() {
    let ns = namespace();
    let cache = Cache::redis(config(&ns).max_value_bytes(4).unwrap())
        .await
        .unwrap();
    let client = redis::Client::open(url()).unwrap();
    let mut con = client.get_multiplexed_async_connection().await.unwrap();
    let _: usize = redis::cmd("LPUSH")
        .arg(format!("{ns}:list"))
        .arg("private_data")
        .query_async(&mut con)
        .await
        .unwrap();
    assert_eq!(cache.get_bytes("list").await, Err(CacheError::Unavailable));
    let _: () = redis::cmd("SET")
        .arg(format!("{ns}:large"))
        .arg("123456789")
        .arg("PX")
        .arg(30000)
        .query_async(&mut con)
        .await
        .unwrap();
    assert_eq!(cache.get_bytes("large").await, Err(CacheError::TooLarge));
    cache.delete("list").await.unwrap();
    cache.delete("large").await.unwrap();
    cache.close().await.unwrap();
}
