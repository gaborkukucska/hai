// START OF FILE hainet-persona/src/messaging/mod.rs

//! Hierarchical agent communication system with constitutional monitoring
//! 
//! This module provides the messaging infrastructure for HAI-Net's multi-agent AI system.
//! It enforces strict communication hierarchy (User↔Admin↔PM↔Workers) and integrates
//! Constitutional Guardian monitoring for all messages.
//! 
//! # Architecture
//! 
//! - **Message Types**: Comprehensive message type system with priority levels
//! - **Message Bus**: Tokio mpsc channel-based routing with hierarchy enforcement
//! - **Priority Router**: 5-level priority queue with fair scheduling
//! - **Guardian Interceptor**: Real-time constitutional compliance monitoring
//! - **Audit Trail**: Immutable SQLite-based logging with tamper detection
//! - **Deadlock Detection**: Dependency graph analysis with timeout enforcement
//! 
//! # Constitutional Compliance
//! 
//! All messages are intercepted by the Constitutional Guardian system before routing,
//! ensuring compliance with HAI-Net's core principles (privacy, harm prevention, etc.).

pub mod types;
pub mod channels;
pub mod priority;
pub mod guardian;
pub mod audit;
pub mod deadlock;

// Re-export core types for convenience
pub use types::{
    AgentId, AgentType, Message, MessageContent, MessageId, MessageMetadata,
    Priority, ChannelType, MessageContext, Task, TaskId, TaskResult,
    CoordinationMsg, StatusUpdate, Alert, ErrorReport,
};

pub use channels::MessageBus;
pub use priority::PriorityRouter;
pub use guardian::{GuardianInterceptor, InterceptResult, BlockReason, PauseReason};
pub use audit::{AuditLogger, AuditEntry};
pub use deadlock::{DeadlockDetector, RequestMetadata};

use anyhow::Result;

/// Initialize the messaging system
pub async fn init() -> Result<MessageBus> {
    tracing::info!("Initializing HAI-Net messaging system");
    
    let message_bus = MessageBus::new().await?;
    
    tracing::info!("Messaging system initialized successfully");
    Ok(message_bus)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_messaging_init() {
        let result = init().await;
        assert!(result.is_ok());
    }
}
