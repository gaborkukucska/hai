//! # START OF FILE hainet-persona/src/agents/failover.rs
//! Model Failover Handler — Ported from TrippleEffect's failover_handler.py
//!
//! Multi-level failover chain:
//! 1. Same model on alternate API key
//! 2. Alternative model on same provider
//! 3. External provider (OpenRouter, etc.)
//!
//! From TE: Tracks failed model/key combos to avoid retrying. Quarantines
//! keys that return auth errors (401, 403, 429).

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use tracing::{info, warn, error};

/// Duration to quarantine a failed API key before retrying
const KEY_QUARANTINE_DURATION: Duration = Duration::from_secs(300); // 5 minutes

/// Maximum failover attempts before giving up
const MAX_FAILOVER_ATTEMPTS: u32 = 5;

/// A provider+model+key combination
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct ModelEndpoint {
    pub provider: String,
    pub model: String,
    pub api_key_id: Option<String>,
}

impl std::fmt::Display for ModelEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.api_key_id {
            Some(key) => write!(f, "{}/{} (key: {})", self.provider, self.model, key),
            None => write!(f, "{}/{}", self.provider, self.model),
        }
    }
}

/// Quarantine record for a failed endpoint
#[derive(Debug, Clone)]
struct QuarantineEntry {
    endpoint: ModelEndpoint,
    reason: String,
    quarantined_at: Instant,
    duration: Duration,
}

/// Failover chain manager
/// (From TE: FailoverHandler class)
pub struct FailoverHandler {
    /// Ordered list of fallback endpoints
    failover_chain: Vec<ModelEndpoint>,
    /// Currently active endpoint
    active_endpoint: Option<ModelEndpoint>,
    /// Quarantined endpoints (temporarily disabled)
    quarantined: HashMap<ModelEndpoint, QuarantineEntry>,
    /// Permanently failed endpoints this session
    failed_this_session: HashSet<ModelEndpoint>,
    /// Failover attempt counter
    attempt_count: u32,
}

impl FailoverHandler {
    pub fn new() -> Self {
        Self {
            failover_chain: Vec::new(),
            active_endpoint: None,
            quarantined: HashMap::new(),
            failed_this_session: HashSet::new(),
            attempt_count: 0,
        }
    }

    /// Add an endpoint to the failover chain
    pub fn add_endpoint(&mut self, endpoint: ModelEndpoint) {
        info!(endpoint = %endpoint, "Adding endpoint to failover chain");
        self.failover_chain.push(endpoint);
    }

    /// Set the currently active endpoint
    pub fn set_active(&mut self, endpoint: ModelEndpoint) {
        self.active_endpoint = Some(endpoint);
        self.attempt_count = 0;
    }

    /// Report a transient failure (timeout, 5xx) — try next endpoint
    pub fn report_transient_failure(&mut self, endpoint: &ModelEndpoint, error: &str) -> Option<ModelEndpoint> {
        warn!(
            endpoint = %endpoint,
            error,
            attempt = self.attempt_count,
            "Transient failure — attempting failover"
        );
        self.attempt_count += 1;

        if self.attempt_count >= MAX_FAILOVER_ATTEMPTS {
            error!("Max failover attempts ({}) reached", MAX_FAILOVER_ATTEMPTS);
            return None;
        }

        self.select_next_available(endpoint)
    }

    /// Report a key-related failure (401, 403, 429) — quarantine the key
    /// (From TE: KEY_RELATED_ERRORS handling)
    pub fn report_key_failure(&mut self, endpoint: &ModelEndpoint, error: &str) {
        warn!(
            endpoint = %endpoint,
            error,
            "Key-related failure — quarantining endpoint"
        );

        self.quarantined.insert(
            endpoint.clone(),
            QuarantineEntry {
                endpoint: endpoint.clone(),
                reason: error.to_string(),
                quarantined_at: Instant::now(),
                duration: KEY_QUARANTINE_DURATION,
            },
        );
    }

    /// Report permanent failure — remove from chain for this session
    pub fn report_permanent_failure(&mut self, endpoint: &ModelEndpoint, error: &str) {
        error!(
            endpoint = %endpoint,
            error,
            "Permanent failure — removing endpoint from chain"
        );
        self.failed_this_session.insert(endpoint.clone());
    }

    /// Select the next available endpoint (skipping quarantined and failed)
    fn select_next_available(&mut self, current: &ModelEndpoint) -> Option<ModelEndpoint> {
        // Clean up expired quarantines
        self.quarantined.retain(|_, entry| {
            entry.quarantined_at.elapsed() < entry.duration
        });

        // Find current position in chain
        let current_idx = self.failover_chain.iter().position(|e| e == current);

        // Try endpoints after current, then wrap around
        let chain_len = self.failover_chain.len();
        for offset in 1..=chain_len {
            let idx = (current_idx.unwrap_or(0) + offset) % chain_len;
            let candidate = &self.failover_chain[idx];

            if !self.quarantined.contains_key(candidate)
                && !self.failed_this_session.contains(candidate)
            {
                info!(endpoint = %candidate, "Failing over to next endpoint");
                let selected = candidate.clone();
                self.active_endpoint = Some(selected.clone());
                return Some(selected);
            }
        }

        warn!("No available endpoints for failover");
        None
    }

    /// Get the currently active endpoint
    pub fn active(&self) -> Option<&ModelEndpoint> {
        self.active_endpoint.as_ref()
    }

    /// Get count of available (non-quarantined, non-failed) endpoints
    pub fn available_count(&self) -> usize {
        self.quarantined.retain_expired();
        self.failover_chain
            .iter()
            .filter(|e| {
                !self.quarantined.contains_key(e) && !self.failed_this_session.contains(e)
            })
            .count()
    }
}

impl Default for FailoverHandler {
    fn default() -> Self {
        Self::new()
    }
}

// Helper trait for HashMap quarantine cleanup
trait RetainExpired {
    fn retain_expired(&self);
}

// Note: This is a no-op trait to keep the available_count method clean.
// Actual cleanup happens in select_next_available.
impl RetainExpired for HashMap<ModelEndpoint, QuarantineEntry> {
    fn retain_expired(&self) {
        // Cleanup is done in select_next_available via retain()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_endpoint(provider: &str, model: &str) -> ModelEndpoint {
        ModelEndpoint {
            provider: provider.to_string(),
            model: model.to_string(),
            api_key_id: None,
        }
    }

    #[test]
    fn test_failover_chain() {
        let mut handler = FailoverHandler::new();
        let ep1 = make_endpoint("ollama", "llama3");
        let ep2 = make_endpoint("ollama", "mistral");
        let ep3 = make_endpoint("openrouter", "llama3-70b");

        handler.add_endpoint(ep1.clone());
        handler.add_endpoint(ep2.clone());
        handler.add_endpoint(ep3.clone());
        handler.set_active(ep1.clone());

        // Transient failure on ep1 → should get ep2
        let next = handler.report_transient_failure(&ep1, "timeout");
        assert_eq!(next, Some(ep2.clone()));
    }

    #[test]
    fn test_quarantine_skips_endpoint() {
        let mut handler = FailoverHandler::new();
        let ep1 = make_endpoint("ollama", "llama3");
        let ep2 = make_endpoint("ollama", "mistral");
        let ep3 = make_endpoint("openrouter", "llama3-70b");

        handler.add_endpoint(ep1.clone());
        handler.add_endpoint(ep2.clone());
        handler.add_endpoint(ep3.clone());
        handler.set_active(ep1.clone());

        // Quarantine ep2
        handler.report_key_failure(&ep2, "401 auth error");

        // Failover from ep1 should skip ep2, go to ep3
        let next = handler.report_transient_failure(&ep1, "timeout");
        assert_eq!(next, Some(ep3.clone()));
    }

    #[test]
    fn test_max_attempts_exhausted() {
        let mut handler = FailoverHandler::new();
        let ep1 = make_endpoint("ollama", "llama3");
        handler.add_endpoint(ep1.clone());
        handler.set_active(ep1.clone());

        // Exhaust attempts
        for _ in 0..MAX_FAILOVER_ATTEMPTS {
            handler.report_transient_failure(&ep1, "timeout");
        }

        // Should return None now
        assert!(handler.report_transient_failure(&ep1, "timeout").is_none());
    }
}
