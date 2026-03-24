//! Token-bucket rate limiting for task dispatch.
//!
//! Rate limits control how many tasks *start per unit of time*, complementing
//! concurrency limits which control how many run *simultaneously*. A
//! [`RateLimit`] configures the steady-state rate and burst capacity; the
//! scheduler enforces it via [`RateLimits`] collections scoped by task type
//! and/or group key.

use std::collections::HashMap;
use std::time::Duration;

use tokio::time::Instant;

// ── Configuration ────────────────────────────────────────────────────

/// Rate limit configuration.
///
/// Defines the steady-state rate (`permits` per `interval`) and optional
/// burst capacity. Use the convenience constructors [`per_second`](Self::per_second)
/// and [`per_minute`](Self::per_minute) for common patterns.
///
/// # Examples
///
/// ```
/// use taskmill::RateLimit;
///
/// // 100 requests per second, no burst beyond steady rate.
/// let limit = RateLimit::per_second(100);
///
/// // 10 per minute with a burst of 20 (allows short spikes).
/// let limit = RateLimit::per_minute(10).with_burst(20);
/// ```
#[derive(Debug, Clone)]
pub struct RateLimit {
    /// Maximum tokens replenished per `interval`.
    pub permits: u32,
    /// Replenishment interval.
    pub interval: Duration,
    /// Burst capacity — max tokens the bucket can hold.
    /// Defaults to `permits` (no burst beyond steady rate).
    pub burst: u32,
}

impl RateLimit {
    /// Create a rate limit of `n` permits per second.
    pub fn per_second(n: u32) -> Self {
        Self {
            permits: n,
            interval: Duration::from_secs(1),
            burst: n,
        }
    }

    /// Create a rate limit of `n` permits per minute.
    pub fn per_minute(n: u32) -> Self {
        Self {
            permits: n,
            interval: Duration::from_secs(60),
            burst: n,
        }
    }

    /// Set the burst capacity (max tokens the bucket can hold).
    ///
    /// Burst allows short spikes above the steady-state rate. For example,
    /// `RateLimit::per_second(10).with_burst(20)` allows 20 immediate
    /// dispatches followed by a sustained 10/sec.
    pub fn with_burst(mut self, burst: u32) -> Self {
        self.burst = burst;
        self
    }
}

// ── Token Bucket ─────────────────────────────────────────────────────

/// Classic token-bucket: tracks available tokens and last refill time.
///
/// All methods take `&mut self`. Thread safety is provided by the
/// `Mutex` at the [`RateLimits`] collection level.
pub(crate) struct TokenBucket {
    /// Fractional tokens for smooth refill.
    available: f64,
    /// Max capacity (burst).
    burst: f64,
    /// Tokens per nanosecond.
    rate: f64,
    /// Last time tokens were refilled.
    last_refill: Instant,
    /// Original config — kept for snapshot reporting.
    config: RateLimit,
}

impl TokenBucket {
    fn new(config: &RateLimit) -> Self {
        let rate = config.permits as f64 / config.interval.as_nanos() as f64;
        Self {
            available: config.burst as f64,
            burst: config.burst as f64,
            rate,
            last_refill: Instant::now(),
            config: config.clone(),
        }
    }

    /// Refill tokens based on elapsed time since last refill.
    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed_nanos = now.duration_since(self.last_refill).as_nanos() as f64;
        self.available = (self.available + elapsed_nanos * self.rate).min(self.burst);
        self.last_refill = now;
    }

    /// Try to consume one token. Returns `Ok(())` if granted,
    /// `Err(next_available)` with the `Instant` the next token
    /// will be available if denied.
    fn try_acquire(&mut self) -> Result<(), Instant> {
        self.refill();
        if self.available >= 1.0 {
            self.available -= 1.0;
            Ok(())
        } else {
            // Compute when the next token will be available.
            let deficit = 1.0 - self.available;
            let wait_nanos = deficit / self.rate;
            Err(Instant::now() + Duration::from_nanos(wait_nanos as u64))
        }
    }

    /// Update the bucket's rate and burst from a new config.
    /// Preserves current token count (clamped to new burst).
    fn reconfigure(&mut self, config: &RateLimit) {
        let new_rate = config.permits as f64 / config.interval.as_nanos() as f64;
        let new_burst = config.burst as f64;
        self.refill(); // settle tokens before changing params
        self.rate = new_rate;
        self.burst = new_burst;
        self.available = self.available.min(new_burst);
        self.config = config.clone();
    }
}

// ── Rate Limits Collection ───────────────────────────────────────────

/// Per-scope rate limit collection.
///
/// Maps scope keys (task-type prefix or group key) to token buckets.
/// Thread-safe: a single `Mutex` guards the inner `HashMap`.
pub struct RateLimits {
    inner: std::sync::Mutex<HashMap<String, TokenBucket>>,
}

impl Default for RateLimits {
    fn default() -> Self {
        Self::new()
    }
}

impl RateLimits {
    /// Create a new empty collection.
    pub fn new() -> Self {
        Self {
            inner: std::sync::Mutex::new(HashMap::new()),
        }
    }

    /// Try to acquire a token for `scope`. Returns:
    /// - `None` if no rate limit is configured for this scope (always pass).
    /// - `Some(Ok(()))` if a token was granted.
    /// - `Some(Err(next))` if denied, with the Instant the next token arrives.
    pub fn try_acquire(&self, scope: &str) -> Option<Result<(), Instant>> {
        let mut map = self.inner.lock().unwrap();
        let bucket = map.get_mut(scope)?;
        Some(bucket.try_acquire())
    }

    /// Set or update the rate limit for a scope.
    /// If a bucket already exists, reconfigure in-place (preserving tokens).
    pub fn set(&self, scope: String, config: RateLimit) {
        let mut map = self.inner.lock().unwrap();
        if let Some(bucket) = map.get_mut(&scope) {
            bucket.reconfigure(&config);
        } else {
            map.insert(scope, TokenBucket::new(&config));
        }
    }

    /// Remove the rate limit for a scope.
    pub fn remove(&self, scope: &str) {
        self.inner.lock().unwrap().remove(scope);
    }

    /// Returns true if any rate limits are configured.
    pub fn is_empty(&self) -> bool {
        self.inner.lock().unwrap().is_empty()
    }

    /// Earliest instant at which any bucket will have a token available.
    /// Returns `None` if no buckets are depleted (or no limits configured).
    pub fn next_available(&self) -> Option<Instant> {
        let mut map = self.inner.lock().unwrap();
        let mut earliest: Option<Instant> = None;
        for bucket in map.values_mut() {
            bucket.refill();
            if bucket.available < 1.0 {
                let deficit = 1.0 - bucket.available;
                let wait_nanos = deficit / bucket.rate;
                let ready_at = Instant::now() + Duration::from_nanos(wait_nanos as u64);
                earliest = Some(match earliest {
                    Some(e) => e.min(ready_at),
                    None => ready_at,
                });
            }
        }
        earliest
    }

    /// Snapshot of all configured rate limits with current utilization.
    pub fn snapshot_info(&self, scope_kind: &str) -> Vec<RateLimitInfo> {
        let mut map = self.inner.lock().unwrap();
        let mut infos = Vec::with_capacity(map.len());
        for (scope, bucket) in map.iter_mut() {
            bucket.refill();
            infos.push(RateLimitInfo {
                scope: format!("{scope_kind}:{scope}"),
                scope_kind: scope_kind.to_string(),
                permits: bucket.config.permits,
                interval_ms: bucket.config.interval.as_millis() as u64,
                burst: bucket.config.burst,
                available_tokens: bucket.available,
            });
        }
        infos
    }
}

// ── Snapshot Info ─────────────────────────────────────────────────────

/// Configured rate limit with current utilization, for snapshot reporting.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RateLimitInfo {
    /// Scope identifier, e.g. `"type:media::upload"` or `"group:s3://b2"`.
    pub scope: String,
    /// Scope kind: `"type"` or `"group"`.
    pub scope_kind: String,
    /// Permits per interval.
    pub permits: u32,
    /// Interval in milliseconds.
    pub interval_ms: u64,
    /// Burst capacity.
    pub burst: u32,
    /// Current available tokens.
    pub available_tokens: f64,
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bucket_allows_up_to_burst() {
        let config = RateLimit::per_second(10).with_burst(5);
        let mut bucket = TokenBucket::new(&config);
        for _ in 0..5 {
            assert!(bucket.try_acquire().is_ok());
        }
    }

    #[test]
    fn bucket_denies_after_burst() {
        let config = RateLimit::per_second(10).with_burst(3);
        let mut bucket = TokenBucket::new(&config);
        for _ in 0..3 {
            assert!(bucket.try_acquire().is_ok());
        }
        let result = bucket.try_acquire();
        assert!(result.is_err());
        // The returned instant should be in the future.
        let next = result.unwrap_err();
        assert!(next > Instant::now() || next == Instant::now());
    }

    #[tokio::test]
    async fn bucket_refills_over_time() {
        let config = RateLimit::per_second(10); // 10 tokens/sec, burst=10
        let mut bucket = TokenBucket::new(&config);
        // Drain all tokens.
        for _ in 0..10 {
            assert!(bucket.try_acquire().is_ok());
        }
        assert!(bucket.try_acquire().is_err());
        // Wait for ~1 token worth of time (100ms for 10/sec).
        tokio::time::sleep(Duration::from_millis(120)).await;
        assert!(bucket.try_acquire().is_ok());
    }

    #[test]
    fn reconfigure_preserves_tokens() {
        let config = RateLimit::per_second(10).with_burst(10);
        let mut bucket = TokenBucket::new(&config);
        // Use 5 tokens, leaving 5.
        for _ in 0..5 {
            bucket.try_acquire().unwrap();
        }
        // Reconfigure to burst=3 — tokens clamped to 3.
        let new_config = RateLimit::per_second(20).with_burst(3);
        bucket.reconfigure(&new_config);
        assert!(bucket.available <= 3.0);
        // Should still be able to acquire (3 available).
        assert!(bucket.try_acquire().is_ok());
    }

    #[test]
    fn collection_pass_unconfigured() {
        let limits = RateLimits::new();
        assert!(limits.try_acquire("unknown").is_none());
    }

    #[test]
    fn snapshot_info_returns_state() {
        let limits = RateLimits::new();
        limits.set("media::upload".to_string(), RateLimit::per_second(100));

        let infos = limits.snapshot_info("type");
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].scope, "type:media::upload");
        assert_eq!(infos[0].scope_kind, "type");
        assert_eq!(infos[0].permits, 100);
        assert_eq!(infos[0].burst, 100);
        assert!(infos[0].available_tokens > 0.0);
    }
}
