//! System resource monitoring for IO-aware scheduling.
//!
//! Implement [`ResourceSampler`] to feed CPU, disk IO, and network throughput
//! metrics into the scheduler, or use the built-in [`sysinfo_monitor`] module
//! (enabled by the `sysinfo-monitor` feature) for cross-platform monitoring.
//! Enable via [`SchedulerBuilder::with_resource_monitoring`](crate::SchedulerBuilder::with_resource_monitoring)
//! or provide a custom sampler with
//! [`SchedulerBuilder::resource_sampler`](crate::SchedulerBuilder::resource_sampler).
//!
//! The scheduler reads the latest EWMA-smoothed snapshot via [`ResourceReader`]
//! when making IO-budget dispatch decisions. For network-aware throttling, use
//! [`SchedulerBuilder::bandwidth_limit`](crate::SchedulerBuilder::bandwidth_limit)
//! to register a built-in [`NetworkPressure`](network_pressure::NetworkPressure) source.

pub mod sampler;

pub mod network_pressure;

#[cfg(feature = "sysinfo-monitor")]
pub mod sysinfo_monitor;

use serde::{Deserialize, Serialize};

/// Point-in-time snapshot of system resource utilization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSnapshot {
    /// CPU utilization 0.0 to 1.0 (EWMA-smoothed).
    pub cpu_usage: f64,
    /// Disk read throughput in bytes/sec (EWMA-smoothed).
    pub io_read_bytes_per_sec: f64,
    /// Disk write throughput in bytes/sec (EWMA-smoothed).
    pub io_write_bytes_per_sec: f64,
    /// Network receive throughput in bytes/sec (EWMA-smoothed).
    pub net_rx_bytes_per_sec: f64,
    /// Network transmit throughput in bytes/sec (EWMA-smoothed).
    pub net_tx_bytes_per_sec: f64,
}

impl Default for ResourceSnapshot {
    fn default() -> Self {
        Self {
            cpu_usage: 0.0,
            io_read_bytes_per_sec: 0.0,
            io_write_bytes_per_sec: 0.0,
            net_rx_bytes_per_sec: 0.0,
            net_tx_bytes_per_sec: 0.0,
        }
    }
}

/// Trait for sampling raw system resources.
///
/// Implementations read platform-specific counters and return raw deltas.
/// The sampler loop handles EWMA smoothing separately.
///
/// To override the built-in monitor (e.g. for container cgroup-aware monitoring),
/// implement this trait and pass it via
/// [`SchedulerBuilder::resource_sampler`](crate::SchedulerBuilder::resource_sampler).
///
/// # Example
///
/// ```ignore
/// use taskmill::{ResourceSampler, ResourceSnapshot};
///
/// struct CgroupSampler { /* ... */ }
///
/// impl ResourceSampler for CgroupSampler {
///     fn sample(&mut self) -> ResourceSnapshot {
///         // Read from /sys/fs/cgroup/...
///         ResourceSnapshot {
///             cpu_usage: 0.42,
///             io_read_bytes_per_sec: 50_000_000.0,
///             io_write_bytes_per_sec: 20_000_000.0,
///         }
///     }
/// }
/// ```
pub trait ResourceSampler: Send + Sync + 'static {
    /// Take a raw sample. Called periodically by the sampler loop.
    /// Returns a snapshot with absolute values (not smoothed — the sampler
    /// applies EWMA).
    fn sample(&mut self) -> ResourceSnapshot;
}

/// Read-only access to the latest smoothed resource snapshot.
///
/// This is the interface consumed by the scheduler for IO budget decisions.
/// The sampler loop updates it; the scheduler reads it. Separating this from
/// [`ResourceSampler`] keeps the public API clean — consumers only see the
/// latest reading, never the sampling mechanics.
pub trait ResourceReader: Send + Sync + 'static {
    /// The most recent smoothed snapshot.
    fn latest(&self) -> ResourceSnapshot;
}

/// Create the platform-appropriate sampler.
///
/// Uses `sysinfo` for cross-platform CPU and disk IO monitoring on
/// Linux, macOS, and Windows.
///
/// Only available with the `sysinfo-monitor` feature (enabled by default).
#[cfg(feature = "sysinfo-monitor")]
pub fn platform_sampler() -> Box<dyn ResourceSampler> {
    Box::new(sysinfo_monitor::SysinfoSampler::new())
}
