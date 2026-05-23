// START OF FILE hainet-persona/src/messaging/channels.rs

//! Message bus and channel infrastructure for agent communication
//!
//! This module provides the core messaging infrastructure for HAI-Net's hierarchical
//! agent system. It enforces constitutional compliance through:
//! - Strict hierarchy validation (User↔Admin↔PM↔Workers)
//! - Guardian interception hooks
//! - Priority routing integration
//! - Bounded queues to prevent resource exhaustion

use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};
use serde::{Serialize, Deserialize};

use super::types::{AgentId, AgentType, ChannelType, Message, Priority};

/// Default channel buffer size per agent
const DEFAULT_CHANNEL_BUFFER: usize = 100;

/// Channel endpoint for sending and receiving messages
#[derive(Clone)]
pub struct ChannelEndpoint {
    sender: mpsc::Sender<Message>,
    agent_id: AgentId,
}

impl ChannelEndpoint {
    /// Send a message through this channel
    pub async fn send(&self, message: Message) -> Result<()> {
        self.sender
            .send(message)
            .await
            .context("Failed to send message through channel")
    }

    /// Get the agent ID for this endpoint
    pub fn agent_id(&self) -> &AgentId {
        &self.agent_id
    }
}

/// Statistics for a single channel
#[derive(Debug, Clone, Default)]
pub struct ChannelStats {
    pub messages_sent: u64,
    pub messages_received: u64,
    pub messages_dropped: u64,
    pub current_queue_depth: usize,
    pub max_queue_depth: usize,
}

/// Status of an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStatus {
    pub state: String,
    pub activity: String,
    pub last_updated: u64,
}

/// Combined agent info for UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub id: AgentId,
    pub status: Option<AgentStatus>,
}

/// Message bus coordinating all agent communication channels
///
/// The MessageBus enforces HAI-Net's constitutional principles:
/// - Article V, Section 1: All messages can be intercepted by Guardian
/// - Article II, Section 2: Hierarchy preserves human agency
/// - Article I, Section 2: Communication is transparent and auditable
pub struct MessageBus {
    /// Channels indexed by agent ID
    channels: Arc<RwLock<HashMap<AgentId, mpsc::Sender<Message>>>>,
    
    /// Channel statistics
    stats: Arc<RwLock<HashMap<AgentId, ChannelStats>>>,
    
    /// Guardian monitoring channel (broadcast to Guardian for all messages)
    guardian_channel: Arc<RwLock<Option<mpsc::Sender<Message>>>>,
    
    /// Guardian interception hook (optional, set in Cycle 0.4)
    guardian_hook: Arc<RwLock<Option<GuardianHook>>>,
    
    /// Priority router hook (optional, set in Module 3)
    priority_hook: Arc<RwLock<Option<PriorityHook>>>,
    
    /// Agent statuses
    agent_statuses: Arc<RwLock<HashMap<AgentId, AgentStatus>>>,
    
    /// Channel buffer size
    buffer_size: usize,
}

/// Guardian interception hook (placeholder for Cycle 0.4)
pub type GuardianHook = Arc<dyn Fn(&Message) -> GuardianDecision + Send + Sync>;

/// Guardian decision on message routing
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardianDecision {
    Allow,
    Block { reason: String },
    Pause { reason: String },
}

/// Priority routing hook (placeholder for Module 3)
pub type PriorityHook = Arc<dyn Fn(&Message) -> Priority + Send + Sync>;

impl MessageBus {
    /// Create a new message bus
    pub async fn new() -> Result<Self> {
        Self::with_buffer_size(DEFAULT_CHANNEL_BUFFER).await
    }

    /// Create a new message bus with custom buffer size
    pub async fn with_buffer_size(buffer_size: usize) -> Result<Self> {
        info!("Initializing MessageBus with buffer size: {}", buffer_size);
        
        Ok(Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(HashMap::new())),
            guardian_channel: Arc::new(RwLock::new(None)),
            guardian_hook: Arc::new(RwLock::new(None)),
            priority_hook: Arc::new(RwLock::new(None)),
            agent_statuses: Arc::new(RwLock::new(HashMap::new())),
            buffer_size,
        })
    }

    /// Register a new agent and create its communication channel
    ///
    /// Returns a receiver for the agent to listen on and a sender endpoint
    /// for other agents to use when sending messages to this agent.
    pub async fn register_agent(
        &self,
        agent_id: AgentId,
    ) -> Result<(mpsc::Receiver<Message>, ChannelEndpoint)> {
        debug!("Registering agent: {:?}", agent_id);

        // Create bounded channel for this agent
        let (tx, rx) = mpsc::channel::<Message>(self.buffer_size);

        // Store the sender
        {
            let mut channels = self.channels.write().await;
            if channels.contains_key(&agent_id) {
                return Err(anyhow!("Agent already registered: {:?}", agent_id));
            }
            channels.insert(agent_id.clone(), tx.clone());
        }

        // Initialize statistics
        {
            let mut stats = self.stats.write().await;
            stats.insert(
                agent_id.clone(),
                ChannelStats {
                    max_queue_depth: self.buffer_size,
                    ..Default::default()
                },
            );
        }

        info!("Agent registered successfully: {:?}", agent_id);

        Ok((rx, ChannelEndpoint {
            sender: tx,
            agent_id,
        }))
    }



    /// Register Guardian agent for monitoring all messages
    ///
    /// The Guardian receives a copy of every message (read-only monitoring).
    /// This enables constitutional compliance checking without blocking normal operations.
    pub async fn register_guardian_monitor(
        &self,
        guardian_id: AgentId,
    ) -> Result<mpsc::Receiver<Message>> {
        info!("Registering Guardian monitoring channel: {:?}", guardian_id);
        
        // Create channel for Guardian (larger buffer for monitoring all messages)
        let (tx, rx) = mpsc::channel::<Message>(self.buffer_size * 10);
        
        // Store Guardian channel
        {
            let mut guardian_channel = self.guardian_channel.write().await;
            if guardian_channel.is_some() {
                return Err(anyhow!("Guardian already registered for monitoring"));
            }
            *guardian_channel = Some(tx);
        }
        
        info!("Guardian monitoring channel registered successfully");
        Ok(rx)
    }

    /// Unregister an agent and close its channel
    pub async fn unregister_agent(&self, agent_id: &AgentId) -> Result<()> {
        debug!("Unregistering agent: {:?}", agent_id);

        let mut removed = false;

        {
            let mut channels = self.channels.write().await;
            if channels.remove(agent_id).is_some() {
                removed = true;
            }
        }

        {
            let mut statuses = self.agent_statuses.write().await;
            statuses.remove(agent_id);
        }

        {
            let mut stats = self.stats.write().await;
            stats.remove(agent_id);
        }

        if removed {
            info!("Agent unregistered successfully: {:?}", agent_id);
            Ok(())
        } else {
            Err(anyhow!("Agent not found in registry: {:?}", agent_id))
        }
    }

    /// Send a message from one agent to another
    ///
    /// This is the core routing method that enforces:
    /// 1. Hierarchy validation
    /// 2. Guardian interception
    /// 3. Priority routing
    /// 4. Statistics tracking
    pub async fn send_message(&self, message: Message) -> Result<()> {
        // Validate route according to hierarchy
        self.validate_route(&message)?;

        // Send copy to Guardian for monitoring (non-blocking, fire and forget)
        self.notify_guardian(&message).await;

        // Guardian interception (if configured)
        if let Some(decision) = self.intercept_with_guardian(&message).await? {
            match decision {
                GuardianDecision::Allow => {
                    // Proceed
                }
                GuardianDecision::Block { reason } => {
                    warn!(
                        "Guardian blocked message from {:?} to {:?}: {}",
                        message.from, message.to, reason
                    );
                    return Err(anyhow!("Message blocked by Guardian: {}", reason));
                }
                GuardianDecision::Pause { reason } => {
                    warn!(
                        "Guardian paused message from {:?} to {:?}: {}",
                        message.from, message.to, reason
                    );
                    // In a full implementation, this would queue for human review
                    // For now, we treat it as a warning and allow
                }
            }
        }

        // Get priority (use message metadata priority or consult priority hook)
        let _priority = self.get_message_priority(&message).await;

        // Route the message
        let result = self.route_message(message.clone()).await;

        // Update statistics
        self.update_stats(&message, result.is_ok()).await;

        result
    }

    /// Notify Guardian of message (non-blocking monitoring copy)
    async fn notify_guardian(&self, message: &Message) {
        let guardian_channel = self.guardian_channel.read().await;
        
        if let Some(tx) = guardian_channel.as_ref() {
            // Send copy to Guardian (non-blocking, drop if full)
            if let Err(e) = tx.try_send(message.clone()) {
                debug!("Guardian monitoring channel full or closed: {}", e);
            }
        }
    }

    /// Validate that the message route follows the hierarchy rules
    ///
    /// Constitutional compliance: Article II, Section 2 - Human agency is preserved
    /// through strict hierarchy where Admin AI is the primary interface.
    fn validate_route(&self, message: &Message) -> Result<()> {
        let channel_type = ChannelType::from_agents(&message.from, &message.to)?;

        // Validate based on channel type
        match channel_type {
            ChannelType::UserToAdmin | ChannelType::AdminToUser => {
                // Always allowed - user is sovereign
                Ok(())
            }
            ChannelType::AdminToPM | ChannelType::PMToAdmin => {
                // Admin orchestrates PMs
                Ok(())
            }
            ChannelType::PMToWorker | ChannelType::WorkerToPM => {
                // PMs orchestrate workers
                Ok(())
            }
            ChannelType::PMToPM => {
                // PMs can communicate for coordination
                Ok(())
            }
            ChannelType::WorkerToWorker => {
                // Workers should go through their PM, but we allow direct
                // communication for efficiency (with Guardian monitoring)
                debug!(
                    "Direct worker-to-worker communication: {:?} -> {:?}",
                    message.from, message.to
                );
                Ok(())
            }
            ChannelType::GuardianMonitoring => {
                // Guardian monitoring channel - always allowed
                Ok(())
            }
            ChannelType::Invalid => {
                error!(
                    "Invalid communication route: {:?} -> {:?}",
                    message.from, message.to
                );
                Err(anyhow!(
                    "Invalid communication route: {:?} -> {:?}. Hierarchy must be respected.",
                    message.from.agent_type,
                    message.to.agent_type
                ))
            }
        }
    }

    /// Intercept message with Guardian (if configured)
    async fn intercept_with_guardian(&self, message: &Message) -> Result<Option<GuardianDecision>> {
        let hook = self.guardian_hook.read().await;
        
        if let Some(guardian) = hook.as_ref() {
            let decision = guardian(message);
            Ok(Some(decision))
        } else {
            Ok(None)
        }
    }

    /// Get message priority (use message metadata priority or consult priority hook)
    async fn get_message_priority(&self, message: &Message) -> Priority {
        let hook = self.priority_hook.read().await;
        
        if let Some(priority_fn) = hook.as_ref() {
            priority_fn(message)
        } else {
            message.metadata.priority
        }
    }

    /// Route the message to its destination
    async fn route_message(&self, message: Message) -> Result<()> {
        let channels = self.channels.read().await;
        
        let sender = channels
            .get(&message.to)
            .ok_or_else(|| anyhow!("Destination agent not registered: {:?}", message.to))?;

        sender
            .send(message.clone())
            .await
            .context("Failed to route message to destination")?;

        debug!(
            "Message routed: {:?} -> {:?} (priority: {:?})",
            message.from, message.to, message.metadata.priority
        );

        Ok(())
    }

    /// Update statistics for a message send
    async fn update_stats(&self, message: &Message, success: bool) {
        let mut stats = self.stats.write().await;

        // Update sender stats
        if let Some(sender_stats) = stats.get_mut(&message.from) {
            sender_stats.messages_sent += 1;
            if !success {
                sender_stats.messages_dropped += 1;
            }
        }

        // Update receiver stats
        if let Some(receiver_stats) = stats.get_mut(&message.to) {
            if success {
                receiver_stats.messages_received += 1;
                // Note: actual queue depth would require channel introspection
                // which Tokio mpsc doesn't expose. This is a placeholder.
            }
        }
    }

    /// Get statistics for a specific agent
    pub async fn get_agent_stats(&self, agent_id: &AgentId) -> Option<ChannelStats> {
        let stats = self.stats.read().await;
        stats.get(agent_id).cloned()
    }

    /// Get statistics for all agents
    pub async fn get_all_stats(&self) -> HashMap<AgentId, ChannelStats> {
        let stats = self.stats.read().await;
        stats.clone()
    }

    /// Set the guardian interception hook
    ///
    /// This will be fully implemented in Cycle 0.4
    pub async fn set_guardian_hook(&self, hook: GuardianHook) {
        let mut guardian_hook = self.guardian_hook.write().await;
        *guardian_hook = Some(hook);
        info!("Guardian hook registered");
    }

    /// Set the priority routing hook
    ///
    /// This will be fully implemented in Module 3
    pub async fn set_priority_hook(&self, hook: PriorityHook) {
        let mut priority_hook = self.priority_hook.write().await;
        *priority_hook = Some(hook);
        info!("Priority hook registered");
    }

    /// Get number of registered agents
    pub async fn agent_count(&self) -> usize {
        let channels = self.channels.read().await;
        channels.len()
    }

    /// Get list of all active agents with their status
    pub async fn get_active_agents(&self) -> Vec<AgentInfo> {
        let channels = self.channels.read().await;
        let statuses = self.agent_statuses.read().await;
        
        channels.keys().map(|id| {
            AgentInfo {
                id: id.clone(),
                status: statuses.get(id).cloned(),
            }
        }).collect()
    }
    
    
    /// Find an agent by name pattern (useful for finding PM agents by project ID)
    /// Returns the first matching agent ID, or None if not found
    pub async fn find_agent_by_name(&self, agent_type: AgentType, name_pattern: &str) -> Option<AgentId> {
        let channels = self.channels.read().await;
        
        channels.keys()
            .find(|id| id.agent_type == agent_type && id.name.contains(name_pattern))
            .cloned()
    }
    
    /// Update agent status
    pub async fn update_agent_status(&self, agent_id: AgentId, state: String, activity: String) {
        let mut statuses = self.agent_statuses.write().await;
        statuses.insert(agent_id, AgentStatus {
            state,
            activity,
            last_updated: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        });
    }
}

/// An `AgentEndpoint` provides a convenient way for an agent to send messages
/// back into the `MessageBus` without needing direct access to the bus itself.
/// It holds a reference to the `MessageBus` and the agent's own ID.
#[derive(Clone)]
pub struct AgentEndpoint {
    agent_id: AgentId,
    bus: Arc<MessageBus>,
}

impl AgentEndpoint {
    /// Returns the `AgentId` associated with this endpoint.
    pub fn agent_id(&self) -> &AgentId {
        &self.agent_id
    }

    /// Sends a message from this agent through the `MessageBus`.
    /// The `from` field of the message will be automatically set to this agent's ID.
    pub async fn send(&self, mut message: Message) -> Result<()> {
        message.from = self.agent_id.clone();
        self.bus.send_message(message).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messaging::types::{AgentType, MessageContent};

    async fn create_test_message(
        from_type: AgentType,
        to_type: AgentType,
        priority: Priority,
    ) -> Message {
        let from = AgentId::new(from_type, format!("{:?}-1", from_type));
        let to = AgentId::new(to_type, format!("{:?}-1", to_type));

        Message::new(
            from,
            to,
            MessageContent::UserInput("test".to_string()),
        )
        .with_priority(priority)
    }

    #[tokio::test]
    async fn test_message_bus_creation() {
        let bus = MessageBus::new().await.unwrap();
        assert_eq!(bus.agent_count().await, 0);
    }

    #[tokio::test]
    async fn test_agent_registration() {
        let bus = MessageBus::new().await.unwrap();
        
        let admin_id = AgentId::new(AgentType::Admin, "admin-1".to_string());
        let (mut rx, endpoint) = bus.register_agent(admin_id.clone()).await.unwrap();

        assert_eq!(bus.agent_count().await, 1);
        assert_eq!(endpoint.agent_id(), &admin_id);

        // Verify receiver works
        let msg = Message::new(
            AgentId::new(AgentType::Admin, "admin-2".to_string()),
            admin_id.clone(),
            MessageContent::UserInput("hello".to_string()),
        )
        .with_priority(Priority::Normal);

        endpoint.send(msg.clone()).await.unwrap();
        let received = rx.recv().await.unwrap();
        assert_eq!(received.content, msg.content);
    }

    #[tokio::test]
    async fn test_guardian_monitoring_channel() {
        let bus = MessageBus::new().await.unwrap();
        
        let guardian_id = AgentId::new_guardian("guardian-1".to_string());
        let mut guardian_rx = bus.register_guardian_monitor(guardian_id).await.unwrap();
        
        let admin_id = AgentId::new(AgentType::Admin, "admin-1".to_string());
        let pm_id = AgentId::new(AgentType::PM, "pm-1".to_string());
        
        let (_admin_rx, _admin_endpoint) = bus.register_agent(admin_id.clone()).await.unwrap();
        let (_pm_rx, _pm_endpoint) = bus.register_agent(pm_id.clone()).await.unwrap();
        
        let msg = Message::new(
            admin_id,
            pm_id,
            MessageContent::UserInput("test".to_string()),
        )
        .with_priority(Priority::Normal);
        
        bus.send_message(msg.clone()).await.unwrap();
        
        // Guardian should receive a copy
        let guardian_msg = guardian_rx.recv().await.unwrap();
        assert_eq!(guardian_msg.content, msg.content);
    }

    #[tokio::test]
    async fn test_duplicate_registration() {
        let bus = MessageBus::new().await.unwrap();
        
        let admin_id = AgentId::new(AgentType::Admin, "admin-1".to_string());
        bus.register_agent(admin_id.clone()).await.unwrap();
        
        let result = bus.register_agent(admin_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_valid_hierarchy_user_to_admin() {
        let bus = MessageBus::new().await.unwrap();
        
        let user_id = AgentId::user("user-1".to_string());
        let admin_id = AgentId::new(AgentType::Admin, "admin-1".to_string());
        
        bus.register_agent(user_id.clone()).await.unwrap();
        let (_rx, _endpoint) = bus.register_agent(admin_id.clone()).await.unwrap();

        let msg = create_test_message(AgentType::User, AgentType::Admin, Priority::Normal).await;
        
        let result = bus.validate_route(&msg);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_valid_hierarchy_admin_to_pm() {
        let bus = MessageBus::new().await.unwrap();
        
        let msg = create_test_message(AgentType::Admin, AgentType::PM, Priority::Normal).await;
        
        let result = bus.validate_route(&msg);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_valid_hierarchy_pm_to_worker() {
        let bus = MessageBus::new().await.unwrap();
        
        let msg = create_test_message(AgentType::PM, AgentType::Worker, Priority::Normal).await;
        
        let result = bus.validate_route(&msg);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_invalid_hierarchy_user_to_worker() {
        let bus = MessageBus::new().await.unwrap();
        
        let msg = create_test_message(AgentType::User, AgentType::Worker, Priority::Normal).await;
        
        let result = bus.validate_route(&msg);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_send_message_success() {
        let bus = MessageBus::new().await.unwrap();
        
        let admin_id = AgentId::new(AgentType::Admin, "admin-1".to_string());
        let pm_id = AgentId::new(AgentType::PM, "pm-comms".to_string());
        
        bus.register_agent(admin_id.clone()).await.unwrap();
        let (mut pm_rx, _pm_endpoint) = bus.register_agent(pm_id.clone()).await.unwrap();

        let msg = Message::new(
            admin_id.clone(),
            pm_id.clone(),
            MessageContent::UserInput("delegate task".to_string()),
        )
        .with_priority(Priority::Normal);

        bus.send_message(msg.clone()).await.unwrap();

        let received = pm_rx.recv().await.unwrap();
        assert_eq!(received.from, admin_id);
        assert_eq!(received.to, pm_id);
    }

    #[tokio::test]
    async fn test_statistics_tracking() {
        let bus = MessageBus::new().await.unwrap();
        
        let admin_id = AgentId::new(AgentType::Admin, "admin-1".to_string());
        let pm_id = AgentId::new(AgentType::PM, "pm-comms".to_string());
        
        bus.register_agent(admin_id.clone()).await.unwrap();
        let (_pm_rx, _pm_endpoint) = bus.register_agent(pm_id.clone()).await.unwrap();

        let msg = Message::new(
            admin_id.clone(),
            pm_id.clone(),
            MessageContent::UserInput("test".to_string()),
        )
        .with_priority(Priority::Normal);

        bus.send_message(msg).await.unwrap();

        let admin_stats = bus.get_agent_stats(&admin_id).await.unwrap();
        assert_eq!(admin_stats.messages_sent, 1);

        let pm_stats = bus.get_agent_stats(&pm_id).await.unwrap();
        assert_eq!(pm_stats.messages_received, 1);
    }

    #[tokio::test]
    async fn test_guardian_hook_allow() {
        let bus = MessageBus::new().await.unwrap();
        
        // Set guardian that always allows
        let hook: GuardianHook = Arc::new(|_msg| GuardianDecision::Allow);
        bus.set_guardian_hook(hook).await;

        let admin_id = AgentId::new(AgentType::Admin, "admin-1".to_string());
        let pm_id = AgentId::new(AgentType::PM, "pm-comms".to_string());
        
        bus.register_agent(admin_id.clone()).await.unwrap();
        let (mut pm_rx, _pm_endpoint) = bus.register_agent(pm_id.clone()).await.unwrap();

        let msg = Message::new(
            admin_id,
            pm_id,
            MessageContent::UserInput("test".to_string()),
        )
        .with_priority(Priority::Normal);

        bus.send_message(msg.clone()).await.unwrap();
        let received = pm_rx.recv().await.unwrap();
        assert_eq!(received.content, msg.content);
    }

    #[tokio::test]
    async fn test_guardian_hook_block() {
        let bus = MessageBus::new().await.unwrap();
        
        // Set guardian that always blocks
        let hook: GuardianHook = Arc::new(|_msg| GuardianDecision::Block {
            reason: "Test block".to_string(),
        });
        bus.set_guardian_hook(hook).await;

        let admin_id = AgentId::new(AgentType::Admin, "admin-1".to_string());
        let pm_id = AgentId::new(AgentType::PM, "pm-comms".to_string());
        
        bus.register_agent(admin_id.clone()).await.unwrap();
        bus.register_agent(pm_id.clone()).await.unwrap();

        let msg = Message::new(
            admin_id,
            pm_id,
            MessageContent::UserInput("test".to_string()),
        )
        .with_priority(Priority::Normal);

        let result = bus.send_message(msg).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("blocked by Guardian"));
    }
}
