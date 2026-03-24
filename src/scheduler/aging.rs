//! Priority aging — anti-starvation for low-priority tasks.
//!
//! When enabled, the scheduler computes an *effective priority* for each
//! pending task at dispatch time. Tasks waiting longer than `grace_period`
//! are gradually promoted, preventing starvation when high-priority tasks
//! arrive continuously.
//!
//! The stored priority is never mutated — effective priority is a
//! dispatch-time computation visible in snapshots and events.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::priority::Priority;

/// Configuration for priority aging (anti-starvation).
///
/// When enabled, the scheduler computes an *effective priority* for each
/// pending task at dispatch time:
///
/// ```text
/// age = now - created_at - pause_duration
/// promotions = max(0, (age - grace_period) / aging_interval)
/// effective = max(base_priority - promotions, max_effective_priority)
/// ```
///
/// Lower numeric value = higher priority. Aging *decreases* the numeric
/// value (promotes the task). `max_effective_priority` is the floor
/// (highest priority a task can age into).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgingConfig {
    /// Time a task must wait before aging begins. Default: 5 minutes.
    pub grace_period: Duration,
    /// Interval between each one-step priority promotion. Default: 60 seconds.
    pub aging_interval: Duration,
    /// Priority ceiling — tasks cannot age above this level.
    /// Default: `Priority::HIGH` (64). Use `Priority::REALTIME` to allow
    /// aging to the absolute highest level.
    pub max_effective_priority: Priority,
    /// When effective priority reaches this level, the task may bypass
    /// group weight allocation (dispatched from the global pool).
    /// `None` disables the urgent override. Default: `None`.
    ///
    /// **Invariant:** Must be `<= max_effective_priority` numerically
    /// (i.e. at least as high priority), since tasks cannot age past
    /// `max_effective_priority`. Validated at build time.
    pub urgent_threshold: Option<Priority>,
}

impl Default for AgingConfig {
    fn default() -> Self {
        Self {
            grace_period: Duration::from_secs(300),
            aging_interval: Duration::from_secs(60),
            max_effective_priority: Priority::HIGH,
            urgent_threshold: None,
        }
    }
}

impl AgingConfig {
    /// Validate configuration invariants. Called by `SchedulerBuilder::build()`.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.aging_interval.is_zero() {
            return Err("aging_interval must be non-zero");
        }
        if let Some(urgent) = self.urgent_threshold {
            // Lower numeric value = higher priority. urgent_threshold must
            // be reachable: its numeric value >= max_effective_priority.
            if urgent.value() < self.max_effective_priority.value() {
                return Err(
                    "urgent_threshold is higher priority than max_effective_priority — \
                     tasks can never age past max_effective_priority, so the \
                     urgent threshold would never trigger",
                );
            }
        }
        Ok(())
    }
}

/// Bind parameters for the aging ORDER BY clause.
/// Computed once per dispatch cycle, passed to peek/pop queries.
pub struct AgingParams {
    pub now_ms: i64,
    pub grace_period_ms: i64,
    pub aging_interval_ms: i64,
    pub max_effective_priority: i64,
}

impl AgingParams {
    pub fn from_config(config: &AgingConfig) -> Self {
        Self {
            now_ms: chrono::Utc::now().timestamp_millis(),
            grace_period_ms: config.grace_period.as_millis() as i64,
            aging_interval_ms: config.aging_interval.as_millis().max(1) as i64,
            max_effective_priority: config.max_effective_priority.value() as i64,
        }
    }
}

/// Compute effective priority for a task record (used in events, snapshots,
/// not in the dispatch hot path which uses SQL).
pub fn effective_priority(
    base: Priority,
    created_at_ms: i64,
    pause_duration_ms: i64,
    config: &AgingConfig,
) -> Priority {
    let now_ms = chrono::Utc::now().timestamp_millis();
    let age_ms = now_ms - created_at_ms - pause_duration_ms;
    let grace_ms = config.grace_period.as_millis() as i64;
    let interval_ms = config.aging_interval.as_millis().max(1) as i64;
    let promotions = ((age_ms - grace_ms).max(0) / interval_ms).min(255) as u8;
    let effective = base.value().saturating_sub(promotions);
    let floor = config.max_effective_priority.value();
    Priority::new(effective.max(floor))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_priority_within_grace() {
        let config = AgingConfig {
            grace_period: Duration::from_secs(300),
            aging_interval: Duration::from_secs(60),
            max_effective_priority: Priority::HIGH,
            urgent_threshold: None,
        };
        // Task created just now — within grace period.
        let now_ms = chrono::Utc::now().timestamp_millis();
        let result = effective_priority(Priority::IDLE, now_ms, 0, &config);
        assert_eq!(result.value(), Priority::IDLE.value());
    }

    #[test]
    fn effective_priority_one_promotion() {
        let config = AgingConfig {
            grace_period: Duration::from_secs(300),
            aging_interval: Duration::from_secs(60),
            max_effective_priority: Priority::HIGH,
            urgent_threshold: None,
        };
        // Task created 361 seconds ago (grace 300 + 1 interval of 60 + 1).
        let now_ms = chrono::Utc::now().timestamp_millis();
        let created_at_ms = now_ms - 361_000;
        let result = effective_priority(Priority::IDLE, created_at_ms, 0, &config);
        assert_eq!(result.value(), Priority::IDLE.value() - 1);
    }

    #[test]
    fn effective_priority_capped_at_max() {
        let config = AgingConfig {
            grace_period: Duration::from_secs(0),
            aging_interval: Duration::from_secs(1),
            max_effective_priority: Priority::HIGH,
            urgent_threshold: None,
        };
        // Task created long ago — many promotions, but capped.
        let now_ms = chrono::Utc::now().timestamp_millis();
        let created_at_ms = now_ms - 1_000_000_000; // ~31 years
        let result = effective_priority(Priority::IDLE, created_at_ms, 0, &config);
        assert_eq!(result.value(), Priority::HIGH.value());
    }

    #[test]
    fn effective_priority_with_pause_duration() {
        let config = AgingConfig {
            grace_period: Duration::from_secs(300),
            aging_interval: Duration::from_secs(60),
            max_effective_priority: Priority::HIGH,
            urgent_threshold: None,
        };
        // Task created 400s ago but paused for 200s → effective age = 200s < grace.
        let now_ms = chrono::Utc::now().timestamp_millis();
        let created_at_ms = now_ms - 400_000;
        let result = effective_priority(Priority::IDLE, created_at_ms, 200_000, &config);
        assert_eq!(result.value(), Priority::IDLE.value());
    }

    #[test]
    fn default_config_values() {
        let config = AgingConfig::default();
        assert_eq!(config.grace_period, Duration::from_secs(300));
        assert_eq!(config.aging_interval, Duration::from_secs(60));
        assert_eq!(
            config.max_effective_priority.value(),
            Priority::HIGH.value()
        );
        assert!(config.urgent_threshold.is_none());
    }

    #[test]
    fn validate_rejects_unreachable_urgent() {
        let config = AgingConfig {
            urgent_threshold: Some(Priority::REALTIME), // value 0 < HIGH value 64
            max_effective_priority: Priority::HIGH,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_rejects_zero_interval() {
        let config = AgingConfig {
            aging_interval: Duration::ZERO,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }
}
