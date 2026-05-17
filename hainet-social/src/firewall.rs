//! # START OF FILE hainet-social/src/firewall.rs
//! Trust Firewall — Ported from gChat v1.4.0

use std::collections::HashSet;
use tracing::debug;

/// Trust-based packet firewall
pub struct TrustFirewall {
    trusted_peers: HashSet<String>,
}

impl TrustFirewall {
    pub fn new() -> Self {
        Self { trusted_peers: HashSet::new() }
    }

    pub fn is_trusted(&self, peer_id: &str) -> bool {
        self.trusted_peers.contains(peer_id)
    }

    pub fn add_trusted(&mut self, peer_id: String) {
        debug!(peer_id, "Adding peer to trust list");
        self.trusted_peers.insert(peer_id);
    }

    pub fn remove_trusted(&mut self, peer_id: &str) {
        self.trusted_peers.remove(peer_id);
    }

    pub fn trusted_count(&self) -> usize {
        self.trusted_peers.len()
    }
}

impl Default for TrustFirewall {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_firewall_trusts_nobody() {
        let fw = TrustFirewall::new();
        assert!(!fw.is_trusted("anyone"));
    }

    #[test]
    fn test_add_and_check_trust() {
        let mut fw = TrustFirewall::new();
        fw.add_trusted("alice".to_string());
        assert!(fw.is_trusted("alice"));
        assert!(!fw.is_trusted("bob"));
    }
}
