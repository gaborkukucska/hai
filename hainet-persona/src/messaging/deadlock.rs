// START OF FILE hainet-persona/src/messaging/deadlock.rs

//! Deadlock detection and prevention for agent communication
//!
//! This module implements cycle detection in the agent dependency graph
//! to prevent deadlocks where agents wait on each other indefinitely.
//! Constitutional compliance:
//! - Article III, Section 2: System resilience and availability
//! - Article II, Section 2: Human agency preserved through timeout enforcement
//!
//! Uses petgraph for efficient cycle detection and timeout enforcement
//! to ensure the system remains responsive.

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tracing::{debug, warn};

use super::types::{AgentId, MessageId};

/// Default timeout for requests (30 seconds)
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Metadata for tracking request lifecycle
#[derive(Debug, Clone)]
pub struct RequestMetadata {
    pub message_id: MessageId,
    pub requester: AgentId,
    pub responder: AgentId,
    pub started_at: SystemTime,
    pub depends_on: Vec<MessageId>,
}

/// Statistics for deadlock detector
#[derive(Debug, Clone, Default)]
pub struct DeadlockStats {
    pub active_requests: usize,
    pub completed_requests: u64,
    pub timed_out_requests: u64,
    pub cycles_detected: u64,
    pub stale_requests_cleaned: u64,
}

/// Deadlock detector using dependency graph analysis
///
/// NOTE: Full petgraph integration will be added when dependency is available.
/// Current implementation uses timeout-based detection and simple cycle checks.
pub struct DeadlockDetector {
    active_requests: Arc<RwLock<HashMap<MessageId, RequestMetadata>>>,
    timeout_duration: Duration,
    stats: Arc<RwLock<DeadlockStats>>,
}

impl DeadlockDetector {
    /// Create a new deadlock detector with default timeout
    pub fn new() -> Self {
        Self::with_timeout(DEFAULT_REQUEST_TIMEOUT)
    }

    /// Create a new deadlock detector with custom timeout
    pub fn with_timeout(timeout: Duration) -> Self {
        debug!("Initializing DeadlockDetector with timeout: {:?}", timeout);
        
        Self {
            active_requests: Arc::new(RwLock::new(HashMap::new())),
            timeout_duration: timeout,
            stats: Arc::new(RwLock::new(DeadlockStats::default())),
        }
    }

    /// Register a new request in the dependency graph
    pub async fn register_request(&self, metadata: RequestMetadata) -> Result<()> {
        let message_id = metadata.message_id;
        
        {
            let mut requests = self.active_requests.write().await;
            requests.insert(message_id, metadata);
        }

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.active_requests = self.active_requests.read().await.len();
        }

        debug!("Registered request: {}", message_id);
        Ok(())
    }

    /// Mark a request as completed
    pub async fn complete_request(&self, message_id: MessageId) -> Result<()> {
        {
            let mut requests = self.active_requests.write().await;
            requests.remove(&message_id);
        }

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.completed_requests += 1;
            stats.active_requests = self.active_requests.read().await.len();
        }

        debug!("Completed request: {}", message_id);
        Ok(())
    }

    /// Check for deadlocks in the dependency graph
    ///
    /// Returns true if a cycle is detected
    pub async fn detect_cycles(&self) -> Result<bool> {
        let requests = self.active_requests.read().await;
        
        if requests.is_empty() {
            return Ok(false);
        }

        // Build dependency graph (agent -> agents it's waiting on)
        let mut graph: HashMap<AgentId, Vec<AgentId>> = HashMap::new();
        
        for metadata in requests.values() {
            let dependencies: Vec<AgentId> = requests
                .values()
                .filter(|req| metadata.depends_on.contains(&req.message_id))
                .map(|req| req.responder.clone())
                .collect();
            
            graph.insert(metadata.requester.clone(), dependencies);
        }

        // Detect cycles using depth-first search
        let cycle_detected = self.has_cycle(&graph)?;

        if cycle_detected {
            warn!("Cycle detected in request dependency graph!");
            let mut stats = self.stats.write().await;
            stats.cycles_detected += 1;
        }

        Ok(cycle_detected)
    }

    /// Check for cycles using DFS
    ///
    /// NOTE: This is a simplified implementation. Full petgraph integration
    /// will provide more efficient cycle detection algorithms.
    fn has_cycle(&self, graph: &HashMap<AgentId, Vec<AgentId>>) -> Result<bool> {
        let mut visited = HashMap::new();
        let mut rec_stack = HashMap::new();

        for agent in graph.keys() {
            if !visited.contains_key(agent) {
                if self.dfs_has_cycle(agent, graph, &mut visited, &mut rec_stack) {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    /// DFS helper for cycle detection
    fn dfs_has_cycle(
        &self,
        agent: &AgentId,
        graph: &HashMap<AgentId, Vec<AgentId>>,
        visited: &mut HashMap<AgentId, bool>,
        rec_stack: &mut HashMap<AgentId, bool>,
    ) -> bool {
        visited.insert(agent.clone(), true);
        rec_stack.insert(agent.clone(), true);

        if let Some(neighbors) = graph.get(agent) {
            for neighbor in neighbors {
                if !visited.contains_key(neighbor) {
                    if self.dfs_has_cycle(neighbor, graph, visited, rec_stack) {
                        return true;
                    }
                } else if *rec_stack.get(neighbor).unwrap_or(&false) {
                    // Back edge found - cycle detected
                    return true;
                }
            }
        }

        rec_stack.insert(agent.clone(), false);
        false
    }

    /// Check for timed-out requests and clean them up
    pub async fn cleanup_stale_requests(&self) -> Result<Vec<MessageId>> {
        let now = SystemTime::now();
        let mut timed_out = Vec::new();

        {
            let requests = self.active_requests.read().await;
            
            for (msg_id, metadata) in requests.iter() {
                if let Ok(elapsed) = now.duration_since(metadata.started_at) {
                    if elapsed > self.timeout_duration {
                        timed_out.push(*msg_id);
                    }
                }
            }
        }

        // Remove timed-out requests
        if !timed_out.is_empty() {
            let mut requests = self.active_requests.write().await;
            for msg_id in &timed_out {
                requests.remove(msg_id);
            }

            // Update stats
            let mut stats = self.stats.write().await;
            stats.timed_out_requests += timed_out.len() as u64;
            stats.stale_requests_cleaned += timed_out.len() as u64;
            stats.active_requests = requests.len();

            warn!(
                "Cleaned up {} timed-out requests (timeout: {:?})",
                timed_out.len(),
                self.timeout_duration
            );
        }

        Ok(timed_out)
    }

    /// Get all active requests
    pub async fn get_active_requests(&self) -> Vec<RequestMetadata> {
        let requests = self.active_requests.read().await;
        requests.values().cloned().collect()
    }

    /// Get statistics
    pub async fn get_stats(&self) -> DeadlockStats {
        let mut stats = self.stats.read().await.clone();
        stats.active_requests = self.active_requests.read().await.len();
        stats
    }

    /// Reset statistics
    pub async fn reset_stats(&self) {
        let mut stats = self.stats.write().await;
        let active_count = self.active_requests.read().await.len();
        *stats = DeadlockStats {
            active_requests: active_count,
            ..Default::default()
        };
    }

    /// Clear all requests (for testing)
    pub async fn clear_all(&self) {
        let mut requests = self.active_requests.write().await;
        requests.clear();

        let mut stats = self.stats.write().await;
        stats.active_requests = 0;
    }

    /// Get timeout duration
    pub fn timeout(&self) -> Duration {
        self.timeout_duration
    }
}

impl Default for DeadlockDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messaging::types::AgentType;
    use std::thread::sleep;

    fn create_test_metadata(
        requester: AgentType,
        responder: AgentType,
        depends_on: Vec<MessageId>,
    ) -> RequestMetadata {
        RequestMetadata {
            message_id: MessageId::new(),
            requester: AgentId::new(requester, format!("{:?}-1", requester)),
            responder: AgentId::new(responder, format!("{:?}-1", responder)),
            started_at: SystemTime::now(),
            depends_on,
        }
    }

    #[tokio::test]
    async fn test_detector_creation() {
        let detector = DeadlockDetector::new();
        let stats = detector.get_stats().await;
        assert_eq!(stats.active_requests, 0);
    }

    #[tokio::test]
    async fn test_register_request() {
        let detector = DeadlockDetector::new();
        
        let metadata = create_test_metadata(AgentType::Admin, AgentType::PM, vec![]);
        detector.register_request(metadata).await.unwrap();

        let stats = detector.get_stats().await;
        assert_eq!(stats.active_requests, 1);
    }

    #[tokio::test]
    async fn test_complete_request() {
        let detector = DeadlockDetector::new();
        
        let metadata = create_test_metadata(AgentType::Admin, AgentType::PM, vec![]);
        let msg_id = metadata.message_id;
        
        detector.register_request(metadata).await.unwrap();
        assert_eq!(detector.get_stats().await.active_requests, 1);

        detector.complete_request(msg_id).await.unwrap();
        assert_eq!(detector.get_stats().await.active_requests, 0);
        
        let stats = detector.get_stats().await;
        assert_eq!(stats.completed_requests, 1);
    }

    #[tokio::test]
    async fn test_no_cycle_independent_requests() {
        let detector = DeadlockDetector::new();
        
        // Two independent requests
        let meta1 = create_test_metadata(AgentType::Admin, AgentType::PM, vec![]);
        let meta2 = create_test_metadata(AgentType::PM, AgentType::Worker, vec![]);
        
        detector.register_request(meta1).await.unwrap();
        detector.register_request(meta2).await.unwrap();

        let has_cycle = detector.detect_cycles().await.unwrap();
        assert!(!has_cycle);
    }

    #[tokio::test]
    async fn test_cycle_detection_simple() {
        let detector = DeadlockDetector::new();
        
        // Create circular dependency: A waits on B, B waits on A
        let msg_a = MessageId::new();
        let msg_b = MessageId::new();

        let admin_id = AgentId::new(AgentType::Admin, "admin-1".to_string());
        let pm_id = AgentId::new(AgentType::PM, "pm-1".to_string());

        let meta_a = RequestMetadata {
            message_id: msg_a,
            requester: admin_id.clone(),
            responder: pm_id.clone(),
            started_at: SystemTime::now(),
            depends_on: vec![msg_b],
        };

        let meta_b = RequestMetadata {
            message_id: msg_b,
            requester: pm_id,
            responder: admin_id,
            started_at: SystemTime::now(),
            depends_on: vec![msg_a],
        };

        detector.register_request(meta_a).await.unwrap();
        detector.register_request(meta_b).await.unwrap();

        let has_cycle = detector.detect_cycles().await.unwrap();
        assert!(has_cycle);

        let stats = detector.get_stats().await;
        assert_eq!(stats.cycles_detected, 1);
    }

    #[tokio::test]
    async fn test_timeout_cleanup() {
        let timeout = Duration::from_millis(100);
        let detector = DeadlockDetector::with_timeout(timeout);
        
        let metadata = create_test_metadata(AgentType::Admin, AgentType::PM, vec![]);
        detector.register_request(metadata).await.unwrap();

        // Wait for timeout
        sleep(Duration::from_millis(150));

        let timed_out = detector.cleanup_stale_requests().await.unwrap();
        assert_eq!(timed_out.len(), 1);

        let stats = detector.get_stats().await;
        assert_eq!(stats.active_requests, 0);
        assert_eq!(stats.timed_out_requests, 1);
    }

    #[tokio::test]
    async fn test_get_active_requests() {
        let detector = DeadlockDetector::new();
        
        let meta1 = create_test_metadata(AgentType::Admin, AgentType::PM, vec![]);
        let meta2 = create_test_metadata(AgentType::PM, AgentType::Worker, vec![]);
        
        detector.register_request(meta1).await.unwrap();
        detector.register_request(meta2).await.unwrap();

        let active = detector.get_active_requests().await;
        assert_eq!(active.len(), 2);
    }

    #[tokio::test]
    async fn test_statistics_tracking() {
        let detector = DeadlockDetector::new();
        
        // Register 3 requests
        for _ in 0..3 {
            let meta = create_test_metadata(AgentType::Admin, AgentType::PM, vec![]);
            detector.register_request(meta).await.unwrap();
        }

        let stats = detector.get_stats().await;
        assert_eq!(stats.active_requests, 3);

        // Complete 2 requests
        let active = detector.get_active_requests().await;
        detector.complete_request(active[0].message_id).await.unwrap();
        detector.complete_request(active[1].message_id).await.unwrap();

        let stats = detector.get_stats().await;
        assert_eq!(stats.active_requests, 1);
        assert_eq!(stats.completed_requests, 2);
    }

    #[tokio::test]
    async fn test_reset_stats() {
        let detector = DeadlockDetector::new();
        
        let meta = create_test_metadata(AgentType::Admin, AgentType::PM, vec![]);
        let msg_id = meta.message_id;
        
        detector.register_request(meta).await.unwrap();
        detector.complete_request(msg_id).await.unwrap();

        let stats = detector.get_stats().await;
        assert_eq!(stats.completed_requests, 1);

        detector.reset_stats().await;

        let stats = detector.get_stats().await;
        assert_eq!(stats.completed_requests, 0);
    }

    #[tokio::test]
    async fn test_clear_all() {
        let detector = DeadlockDetector::new();
        
        for _ in 0..5 {
            let meta = create_test_metadata(AgentType::Admin, AgentType::PM, vec![]);
            detector.register_request(meta).await.unwrap();
        }

        assert_eq!(detector.get_stats().await.active_requests, 5);

        detector.clear_all().await;

        assert_eq!(detector.get_stats().await.active_requests, 0);
    }

    #[tokio::test]
    async fn test_custom_timeout() {
        let custom_timeout = Duration::from_secs(60);
        let detector = DeadlockDetector::with_timeout(custom_timeout);
        
        assert_eq!(detector.timeout(), custom_timeout);
    }
}
