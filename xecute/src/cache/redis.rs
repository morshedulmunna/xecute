//! Redis-backed namespaced cache; command failures stay redacted.
use super::{Backend, Cache, CacheError};
use crate::{InjectError, ProviderDefinition};
use ::redis::{
    Client,
    aio::{ConnectionManager, ConnectionManagerConfig},
};
use std::{fmt, sync::Arc, time::Duration};

/// Redis connection settings. Debug output never includes credentials or connection options.
#[derive(Clone)]
pub struct RedisCacheConfig {
    client: Client,
    namespace: String,
    max_value_bytes: usize,
    timeout: Duration,
}
impl fmt::Debug for RedisCacheConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RedisCacheConfig")
            .field("connection", &"[REDACTED]")
            .field("namespace", &self.namespace)
            .field("max_value_bytes", &self.max_value_bytes)
            .field("timeout", &self.timeout)
            .finish()
    }
}
impl RedisCacheConfig {
    /// Accept redis:// or rediss://. Namespace is 1..=128 ASCII letters, digits,
    /// dots, hyphens or underscores; keys are prefixed with namespace + ':'.
    /// Defaults: 1MiB maximum value and 2s total connection/command timeout.
    pub fn new(url: &str, namespace: &str) -> Result<Self, CacheError> {
        if !(url.starts_with("redis://") || url.starts_with("rediss://")) {
            return Err(CacheError::InvalidConfiguration("expected Redis URL"));
        }
        if namespace.is_empty()
            || namespace.len() > 128
            || !namespace
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
        {
            return Err(CacheError::InvalidConfiguration("invalid namespace"));
        }
        let client =
            Client::open(url).map_err(|_| CacheError::InvalidConfiguration("invalid Redis URL"))?;
        Ok(Self {
            client,
            namespace: namespace.to_owned(),
            max_value_bytes: 1_048_576,
            timeout: Duration::from_secs(2),
        })
    }
    /// Set the largest serialized value this handle will write or return.
    pub fn max_value_bytes(mut self, bytes: usize) -> Result<Self, CacheError> {
        if bytes == 0 {
            return Err(CacheError::InvalidConfiguration(
                "value limit must be positive",
            ));
        }
        self.max_value_bytes = bytes;
        Ok(self)
    }
    /// Bound connection setup and each complete cache command, including reconnection waits.
    pub fn timeout(mut self, timeout: Duration) -> Result<Self, CacheError> {
        if timeout.is_zero() || tokio::time::Instant::now().checked_add(timeout).is_none() {
            return Err(CacheError::InvalidConfiguration(
                "timeout must be positive and representable",
            ));
        }
        self.timeout = timeout;
        Ok(self)
    }
}

pub(super) struct RedisBackend {
    connection: tokio::sync::RwLock<Option<ConnectionManager>>,
    namespace: String,
    timeout: Duration,
}
impl RedisBackend {
    async fn connection(&self) -> Result<ConnectionManager, CacheError> {
        self.connection
            .read()
            .await
            .as_ref()
            .cloned()
            .ok_or(CacheError::Closed)
    }
    fn key(&self, key: &str) -> String {
        format!("{}:{key}", self.namespace)
    }
    pub(super) async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, CacheError> {
        let mut connection = self.connection().await?;
        tokio::time::timeout(
            self.timeout,
            ::redis::cmd("GET")
                .arg(self.key(key))
                .query_async(&mut connection),
        )
        .await
        .map_err(|_| CacheError::Unavailable)?
        .map_err(|_| CacheError::Unavailable)
    }
    pub(super) async fn set(
        &self,
        key: &str,
        value: &[u8],
        milliseconds: u64,
    ) -> Result<(), CacheError> {
        let mut connection = self.connection().await?;
        tokio::time::timeout(
            self.timeout,
            ::redis::cmd("SET")
                .arg(self.key(key))
                .arg(value)
                .arg("PX")
                .arg(milliseconds)
                .query_async::<()>(&mut connection),
        )
        .await
        .map_err(|_| CacheError::Unavailable)?
        .map_err(|_| CacheError::Unavailable)
    }
    pub(super) async fn delete(&self, key: &str) -> Result<bool, CacheError> {
        let mut connection = self.connection().await?;
        let removed: usize = tokio::time::timeout(
            self.timeout,
            ::redis::cmd("DEL")
                .arg(self.key(key))
                .query_async(&mut connection),
        )
        .await
        .map_err(|_| CacheError::Unavailable)?
        .map_err(|_| CacheError::Unavailable)?;
        Ok(removed != 0)
    }
    pub(super) async fn close(&self) {
        self.connection.write().await.take();
    }
}

impl Cache {
    /// Connect a shared Redis cache. Initial connection and commands are timeout-bounded.
    pub async fn redis(config: RedisCacheConfig) -> Result<Self, CacheError> {
        let options = ConnectionManagerConfig::new()
            .set_connection_timeout(Some(config.timeout))
            .set_response_timeout(Some(config.timeout));
        let connection = tokio::time::timeout(
            config.timeout,
            config.client.get_connection_manager_with_config(options),
        )
        .await
        .map_err(|_| CacheError::Unavailable)?
        .map_err(|_| CacheError::Unavailable)?;
        Ok(Self {
            backend: Arc::new(Backend::Redis(RedisBackend {
                connection: tokio::sync::RwLock::new(Some(connection)),
                namespace: config.namespace,
                timeout: config.timeout,
            })),
            max_value_bytes: config.max_value_bytes,
        })
    }
    /// Register a Redis connection at async startup and drop it at lifecycle shutdown.
    /// Shutdown leaves server-side entries to expire normally, without flushing any namespace.
    pub fn redis_provider(config: RedisCacheConfig) -> ProviderDefinition {
        ProviderDefinition::singleton_async::<Self, _, _>(vec![], move |_| {
            let config = config.clone();
            async move {
                Self::redis(config)
                    .await
                    .map_err(|_| InjectError::ConstructionFailed("Redis cache connection failed"))
            }
        })
        .on_shutdown::<Self, _, _>(|cache| async move {
            cache
                .close()
                .await
                .map_err(|_| InjectError::ConstructionFailed("cache shutdown failed"))
        })
    }
}
