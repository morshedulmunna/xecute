//! Bounded per-peer-IP fixed-window quotas for one server process.
//! Use a shared limiter or ingress policy when deploying multiple instances.

use std::{
    collections::HashMap,
    net::IpAddr,
    sync::Mutex,
    time::{Duration, Instant},
};

/// A process-local limiter with a hard cap on remembered client IPs.
pub struct RateLimiter {
    clients: Mutex<HashMap<IpAddr, Window>>,
    requests: u32,
    window: Duration,
    max_clients: usize,
}

struct Window {
    started: Instant,
    requests: u32,
}

impl RateLimiter {
    /// Construct from already validated, nonzero server configuration.
    pub fn new(requests: u32, window: Duration, max_clients: usize) -> Self {
        Self {
            clients: Mutex::new(HashMap::new()),
            requests,
            window,
            max_clients,
        }
    }

    /// Consume one request allowance or return the retry delay in whole seconds.
    pub fn check(&self, ip: IpAddr) -> Result<(), u64> {
        self.check_at(ip, Instant::now())
    }

    /// Use a supplied monotonic instant for deterministic expiry checks.
    pub(crate) fn check_at(&self, ip: IpAddr, now: Instant) -> Result<(), u64> {
        let mut clients = self
            .clients
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !clients.contains_key(&ip) && clients.len() >= self.max_clients {
            clients.retain(|_, entry| now.duration_since(entry.started) < self.window);
            // Bound memory; do not evict active counters and allow quota bypass.
            if clients.len() >= self.max_clients {
                return Err(self.window.as_secs().max(1));
            }
        }
        let entry = clients.entry(ip).or_insert(Window {
            started: now,
            requests: 0,
        });
        if now.duration_since(entry.started) >= self.window {
            *entry = Window {
                started: now,
                requests: 0,
            };
        }
        if entry.requests >= self.requests {
            return Err((self.window - now.duration_since(entry.started))
                .as_secs()
                .saturating_add(1));
        }
        entry.requests += 1;
        Ok(())
    }
}
