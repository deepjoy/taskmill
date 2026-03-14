//! Deduplication key generation.

use sha2::{Digest, Sha256};

/// Maximum payload size in bytes (1 MiB).
pub const MAX_PAYLOAD_BYTES: usize = 1_048_576;

/// Generate a dedup key by hashing the task type and payload.
///
/// Produces a hex-encoded SHA-256 digest of `task_type` concatenated with
/// the payload bytes (or an empty slice when there is no payload).
pub fn generate_dedup_key(task_type: &str, payload: Option<&[u8]>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(task_type.as_bytes());
    hasher.update(b":");
    if let Some(p) = payload {
        hasher.update(p);
    }
    format!("{:x}", hasher.finalize())
}
