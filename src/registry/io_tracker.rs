//! Atomic IO metric counters for task execution.

use std::sync::atomic::{AtomicI64, Ordering};

/// Accumulated IO metrics reported by the executor during execution.
///
/// Accessible via [`TaskContext::record_read_bytes`](super::TaskContext::record_read_bytes),
/// [`TaskContext::record_write_bytes`](super::TaskContext::record_write_bytes), etc. The scheduler reads the
/// final snapshot after the executor returns.
pub(crate) struct IoTracker {
    pub read_bytes: AtomicI64,
    pub write_bytes: AtomicI64,
    pub net_rx_bytes: AtomicI64,
    pub net_tx_bytes: AtomicI64,
}

impl IoTracker {
    pub fn new() -> Self {
        Self {
            read_bytes: AtomicI64::new(0),
            write_bytes: AtomicI64::new(0),
            net_rx_bytes: AtomicI64::new(0),
            net_tx_bytes: AtomicI64::new(0),
        }
    }

    pub fn snapshot(&self) -> crate::task::TaskMetrics {
        crate::task::TaskMetrics {
            read_bytes: self.read_bytes.load(Ordering::Relaxed),
            write_bytes: self.write_bytes.load(Ordering::Relaxed),
            net_rx_bytes: self.net_rx_bytes.load(Ordering::Relaxed),
            net_tx_bytes: self.net_tx_bytes.load(Ordering::Relaxed),
        }
    }
}
