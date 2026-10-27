//! Explicit JSON/byte TTL caching. Memory storage is bounded and process-local.
//! Optional Redis storage shares namespaced keys across application instances.
//! Cache only after authorization; include identity/tenant/version in private keys.

use crate::ProviderDefinition;
use serde::{Serialize, de::DeserializeOwned};
use std::{
    collections::VecDeque,
    fmt,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::time::Instant;

#[cfg(feature = "cache-redis")]
mod redis;
#[cfg(feature = "cache-redis")]
pub use redis::RedisCacheConfig;

/// Safe cache configuration, serialization, storage, or lifecycle failure.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum CacheError {
    /// Invalid limits or backend settings; details exclude submitted values.
    InvalidConfiguration(&'static str),
    /// Empty key or key exceeding 512 bytes.
    InvalidKey,
    /// TTL must be positive and representable by both clock and backend.
    InvalidTtl,
    /// Serialized value exceeds the configured per-value bound.
    TooLarge,
    /// A value could not be encoded/decoded using the requested JSON type.
    Serialization,
    /// Redis connection or command failure; never includes connection details.
    Unavailable,
    /// Explicit cache shutdown has completed.
    Closed,
    /// A memory cache lock was poisoned.
    Poisoned,
}
impl fmt::Display for CacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cache: {self:?}")
    }
}
impl std::error::Error for CacheError {}

struct Entry {
    key: String,
    value: Vec<u8>,
    expires: Instant,
}
struct Memory {
    entries: VecDeque<Entry>,
    capacity: usize,
    closed: bool,
}
enum Backend {
    Memory(Mutex<Memory>),
    #[cfg(feature = "cache-redis")]
    Redis(redis::RedisBackend),
}

/// Shared cache handle. Clones share entries and shutdown state.
#[derive(Clone)]
pub struct Cache {
    backend: Arc<Backend>,
    max_value_bytes: usize,
}

fn valid_key(key: &str) -> Result<(), CacheError> {
    if key.is_empty() || key.len() > 512 {
        Err(CacheError::InvalidKey)
    } else {
        Ok(())
    }
}
fn valid_ttl(ttl: Duration) -> Result<(Instant, u64), CacheError> {
    let expires = Instant::now()
        .checked_add(ttl)
        .filter(|_| !ttl.is_zero())
        .ok_or(CacheError::InvalidTtl)?;
    let milliseconds =
        u64::try_from(ttl.as_nanos().div_ceil(1_000_000)).map_err(|_| CacheError::InvalidTtl)?;
    if milliseconds > i64::MAX as u64 {
        return Err(CacheError::InvalidTtl);
    }
    Ok((expires, milliseconds))
}

impl Cache {
    /// Create bounded memory storage with lazy expiry and least-recently-used eviction.
    /// Capacity and maximum serialized value bytes must be positive.
    pub fn memory(capacity: usize, max_value_bytes: usize) -> Result<Self, CacheError> {
        if capacity == 0 || max_value_bytes == 0 {
            return Err(CacheError::InvalidConfiguration(
                "capacity and value limit must be positive",
            ));
        }
        Ok(Self {
            backend: Arc::new(Backend::Memory(Mutex::new(Memory {
                entries: VecDeque::new(),
                capacity,
                closed: false,
            }))),
            max_value_bytes,
        })
    }
    /// Register shared memory storage and its lifecycle cleanup hook.
    pub fn memory_provider(
        capacity: usize,
        max_value_bytes: usize,
    ) -> Result<ProviderDefinition, CacheError> {
        Ok(
            ProviderDefinition::value(Self::memory(capacity, max_value_bytes)?)
                .on_shutdown::<Self, _, _>(|cache| async move {
                    cache.close().await.map_err(|_| {
                        crate::InjectError::ConstructionFailed("cache shutdown failed")
                    })
                }),
        )
    }
    /// Cache a JSON-serializable value. Every entry requires an explicit positive TTL.
    pub async fn set<T: Serialize + ?Sized>(
        &self,
        key: &str,
        value: &T,
        ttl: Duration,
    ) -> Result<(), CacheError> {
        let bytes = serde_json::to_vec(value).map_err(|_| CacheError::Serialization)?;
        self.set_bytes(key, &bytes, ttl).await
    }
    /// Read a JSON value; a miss/expired key is None, and a type mismatch is an error.
    pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, CacheError> {
        self.get_bytes(key)
            .await?
            .map(|bytes| serde_json::from_slice(&bytes).map_err(|_| CacheError::Serialization))
            .transpose()
    }
    /// Store bytes, replacing an entry and its TTL atomically within the backend.
    pub async fn set_bytes(
        &self,
        key: &str,
        value: &[u8],
        ttl: Duration,
    ) -> Result<(), CacheError> {
        valid_key(key)?;
        let (expires, _milliseconds) = valid_ttl(ttl)?;
        if value.len() > self.max_value_bytes {
            return Err(CacheError::TooLarge);
        }
        match self.backend.as_ref() {
            Backend::Memory(state) => {
                let mut state = state.lock().map_err(|_| CacheError::Poisoned)?;
                if state.closed {
                    return Err(CacheError::Closed);
                }
                state
                    .entries
                    .retain(|entry| entry.expires > Instant::now() && entry.key != key);
                if state.entries.len() == state.capacity {
                    state.entries.pop_front();
                }
                state.entries.push_back(Entry {
                    key: key.to_owned(),
                    value: value.to_vec(),
                    expires,
                });
                Ok(())
            }
            #[cfg(feature = "cache-redis")]
            Backend::Redis(backend) => backend.set(key, value, _milliseconds).await,
        }
    }
    /// Read bytes and refresh LRU position; reads do not extend the TTL.
    pub async fn get_bytes(&self, key: &str) -> Result<Option<Vec<u8>>, CacheError> {
        valid_key(key)?;
        match self.backend.as_ref() {
            Backend::Memory(state) => {
                let mut state = state.lock().map_err(|_| CacheError::Poisoned)?;
                if state.closed {
                    return Err(CacheError::Closed);
                }
                state.entries.retain(|entry| entry.expires > Instant::now());
                let Some(index) = state.entries.iter().position(|entry| entry.key == key) else {
                    return Ok(None);
                };
                let entry = state
                    .entries
                    .remove(index)
                    .expect("position was found while holding lock");
                let value = entry.value.clone();
                state.entries.push_back(entry);
                Ok(Some(value))
            }
            #[cfg(feature = "cache-redis")]
            Backend::Redis(backend) => {
                let value = backend.get(key).await?;
                if value
                    .as_ref()
                    .is_some_and(|bytes| bytes.len() > self.max_value_bytes)
                {
                    return Err(CacheError::TooLarge);
                }
                Ok(value)
            }
        }
    }
    /// Invalidate exactly one key. Expired/missing keys return false.
    pub async fn delete(&self, key: &str) -> Result<bool, CacheError> {
        valid_key(key)?;
        match self.backend.as_ref() {
            Backend::Memory(state) => {
                let mut state = state.lock().map_err(|_| CacheError::Poisoned)?;
                if state.closed {
                    return Err(CacheError::Closed);
                }
                state.entries.retain(|entry| entry.expires > Instant::now());
                if let Some(index) = state.entries.iter().position(|entry| entry.key == key) {
                    state.entries.remove(index);
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            #[cfg(feature = "cache-redis")]
            Backend::Redis(backend) => backend.delete(key).await,
        }
    }
    /// Close this shared handle. Memory entries are cleared; Redis keys retain their TTL.
    /// Outstanding Redis commands may finish; future commands return Closed.
    pub async fn close(&self) -> Result<(), CacheError> {
        match self.backend.as_ref() {
            Backend::Memory(state) => {
                let mut state = state.lock().map_err(|_| CacheError::Poisoned)?;
                state.closed = true;
                state.entries.clear();
                Ok(())
            }
            #[cfg(feature = "cache-redis")]
            Backend::Redis(backend) => {
                backend.close().await;
                Ok(())
            }
        }
    }
}
