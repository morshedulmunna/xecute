//! Quota lifecycle tests: bounded client tracking, active counters, and expired windows.

use crate::server::rate_limit::RateLimiter;
use std::time::{Duration, Instant};
#[test]
fn bounds_clients_without_evicting_live_quotas_and_resets_expired_windows() {
    let limiter = RateLimiter::new(1, Duration::from_secs(60), 1);
    let now = Instant::now();
    let first = "127.0.0.1".parse().unwrap();
    let second = "127.0.0.2".parse().unwrap();
    assert!(limiter.check_at(first, now).is_ok());
    assert!(limiter.check_at(second, now).is_err());
    assert!(limiter.check_at(first, now).is_err());
    assert!(
        limiter
            .check_at(second, now + Duration::from_secs(60))
            .is_ok()
    );
}
