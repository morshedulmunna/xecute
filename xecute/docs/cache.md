# TTL caching: memory and Redis

Xecute exposes an explicit `Cache` API for JSON values or bytes, positive TTLs,
exact-key invalidation, and lifecycle cleanup. Choose bounded process-local memory
or an optional namespaced Redis backend. Caching is opt-in; no controller response
or authenticated data is cached automatically.

These APIs currently live in the local source tree:

```toml
[dependencies]
xecute = { path = "../xecute", features = ["cache"] }
# Use features = ["cache-redis"] for Redis; this also enables cache.
```

## Memory cache

```rust
use std::time::Duration;
use xecute::cache::Cache;

let cache = Cache::memory(1000, 1_048_576)?;
cache.set("catalog:v1:product:42", &serde_json::json!({"name":"Keyboard"}),
    Duration::from_secs(60)).await?;
let product = cache.get::<serde_json::Value>("catalog:v1:product:42").await?;
cache.delete("catalog:v1:product:42").await?;
cache.close().await?;
```

The first limit is entry count; the second is maximum **serialized bytes per
value**. Both must be positive. Keys must be nonempty and at most 512 bytes.
Capacity bounds stored entries, while application limits bound the working input
and serialization allocation. Memory overhead includes keys and bookkeeping in
addition to value bytes. This small cache uses a mutex and linear LRU bookkeeping;
use Redis or a specialized application cache when workloads require larger/highly
contended local caches.

Expired entries are removed lazily on reads/writes/deletes. When capacity is full,
a new key evicts the least recently read/written live entry. An overwrite replaces
both value and expiration; reads refresh LRU position **without** extending TTL.
Clones share entries. Separate application processes do not share memory entries,
and all entries disappear on restart or explicit memory-cache closure.

## API semantics

- `set(key, &value, ttl)` JSON-serializes a value. `get::<T>(key)` returns
  `Option<T>`; misses and expired entries return `None`.
- `set_bytes` / `get_bytes` support raw byte data.
- `delete(key)` invalidates only that key and returns whether a live entry existed.
- `close()` is shared and idempotent. Future operations return `Closed`; memory
  entries are cleared, while Redis entries retain their normal server TTL.

Every set requires a positive TTL. Zero, overflowing, or unrepresentable durations
return `InvalidTtl`; persistent entries are not created accidentally. Redis TTLs
are rounded **up** to whole milliseconds; memory uses the monotonic Tokio clock.
Backend writes apply value and TTL atomically within that backend. A rejected
oversized value or invalid TTL does not replace an old entry.

JSON decoding/type mismatches return `Serialization`, not `None`. Keep key schemas
versioned when changing serialized types. Use an `Option` field or a tagged value
when your domain needs to distinguish a cached null/negative result from a miss.
Errors are static and redacted. Redis failures return `Unavailable`, distinct from
a cache miss. Decide whether your application should recompute, fail, or use a
separate fallback policy; do not silently hide every storage/serialization error.

## Redis configuration

Use a dedicated local instance to try the Redis backend:

```sh
docker run --detach --rm --name xecute-redis-demo \
  -p 127.0.0.1:56379:6379 redis:8
export TEST_REDIS_URL='redis://127.0.0.1:56379/'
cargo test -p xecute --features cache-redis --test cache_redis -- --include-ignored
docker stop xecute-redis-demo
```

Configure application Redis separately from test credentials:

```rust
use std::time::Duration;
use xecute::cache::{Cache, RedisCacheConfig};

let config = RedisCacheConfig::new(
    &std::env::var("REDIS_URL")?, "users-api-v1",
)?
.max_value_bytes(1_048_576)?
.timeout(Duration::from_secs(2))?;
let cache = Cache::redis(config).await?;
cache.set("tenant:7:user:42", &42, Duration::from_secs(60)).await?;
let value = cache.get::<u64>("tenant:7:user:42").await?;
cache.close().await?;
```

URLs may use `redis://` or TLS `rediss://`. Store credentials in environment/secret
management; debug output redacts connection options and runtime errors exclude
URLs and driver messages. For networked production Redis, configure authenticated
ACLs and verified TLS; do not disable certificate verification via URL options.

The namespace is 1..=128 ASCII letters, digits, dots, underscores or hyphens.
Physical keys are `namespace:key`; clients using the same Redis database and
namespace share entries, while different namespaces remain separate. Namespace
isolation is a naming convention, not an authorization boundary: scope Redis ACLs
and infrastructure access appropriately. Xecute never calls FLUSHDB, FLUSHALL, or
wildcard deletion. Individual deletes affect only the selected namespaced key.

The stable Redis client provides multiplexing/reconnection. Initial connection
and each complete operation, including waiting on reconnection, have a configured
positive timeout (default two seconds). A timed-out network command may already
have reached Redis; it is not a transactional undo guarantee. Reconnection is not
a durable write queue. Values written by another application may still allocate
memory while being received before this wrapper can reject an oversized value;
use a cache-owned namespace and server-side memory limits as well.

Redis capacity/eviction is controlled by the server (`maxmemory` and its eviction
policy), not the memory-cache entry limit. Redis persistence, clustering,
replication/failover and availability are operational choices outside this
wrapper. Closing a handle drops its connection manager after outstanding clones
finish; it deliberately leaves shared entries untouched.

## Cache-aside and invalidation

Use explicit application caching after authorization:

```text
Controller → authorize user/record → Service or Query Handler
  → Cache lookup → hit: return value
                 → miss: Repository → cache result with TTL → return value
```

Include tenant, user/ownership scope, representation/version, locale, filters and
pagination wherever they affect a result. For example,
`v1:tenant:7:user:42:orders:page:1` must not become a shared `orders` key across
users. Do not cache authentication decisions, raw bearer tokens, or secrets using
these examples. Never allow a cache hit to bypass the guard/record-level policy
checks added in step 3.

After a successful database write, invalidate affected keys. Cache/database writes
are not one transaction; handle failed invalidation and concurrent stale readers
with your application's consistency strategy (for example versioned keys or an
outbox). TTL alone provides eventual expiration, not strict read-after-write
consistency. No automatic get-or-load/single-flight locking, stampede protection,
HTTP cache interceptor, or cross-backend transaction is provided.

## Dependency injection and lifecycle

```rust
let cache = Cache::memory_provider(1000, 1_048_576)?;
// Redis alternative: Cache::redis_provider(redis_config)
let container = xecute::Container::builder().provider(cache).build()?;
container.initialize_async().await?;
let cache = container.resolve::<Cache>()?;
// Use the cache, drain callers/jobs, then:
container.shutdown_async().await?;
```

Memory provider registration creates the value immediately, but its cleanup hook
still requires async container lifecycle. Redis connects at async startup. Module
registration, exports/imports and isolated testing overrides work as before.
Register one `Cache` provider; use concrete newtype adapters if a feature needs
multiple independent caches in the same container.

Repositories/services/query handlers can depend on `Arc<Cache>`. Workers must
declare `ProviderKey::of::<Cache>()` in their queue factory so lifecycle shutdown
drains the queue before closing the cache. See the
[jobs/cache example](../examples/jobs_cache/main.rs):

```sh
cargo run -p xecute --example jobs_cache --features jobs,cache
```

Generated MVC/CQRS resources are not automatically rewritten to use caching. In
generated servers, use the async initialization path described in the
[lifecycle guide](../README.md#async-providers-and-application-lifecycle), then
retain the container for cleanup after HTTP requests and workers drain.

## Tests and CI

```sh
# No external service needed:
cargo test -p xecute --features cache --test cache

# Redis config/timeout tests, without a live server:
cargo test -p xecute --features cache-redis --test cache_redis

# Explicit live Redis suite:
TEST_REDIS_URL=redis://127.0.0.1:56379/ \
  cargo test -p xecute --features cache-redis --test cache_redis -- --ignored
```

Memory tests cover exact expiry, non-sliding TTL, overwrite, LRU eviction, lazy
expiry before eviction, serialization/type/size errors, concurrent access,
invalidation, and lifecycle closure. Time tests use Tokio's paused clock.
Redis tests cover separate clients, namespace isolation, server TTL/overwrite,
JSON/size errors, wrong-type command failures, connection timeouts, redaction,
and cleanup that preserves shared entries.

Live tests are ignored by default, but explicitly invoking them without
`TEST_REDIS_URL` fails with an actionable message. Tests use random namespaces and
never flush Redis or alter unrelated keys. The dedicated Redis CI job runs these
checks against a disposable service.

Reference: [Redis SET with expiration](https://redis.io/docs/latest/commands/set/)
and [Redis async connection management](https://docs.rs/redis/latest/redis/aio/struct.ConnectionManager.html).
