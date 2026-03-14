//! Cross-platform resource sampler using the [`sysinfo`](https://docs.rs/sysinfo) crate.
//!
//! Tracks CPU utilization, aggregate disk IO throughput across all mounted
//! disks, and aggregate network IO throughput across all network interfaces.
//! Gated behind the `sysinfo-monitor` feature (enabled by default).

use std::time::Instant;

use sysinfo::{Disks, Networks, System};

use crate::resource::{ResourceSampler, ResourceSnapshot};

/// Cross-platform resource sampler using the `sysinfo` crate.
///
/// Works on Linux, macOS, and Windows. Tracks CPU utilization,
/// aggregate disk IO throughput across all mounted disks, and
/// aggregate network IO throughput across all network interfaces.
pub struct SysinfoSampler {
    sys: System,
    disks: Disks,
    networks: Networks,
    prev_read_bytes: u64,
    prev_write_bytes: u64,
    prev_net_rx_bytes: u64,
    prev_net_tx_bytes: u64,
    prev_sample: Option<Instant>,
}

impl SysinfoSampler {
    /// Create a new sampler, taking an initial CPU, disk, and network reading.
    pub fn new() -> Self {
        let mut sys = System::new();
        sys.refresh_cpu_usage();

        let disks = Disks::new_with_refreshed_list();
        let networks = Networks::new_with_refreshed_list();

        // Take initial totals so first delta is meaningful.
        let (read, write) = disk_totals(&disks);
        let (net_rx, net_tx) = network_totals(&networks);

        Self {
            sys,
            disks,
            networks,
            prev_read_bytes: read,
            prev_write_bytes: write,
            prev_net_rx_bytes: net_rx,
            prev_net_tx_bytes: net_tx,
            prev_sample: Some(Instant::now()),
        }
    }
}

impl Default for SysinfoSampler {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceSampler for SysinfoSampler {
    fn sample(&mut self) -> ResourceSnapshot {
        // CPU: sysinfo needs two refresh calls to compute usage delta.
        self.sys.refresh_cpu_usage();
        let cpu_usage = self.sys.global_cpu_usage() as f64 / 100.0;

        // Disk IO: compute bytes/sec since last sample.
        self.disks.refresh(true);
        let (read_bytes, write_bytes) = disk_totals(&self.disks);

        // Network IO: compute bytes/sec since last sample.
        self.networks.refresh(true);
        let (net_rx_bytes, net_tx_bytes) = network_totals(&self.networks);

        let now = Instant::now();

        let (read_bps, write_bps, net_rx_bps, net_tx_bps) = if let Some(prev_ts) = self.prev_sample
        {
            let elapsed = now.duration_since(prev_ts).as_secs_f64();
            if elapsed > 0.0 {
                let read_delta = read_bytes.saturating_sub(self.prev_read_bytes);
                let write_delta = write_bytes.saturating_sub(self.prev_write_bytes);
                let rx_delta = net_rx_bytes.saturating_sub(self.prev_net_rx_bytes);
                let tx_delta = net_tx_bytes.saturating_sub(self.prev_net_tx_bytes);
                (
                    read_delta as f64 / elapsed,
                    write_delta as f64 / elapsed,
                    rx_delta as f64 / elapsed,
                    tx_delta as f64 / elapsed,
                )
            } else {
                (0.0, 0.0, 0.0, 0.0)
            }
        } else {
            (0.0, 0.0, 0.0, 0.0)
        };

        self.prev_read_bytes = read_bytes;
        self.prev_write_bytes = write_bytes;
        self.prev_net_rx_bytes = net_rx_bytes;
        self.prev_net_tx_bytes = net_tx_bytes;
        self.prev_sample = Some(now);

        ResourceSnapshot {
            cpu_usage,
            io_read_bytes_per_sec: read_bps,
            io_write_bytes_per_sec: write_bps,
            net_rx_bytes_per_sec: net_rx_bps,
            net_tx_bytes_per_sec: net_tx_bps,
        }
    }
}

/// Sum read/write bytes across all disks.
fn disk_totals(disks: &Disks) -> (u64, u64) {
    let mut total_read = 0u64;
    let mut total_write = 0u64;
    for disk in disks.list() {
        // sysinfo::Disk exposes usage(); total/available space but not IO counters
        // directly. We use the disk-level process IO as a proxy.
        // Note: sysinfo 0.33+ tracks disk IO via the Disks API on supported platforms.
        let usage = disk.usage();
        total_read += usage.read_bytes;
        total_write += usage.written_bytes;
    }
    (total_read, total_write)
}

/// Sum received/transmitted bytes across all network interfaces.
fn network_totals(networks: &Networks) -> (u64, u64) {
    let mut total_rx = 0u64;
    let mut total_tx = 0u64;
    for (_name, data) in networks {
        total_rx += data.received();
        total_tx += data.transmitted();
    }
    (total_rx, total_tx)
}
