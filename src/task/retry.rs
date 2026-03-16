//! Backoff strategies and retry policies for task failure handling.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Backoff strategy for retryable task failures.
///
/// Controls the delay between consecutive retries. The computed delay is
/// applied via the existing `run_after` mechanism — the task stays `pending`
/// but is invisible to the dispatch loop until the delay elapses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackoffStrategy {
    /// Fixed delay between every retry.
    Constant { delay: Duration },
    /// Linearly increasing delay: `initial + (retry_count * increment)`.
    Linear {
        initial: Duration,
        increment: Duration,
        max: Duration,
    },
    /// Exponential delay: `initial * multiplier^retry_count`, capped at `max`.
    Exponential {
        initial: Duration,
        max: Duration,
        multiplier: f64,
    },
    /// Exponential with full jitter: uniform random in `[0, computed_delay]`.
    ///
    /// Recommended for tasks hitting the same endpoint — decorrelates retry
    /// storms. Uses full jitter per AWS architecture blog recommendations.
    ExponentialJitter {
        initial: Duration,
        max: Duration,
        multiplier: f64,
    },
}

impl BackoffStrategy {
    /// Compute the delay for a given retry attempt (0-indexed: first retry = 0).
    pub fn delay_for(&self, retry_count: i32) -> Duration {
        match self {
            Self::Constant { delay } => *delay,
            Self::Linear {
                initial,
                increment,
                max,
            } => {
                let d = *initial + *increment * retry_count as u32;
                d.min(*max)
            }
            Self::Exponential {
                initial,
                max,
                multiplier,
            } => {
                let d = initial.mul_f64(multiplier.powi(retry_count));
                d.min(*max)
            }
            Self::ExponentialJitter {
                initial,
                max,
                multiplier,
            } => {
                let base = initial.mul_f64(multiplier.powi(retry_count)).min(*max);
                let base_ms = base.as_millis() as u64;
                if base_ms == 0 {
                    return Duration::ZERO;
                }
                Duration::from_millis(fastrand::u64(0..=base_ms))
            }
        }
    }
}

impl Default for BackoffStrategy {
    /// No backoff — immediate retry (preserves current behavior).
    fn default() -> Self {
        Self::Constant {
            delay: Duration::ZERO,
        }
    }
}

/// Per-task-type retry policy combining a backoff strategy with a retry limit.
///
/// Register via [`SchedulerBuilder::executor_with_retry_policy`](crate::SchedulerBuilder)
/// to apply to all tasks of a given type. Tasks without a per-type policy fall
/// back to the global [`SchedulerConfig::max_retries`](crate::SchedulerConfig::max_retries)
/// with no backoff delay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub strategy: BackoffStrategy,
    pub max_retries: i32,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            strategy: BackoffStrategy::default(),
            max_retries: 3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_returns_fixed_delay() {
        let strategy = BackoffStrategy::Constant {
            delay: Duration::from_secs(5),
        };
        assert_eq!(strategy.delay_for(0), Duration::from_secs(5));
        assert_eq!(strategy.delay_for(1), Duration::from_secs(5));
        assert_eq!(strategy.delay_for(5), Duration::from_secs(5));
        assert_eq!(strategy.delay_for(10), Duration::from_secs(5));
    }

    #[test]
    fn linear_increases_by_increment() {
        let strategy = BackoffStrategy::Linear {
            initial: Duration::from_secs(1),
            increment: Duration::from_secs(2),
            max: Duration::from_secs(20),
        };
        assert_eq!(strategy.delay_for(0), Duration::from_secs(1));
        assert_eq!(strategy.delay_for(1), Duration::from_secs(3));
        assert_eq!(strategy.delay_for(5), Duration::from_secs(11));
    }

    #[test]
    fn linear_clamps_at_max() {
        let strategy = BackoffStrategy::Linear {
            initial: Duration::from_secs(1),
            increment: Duration::from_secs(2),
            max: Duration::from_secs(10),
        };
        // retry_count=10 -> 1 + 2*10 = 21, clamped to 10
        assert_eq!(strategy.delay_for(10), Duration::from_secs(10));
    }

    #[test]
    fn exponential_doubles_by_default() {
        let strategy = BackoffStrategy::Exponential {
            initial: Duration::from_secs(1),
            max: Duration::from_secs(3600),
            multiplier: 2.0,
        };
        assert_eq!(strategy.delay_for(0), Duration::from_secs(1));
        assert_eq!(strategy.delay_for(1), Duration::from_secs(2));
        assert_eq!(strategy.delay_for(5), Duration::from_secs(32));
        assert_eq!(strategy.delay_for(10), Duration::from_secs(1024));
    }

    #[test]
    fn exponential_clamps_at_max() {
        let strategy = BackoffStrategy::Exponential {
            initial: Duration::from_secs(1),
            max: Duration::from_secs(60),
            multiplier: 2.0,
        };
        // retry_count=10 -> 1 * 2^10 = 1024, clamped to 60
        assert_eq!(strategy.delay_for(10), Duration::from_secs(60));
    }

    #[test]
    fn exponential_jitter_within_bounds() {
        let strategy = BackoffStrategy::ExponentialJitter {
            initial: Duration::from_secs(1),
            max: Duration::from_secs(3600),
            multiplier: 2.0,
        };
        // Run multiple times to check bounds (jitter is random).
        for _ in 0..100 {
            let d = strategy.delay_for(0);
            assert!(d <= Duration::from_secs(1), "got {:?}", d);

            let d = strategy.delay_for(1);
            assert!(d <= Duration::from_secs(2), "got {:?}", d);

            let d = strategy.delay_for(5);
            assert!(d <= Duration::from_secs(32), "got {:?}", d);
        }
    }

    #[test]
    fn exponential_jitter_clamps_at_max() {
        let strategy = BackoffStrategy::ExponentialJitter {
            initial: Duration::from_secs(1),
            max: Duration::from_secs(60),
            multiplier: 2.0,
        };
        for _ in 0..100 {
            let d = strategy.delay_for(10);
            assert!(d <= Duration::from_secs(60), "got {:?}", d);
        }
    }

    #[test]
    fn exponential_jitter_zero_initial() {
        let strategy = BackoffStrategy::ExponentialJitter {
            initial: Duration::ZERO,
            max: Duration::from_secs(60),
            multiplier: 2.0,
        };
        assert_eq!(strategy.delay_for(0), Duration::ZERO);
        assert_eq!(strategy.delay_for(5), Duration::ZERO);
    }

    #[test]
    fn default_strategy_is_zero_constant() {
        let strategy = BackoffStrategy::default();
        assert_eq!(strategy.delay_for(0), Duration::ZERO);
        assert_eq!(strategy.delay_for(10), Duration::ZERO);
    }

    #[test]
    fn default_retry_policy() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.max_retries, 3);
        assert_eq!(policy.strategy.delay_for(0), Duration::ZERO);
    }
}
