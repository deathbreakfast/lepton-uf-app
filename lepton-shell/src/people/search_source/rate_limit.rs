//! Per-person token bucket for people searches, held in process memory.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, PoisonError};
use std::time::Instant;

/// Searches one person may run back to back.
pub const BURST: u32 = 20;

/// Searches handed back per second.
pub const REFILL_PER_SEC: f64 = 5.0;

/// Buckets kept before full ones are dropped.
const MAX_TRACKED: usize = 10_000;

/// The limiter every `lepton_people` search goes through.
pub static PEOPLE_SEARCH_LIMITER: LazyLock<RateLimiter> =
    LazyLock::new(|| RateLimiter::new(f64::from(BURST), REFILL_PER_SEC));

/// Tokens left for one person, and when they were last counted.
#[derive(Debug, Clone, Copy)]
pub struct Bucket {
    tokens: f64,
    last: Instant,
}

impl Bucket {
    /// A bucket holding `capacity` tokens at `now`.
    pub fn full(capacity: f64, now: Instant) -> Self {
        Self {
            tokens: capacity,
            last: now,
        }
    }

    fn refill(&mut self, capacity: f64, refill_per_sec: f64, now: Instant) {
        let elapsed = now.saturating_duration_since(self.last).as_secs_f64();
        self.tokens = elapsed.mul_add(refill_per_sec, self.tokens).min(capacity);
        self.last = now;
    }

    /// Refill for the time since the last call, then spend one token if there is one.
    pub fn take(&mut self, capacity: f64, refill_per_sec: f64, now: Instant) -> bool {
        self.refill(capacity, refill_per_sec, now);
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

/// Token buckets keyed by user id.
pub struct RateLimiter {
    capacity: f64,
    refill_per_sec: f64,
    buckets: Mutex<HashMap<String, Bucket>>,
}

impl RateLimiter {
    pub fn new(capacity: f64, refill_per_sec: f64) -> Self {
        Self {
            capacity,
            refill_per_sec,
            buckets: Mutex::new(HashMap::new()),
        }
    }

    /// Spend one of `key`'s searches at `now`; `false` when they have none left.
    pub fn try_take(&self, key: &str, now: Instant) -> bool {
        let mut buckets = self.buckets.lock().unwrap_or_else(PoisonError::into_inner);
        if buckets.len() >= MAX_TRACKED && !buckets.contains_key(key) {
            // A full bucket carries no state worth keeping.
            let (capacity, refill) = (self.capacity, self.refill_per_sec);
            buckets.retain(|_, b| {
                b.refill(capacity, refill, now);
                b.tokens < capacity
            });
        }
        buckets
            .entry(key.to_string())
            .or_insert_with(|| Bucket::full(self.capacity, now))
            .take(self.capacity, self.refill_per_sec, now)
    }
}
