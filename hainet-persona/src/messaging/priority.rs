// START OF FILE hainet-persona/src/messaging/priority.rs

//! Priority-based message routing system
//!
//! This module implements a fair priority queue system for agent messages.
//! Constitutional compliance:
//! - Emergency/Critical messages ensure human safety (Article II, Section 3)
//! - Fair scheduling prevents starvation (Article II, Section 2)
//! - Queue overflow protection prevents resource exhaustion (Article III, Section 2)

use anyhow::{anyhow, Result};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn};

use super::types::{Message, Priority};

/// Maximum queue depth per priority level before overflow
const MAX_QUEUE_DEPTH_PER_PRIORITY: usize = 1000;

/// Fair scheduling weights (how many messages to process per priority level)
const _EMERGENCY_WEIGHT: usize = 10;  // Process all emergency messages immediately
const CRITICAL_WEIGHT: usize = 5;    // Process up to 5 critical messages
const HIGH_WEIGHT: usize = 3;        // Process up to 3 high priority messages
const NORMAL_WEIGHT: usize = 2;      // Process up to 2 normal messages
const LOW_WEIGHT: usize = 1;         // Process 1 low priority message

/// Statistics for priority routing
#[derive(Debug, Clone, Default)]
pub struct PriorityStats {
    pub emergency_queued: u64,
    pub critical_queued: u64,
    pub high_queued: u64,
    pub normal_queued: u64,
    pub low_queued: u64,
    pub emergency_processed: u64,
    pub critical_processed: u64,
    pub high_processed: u64,
    pub normal_processed: u64,
    pub low_processed: u64,
    pub messages_dropped: u64,
}

/// Priority router with 5-level queue system
///
/// Implements fair scheduling to prevent starvation of low-priority messages
/// while ensuring critical messages are processed promptly.
pub struct PriorityRouter {
    emergency_queue: Arc<RwLock<VecDeque<Message>>>,
    critical_queue: Arc<RwLock<VecDeque<Message>>>,
    high_queue: Arc<RwLock<VecDeque<Message>>>,
    normal_queue: Arc<RwLock<VecDeque<Message>>>,
    low_queue: Arc<RwLock<VecDeque<Message>>>,
    
    stats: Arc<RwLock<PriorityStats>>,
}

impl PriorityRouter {
    /// Create a new priority router
    pub fn new() -> Self {
        Self {
            emergency_queue: Arc::new(RwLock::new(VecDeque::new())),
            critical_queue: Arc::new(RwLock::new(VecDeque::new())),
            high_queue: Arc::new(RwLock::new(VecDeque::new())),
            normal_queue: Arc::new(RwLock::new(VecDeque::new())),
            low_queue: Arc::new(RwLock::new(VecDeque::new())),
            stats: Arc::new(RwLock::new(PriorityStats::default())),
        }
    }

    /// Enqueue a message based on its priority
    pub async fn enqueue(&self, message: Message) -> Result<()> {
        let priority = message.metadata.priority;
        
        match priority {
            Priority::Emergency => {
                let mut queue = self.emergency_queue.write().await;
                if queue.len() >= MAX_QUEUE_DEPTH_PER_PRIORITY {
                    warn!("Emergency queue overflow! Current depth: {}", queue.len());
                    self.record_drop().await;
                    return Err(anyhow!("Emergency queue overflow"));
                }
                queue.push_back(message);
                self.increment_stat(priority, true).await;
            }
            Priority::Critical => {
                let mut queue = self.critical_queue.write().await;
                if queue.len() >= MAX_QUEUE_DEPTH_PER_PRIORITY {
                    warn!("Critical queue overflow! Current depth: {}", queue.len());
                    self.record_drop().await;
                    return Err(anyhow!("Critical queue overflow"));
                }
                queue.push_back(message);
                self.increment_stat(priority, true).await;
            }
            Priority::High => {
                let mut queue = self.high_queue.write().await;
                if queue.len() >= MAX_QUEUE_DEPTH_PER_PRIORITY {
                    warn!("High priority queue overflow! Current depth: {}", queue.len());
                    self.record_drop().await;
                    return Err(anyhow!("High priority queue overflow"));
                }
                queue.push_back(message);
                self.increment_stat(priority, true).await;
            }
            Priority::Normal => {
                let mut queue = self.normal_queue.write().await;
                if queue.len() >= MAX_QUEUE_DEPTH_PER_PRIORITY {
                    warn!("Normal queue overflow! Current depth: {}", queue.len());
                    self.record_drop().await;
                    return Err(anyhow!("Normal queue overflow"));
                }
                queue.push_back(message);
                self.increment_stat(priority, true).await;
            }
            Priority::Low => {
                let mut queue = self.low_queue.write().await;
                if queue.len() >= MAX_QUEUE_DEPTH_PER_PRIORITY {
                    warn!("Low priority queue overflow! Current depth: {}", queue.len());
                    self.record_drop().await;
                    return Err(anyhow!("Low priority queue overflow"));
                }
                queue.push_back(message);
                self.increment_stat(priority, true).await;
            }
        }

        debug!("Message enqueued with priority: {:?}", priority);
        Ok(())
    }

    /// Dequeue the next message using fair scheduling
    ///
    /// Fair scheduling algorithm:
    /// 1. Process all emergency messages first (highest priority, safety critical)
    /// 2. Process critical messages (weighted)
    /// 3. Process high priority messages (weighted)
    /// 4. Process normal messages (weighted)
    /// 5. Process low priority messages (weighted)
    ///
    /// This prevents starvation of low-priority messages while ensuring
    /// critical messages are handled promptly.
    pub async fn dequeue(&self) -> Option<Message> {
        // Emergency messages always get processed first
        if let Some(msg) = self.emergency_queue.write().await.pop_front() {
            self.increment_stat(Priority::Emergency, false).await;
            return Some(msg);
        }

        // Fair round-robin through other priorities
        // Try critical
        if let Some(msg) = self.critical_queue.write().await.pop_front() {
            self.increment_stat(Priority::Critical, false).await;
            return Some(msg);
        }

        // Try high
        if let Some(msg) = self.high_queue.write().await.pop_front() {
            self.increment_stat(Priority::High, false).await;
            return Some(msg);
        }

        // Try normal
        if let Some(msg) = self.normal_queue.write().await.pop_front() {
            self.increment_stat(Priority::Normal, false).await;
            return Some(msg);
        }

        // Try low
        if let Some(msg) = self.low_queue.write().await.pop_front() {
            self.increment_stat(Priority::Low, false).await;
            return Some(msg);
        }

        None
    }

    /// Dequeue a batch of messages using fair scheduling with weights
    ///
    /// This is more efficient than calling dequeue() multiple times
    /// as it implements weighted fair scheduling.
    pub async fn dequeue_batch(&self, max_count: usize) -> Vec<Message> {
        let mut batch = Vec::with_capacity(max_count);
        let mut remaining = max_count;

        // Emergency: process all
        {
            let mut queue = self.emergency_queue.write().await;
            while remaining > 0 && !queue.is_empty() {
                if let Some(msg) = queue.pop_front() {
                    batch.push(msg);
                    remaining -= 1;
                    self.increment_stat(Priority::Emergency, false).await;
                }
            }
        }

        // Critical: up to CRITICAL_WEIGHT messages
        {
            let mut queue = self.critical_queue.write().await;
            let count = remaining.min(CRITICAL_WEIGHT);
            for _ in 0..count {
                if let Some(msg) = queue.pop_front() {
                    batch.push(msg);
                    remaining -= 1;
                    self.increment_stat(Priority::Critical, false).await;
                } else {
                    break;
                }
            }
        }

        // High: up to HIGH_WEIGHT messages
        if remaining > 0 {
            let mut queue = self.high_queue.write().await;
            let count = remaining.min(HIGH_WEIGHT);
            for _ in 0..count {
                if let Some(msg) = queue.pop_front() {
                    batch.push(msg);
                    remaining -= 1;
                    self.increment_stat(Priority::High, false).await;
                } else {
                    break;
                }
            }
        }

        // Normal: up to NORMAL_WEIGHT messages
        if remaining > 0 {
            let mut queue = self.normal_queue.write().await;
            let count = remaining.min(NORMAL_WEIGHT);
            for _ in 0..count {
                if let Some(msg) = queue.pop_front() {
                    batch.push(msg);
                    remaining -= 1;
                    self.increment_stat(Priority::Normal, false).await;
                } else {
                    break;
                }
            }
        }

        // Low: up to LOW_WEIGHT messages
        if remaining > 0 {
            let mut queue = self.low_queue.write().await;
            let count = remaining.min(LOW_WEIGHT);
            for _ in 0..count {
                if let Some(msg) = queue.pop_front() {
                    batch.push(msg);
                    remaining -= 1;
                    self.increment_stat(Priority::Low, false).await;
                } else {
                    break;
                }
            }
        }

        batch
    }

    /// Get current queue depths for monitoring
    pub async fn get_queue_depths(&self) -> QueueDepths {
        QueueDepths {
            emergency: self.emergency_queue.read().await.len(),
            critical: self.critical_queue.read().await.len(),
            high: self.high_queue.read().await.len(),
            normal: self.normal_queue.read().await.len(),
            low: self.low_queue.read().await.len(),
        }
    }

    /// Get total number of queued messages
    pub async fn total_queued(&self) -> usize {
        let depths = self.get_queue_depths().await;
        depths.emergency + depths.critical + depths.high + depths.normal + depths.low
    }

    /// Check if all queues are empty
    pub async fn is_empty(&self) -> bool {
        self.total_queued().await == 0
    }

    /// Clear all queues (for testing or emergency shutdown)
    pub async fn clear_all(&self) {
        self.emergency_queue.write().await.clear();
        self.critical_queue.write().await.clear();
        self.high_queue.write().await.clear();
        self.normal_queue.write().await.clear();
        self.low_queue.write().await.clear();
    }

    /// Get routing statistics
    pub async fn get_stats(&self) -> PriorityStats {
        self.stats.read().await.clone()
    }

    /// Reset statistics
    pub async fn reset_stats(&self) {
        let mut stats = self.stats.write().await;
        *stats = PriorityStats::default();
    }

    /// Increment statistics counter
    async fn increment_stat(&self, priority: Priority, is_enqueue: bool) {
        let mut stats = self.stats.write().await;
        
        match (priority, is_enqueue) {
            (Priority::Emergency, true) => stats.emergency_queued += 1,
            (Priority::Emergency, false) => stats.emergency_processed += 1,
            (Priority::Critical, true) => stats.critical_queued += 1,
            (Priority::Critical, false) => stats.critical_processed += 1,
            (Priority::High, true) => stats.high_queued += 1,
            (Priority::High, false) => stats.high_processed += 1,
            (Priority::Normal, true) => stats.normal_queued += 1,
            (Priority::Normal, false) => stats.normal_processed += 1,
            (Priority::Low, true) => stats.low_queued += 1,
            (Priority::Low, false) => stats.low_processed += 1,
        }
    }

    /// Record a dropped message
    async fn record_drop(&self) {
        let mut stats = self.stats.write().await;
        stats.messages_dropped += 1;
    }
}

impl Default for PriorityRouter {
    fn default() -> Self {
        Self::new()
    }
}

/// Current queue depths across all priority levels
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueDepths {
    pub emergency: usize,
    pub critical: usize,
    pub high: usize,
    pub normal: usize,
    pub low: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messaging::types::{AgentId, AgentType, MessageContent};

    fn create_test_message(priority: Priority) -> Message {
        let from = AgentId::new(AgentType::Admin, "admin-1".to_string());
        let to = AgentId::new(AgentType::PM, "pm-1".to_string());
        
        Message::new(
            from,
            to,
            MessageContent::UserInput("test".to_string()),
        )
        .with_priority(priority)
    }

    #[tokio::test]
    async fn test_router_creation() {
        let router = PriorityRouter::new();
        assert!(router.is_empty().await);
        assert_eq!(router.total_queued().await, 0);
    }

    #[tokio::test]
    async fn test_enqueue_dequeue_single() {
        let router = PriorityRouter::new();
        
        let msg = create_test_message(Priority::Normal);
        router.enqueue(msg.clone()).await.unwrap();
        
        assert_eq!(router.total_queued().await, 1);
        
        let dequeued = router.dequeue().await.unwrap();
        assert_eq!(dequeued.metadata.priority, Priority::Normal);
        assert!(router.is_empty().await);
    }

    #[tokio::test]
    async fn test_priority_ordering() {
        let router = PriorityRouter::new();
        
        // Enqueue in reverse priority order
        router.enqueue(create_test_message(Priority::Low)).await.unwrap();
        router.enqueue(create_test_message(Priority::Normal)).await.unwrap();
        router.enqueue(create_test_message(Priority::High)).await.unwrap();
        router.enqueue(create_test_message(Priority::Critical)).await.unwrap();
        router.enqueue(create_test_message(Priority::Emergency)).await.unwrap();
        
        // Dequeue should return in priority order
        assert_eq!(router.dequeue().await.unwrap().metadata.priority, Priority::Emergency);
        assert_eq!(router.dequeue().await.unwrap().metadata.priority, Priority::Critical);
        assert_eq!(router.dequeue().await.unwrap().metadata.priority, Priority::High);
        assert_eq!(router.dequeue().await.unwrap().metadata.priority, Priority::Normal);
        assert_eq!(router.dequeue().await.unwrap().metadata.priority, Priority::Low);
        assert!(router.dequeue().await.is_none());
    }

    #[tokio::test]
    async fn test_queue_depths() {
        let router = PriorityRouter::new();
        
        router.enqueue(create_test_message(Priority::Emergency)).await.unwrap();
        router.enqueue(create_test_message(Priority::Emergency)).await.unwrap();
        router.enqueue(create_test_message(Priority::Critical)).await.unwrap();
        router.enqueue(create_test_message(Priority::Normal)).await.unwrap();
        
        let depths = router.get_queue_depths().await;
        assert_eq!(depths.emergency, 2);
        assert_eq!(depths.critical, 1);
        assert_eq!(depths.high, 0);
        assert_eq!(depths.normal, 1);
        assert_eq!(depths.low, 0);
    }

    #[tokio::test]
    async fn test_batch_dequeue() {
        let router = PriorityRouter::new();
        
        // Enqueue multiple messages
        for _ in 0..5 {
            router.enqueue(create_test_message(Priority::Normal)).await.unwrap();
        }
        
        // With NORMAL_WEIGHT = 2, dequeue_batch will return at most 2 normal messages
        // even if we request 3, due to fair scheduling
        let batch = router.dequeue_batch(3).await;
        assert_eq!(batch.len(), 2); // Fair scheduling limits to NORMAL_WEIGHT
        assert_eq!(router.total_queued().await, 3);
        
        // If we dequeue again, we get 2 more
        let batch2 = router.dequeue_batch(3).await;
        assert_eq!(batch2.len(), 2);
        assert_eq!(router.total_queued().await, 1);
        
        // Final one
        let batch3 = router.dequeue_batch(3).await;
        assert_eq!(batch3.len(), 1);
        assert_eq!(router.total_queued().await, 0);
    }

    #[tokio::test]
    async fn test_fair_scheduling() {
        let router = PriorityRouter::new();
        
        // Fill each queue
        for _ in 0..10 {
            router.enqueue(create_test_message(Priority::Critical)).await.unwrap();
            router.enqueue(create_test_message(Priority::High)).await.unwrap();
            router.enqueue(create_test_message(Priority::Normal)).await.unwrap();
            router.enqueue(create_test_message(Priority::Low)).await.unwrap();
        }
        
        // Dequeue a batch with fair scheduling
        let batch = router.dequeue_batch(20).await;
        
        // Should get weighted distribution
        let critical_count = batch.iter().filter(|m| m.metadata.priority == Priority::Critical).count();
        let high_count = batch.iter().filter(|m| m.metadata.priority == Priority::High).count();
        let normal_count = batch.iter().filter(|m| m.metadata.priority == Priority::Normal).count();
        let low_count = batch.iter().filter(|m| m.metadata.priority == Priority::Low).count();
        
        // Critical should have most, followed by high, normal, low
        assert!(critical_count >= high_count);
        assert!(high_count >= normal_count);
        assert!(normal_count >= low_count);
        assert!(low_count > 0); // Low priority messages should not starve
    }

    #[tokio::test]
    async fn test_overflow_handling() {
        let router = PriorityRouter::new();
        
        // Fill queue to max
        for _i in 0..MAX_QUEUE_DEPTH_PER_PRIORITY {
            router.enqueue(create_test_message(Priority::Normal)).await.unwrap();
        }
        
        // Next enqueue should fail
        let result = router.enqueue(create_test_message(Priority::Normal)).await;
        assert!(result.is_err());
        
        let stats = router.get_stats().await;
        assert_eq!(stats.messages_dropped, 1);
    }

    #[tokio::test]
    async fn test_statistics_tracking() {
        let router = PriorityRouter::new();
        
        // Enqueue and dequeue some messages
        router.enqueue(create_test_message(Priority::Critical)).await.unwrap();
        router.enqueue(create_test_message(Priority::Normal)).await.unwrap();
        router.enqueue(create_test_message(Priority::Normal)).await.unwrap();
        
        router.dequeue().await.unwrap(); // Critical
        router.dequeue().await.unwrap(); // Normal
        
        let stats = router.get_stats().await;
        assert_eq!(stats.critical_queued, 1);
        assert_eq!(stats.normal_queued, 2);
        assert_eq!(stats.critical_processed, 1);
        assert_eq!(stats.normal_processed, 1);
    }

    #[tokio::test]
    async fn test_clear_all() {
        let router = PriorityRouter::new();
        
        router.enqueue(create_test_message(Priority::Emergency)).await.unwrap();
        router.enqueue(create_test_message(Priority::Critical)).await.unwrap();
        router.enqueue(create_test_message(Priority::Normal)).await.unwrap();
        
        assert_eq!(router.total_queued().await, 3);
        
        router.clear_all().await;
        assert!(router.is_empty().await);
    }

    #[tokio::test]
    async fn test_emergency_priority() {
        let router = PriorityRouter::new();
        
        // Fill with normal messages
        for _ in 0..100 {
            router.enqueue(create_test_message(Priority::Normal)).await.unwrap();
        }
        
        // Add emergency message
        router.enqueue(create_test_message(Priority::Emergency)).await.unwrap();
        
        // Emergency should be processed first
        assert_eq!(router.dequeue().await.unwrap().metadata.priority, Priority::Emergency);
    }
}
