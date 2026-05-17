//! # START OF FILE hainet-social/src/dedup.rs
//! Packet Deduplication — Ported from gChat's _processedPacketIds

use std::collections::HashMap;
use std::time::{Duration, Instant};

const DEDUP_WINDOW: Duration = Duration::from_secs(300); // 5 minutes

/// Time-windowed packet deduplication filter
pub struct PacketDedup {
    seen: HashMap<String, Instant>,
}

impl PacketDedup {
    pub fn new() -> Self {
        Self { seen: HashMap::new() }
    }

    /// Check if packet is new. Returns true if new (not seen), false if duplicate.
    pub fn check_and_mark(&mut self, packet_id: &str) -> bool {
        self.cleanup();
        if self.seen.contains_key(packet_id) {
            return false;
        }
        self.seen.insert(packet_id.to_string(), Instant::now());
        true
    }

    fn cleanup(&mut self) {
        let cutoff = Instant::now() - DEDUP_WINDOW;
        self.seen.retain(|_, ts| *ts > cutoff);
    }
}

impl Default for PacketDedup {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_first_packet_is_new() {
        let mut d = PacketDedup::new();
        assert!(d.check_and_mark("pkt-1"));
    }

    #[test]
    fn test_duplicate_is_detected() {
        let mut d = PacketDedup::new();
        assert!(d.check_and_mark("pkt-1"));
        assert!(!d.check_and_mark("pkt-1"));
    }
}
