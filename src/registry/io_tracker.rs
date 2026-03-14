//! Atomic IO metric counters for task execution.

use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

/// Accumulated IO metrics reported by the executor during execution.
///
/// Accessible via [`TaskContext::record_read_bytes`](super::TaskContext::record_read_bytes),
/// [`TaskContext::record_write_bytes`](super::TaskContext::record_write_bytes), etc. The scheduler reads the
/// final snapshot after the executor returns.
///
/// Also carries byte-level progress atomics (`progress_bytes` / `total_bytes`)
/// for real-time transfer progress reporting via the dedicated progress channel.
pub(crate) struct IoTracker {
    pub read_bytes: AtomicI64,
    pub write_bytes: AtomicI64,
    pub net_rx_bytes: AtomicI64,
    pub net_tx_bytes: AtomicI64,
    /// Bytes completed so far (for byte-level progress reporting).
    progress_bytes: AtomicU64,
    /// Total bytes expected (`u64::MAX` = unknown).
    total_bytes: AtomicU64,
}

impl IoTracker {
    pub fn new() -> Self {
        Self {
            read_bytes: AtomicI64::new(0),
            write_bytes: AtomicI64::new(0),
            net_rx_bytes: AtomicI64::new(0),
            net_tx_bytes: AtomicI64::new(0),
            progress_bytes: AtomicU64::new(0),
            total_bytes: AtomicU64::new(u64::MAX),
        }
    }

    pub fn snapshot(&self) -> crate::task::IoBudget {
        crate::task::IoBudget {
            disk_read: self.read_bytes.load(Ordering::Relaxed),
            disk_write: self.write_bytes.load(Ordering::Relaxed),
            net_rx: self.net_rx_bytes.load(Ordering::Relaxed),
            net_tx: self.net_tx_bytes.load(Ordering::Relaxed),
        }
    }

    /// Set the total number of bytes expected.
    pub fn set_total(&self, total: u64) {
        self.total_bytes.store(total, Ordering::Relaxed);
    }

    /// Increment completed bytes by `delta`.
    pub fn add_progress(&self, delta: u64) {
        self.progress_bytes.fetch_add(delta, Ordering::Relaxed);
    }

    /// Set completed bytes to an absolute value.
    pub fn set_progress(&self, completed: u64) {
        self.progress_bytes.store(completed, Ordering::Relaxed);
    }

    /// Snapshot of byte-level progress: `(completed, Option<total>)`.
    ///
    /// Returns `None` for total when it is unknown (`u64::MAX` sentinel).
    pub fn progress_snapshot(&self) -> (u64, Option<u64>) {
        let completed = self.progress_bytes.load(Ordering::Relaxed);
        let total = self.total_bytes.load(Ordering::Relaxed);
        let total = if total == u64::MAX { None } else { Some(total) };
        (completed, total)
    }
}
