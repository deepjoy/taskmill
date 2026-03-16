//! Built-in [`PressureSource`] derived from network bandwidth utilization.
//!
//! [`NetworkPressure`] reads the latest [`ResourceSnapshot`](crate::ResourceSnapshot)
//! and computes pressure as the ratio of observed network throughput to a
//! configured bandwidth cap. Register via
//! [`SchedulerBuilder::bandwidth_limit`](crate::SchedulerBuilder::bandwidth_limit).

use std::sync::Arc;

use crate::backpressure::PressureSource;
use crate::resource::ResourceReader;

/// Pressure source based on network bandwidth utilization.
///
/// Reads EWMA-smoothed RX+TX throughput from a [`ResourceReader`] and
/// returns `(rx + tx) / max_bandwidth_bps` clamped to `[0.0, 1.0]`.
///
/// # Example
///
/// ```ignore
/// use taskmill::resource::network_pressure::NetworkPressure;
///
/// let pressure = NetworkPressure::new(reader, 100_000_000.0); // 100 MB/s cap
/// assert!(pressure.pressure() >= 0.0);
/// ```
pub struct NetworkPressure {
    reader: Arc<dyn ResourceReader>,
    /// Combined RX+TX bandwidth limit in bytes/sec.
    max_bandwidth_bps: f64,
}

impl NetworkPressure {
    /// Create a new `NetworkPressure` source.
    ///
    /// `max_bandwidth_bps` is the combined (RX + TX) bandwidth cap in
    /// bytes per second. When observed throughput reaches this value,
    /// pressure reports 1.0 (fully saturated).
    pub fn new(reader: Arc<dyn ResourceReader>, max_bandwidth_bps: f64) -> Self {
        assert!(
            max_bandwidth_bps > 0.0,
            "max_bandwidth_bps must be positive"
        );
        Self {
            reader,
            max_bandwidth_bps,
        }
    }
}

impl PressureSource for NetworkPressure {
    fn pressure(&self) -> f32 {
        let snapshot = self.reader.latest();
        let total_bps = snapshot.net_rx_bytes_per_sec + snapshot.net_tx_bytes_per_sec;
        (total_bps / self.max_bandwidth_bps).min(1.0) as f32
    }

    fn name(&self) -> &str {
        "network"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource::ResourceSnapshot;

    struct FixedReader(ResourceSnapshot);

    impl ResourceReader for FixedReader {
        fn latest(&self) -> ResourceSnapshot {
            self.0.clone()
        }
    }

    #[test]
    fn pressure_scales_with_throughput() {
        let reader = Arc::new(FixedReader(ResourceSnapshot {
            net_rx_bytes_per_sec: 30_000_000.0,
            net_tx_bytes_per_sec: 20_000_000.0,
            ..Default::default()
        }));
        let pressure = NetworkPressure::new(reader, 100_000_000.0);
        assert!((pressure.pressure() - 0.5).abs() < 0.01);
    }

    #[test]
    fn pressure_clamps_to_one() {
        let reader = Arc::new(FixedReader(ResourceSnapshot {
            net_rx_bytes_per_sec: 80_000_000.0,
            net_tx_bytes_per_sec: 80_000_000.0,
            ..Default::default()
        }));
        let pressure = NetworkPressure::new(reader, 100_000_000.0);
        assert!((pressure.pressure() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn zero_throughput_is_zero_pressure() {
        let reader = Arc::new(FixedReader(ResourceSnapshot::default()));
        let pressure = NetworkPressure::new(reader, 100_000_000.0);
        assert_eq!(pressure.pressure(), 0.0);
    }
}
