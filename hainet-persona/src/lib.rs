//! HAI-Net Persona Library
//! 
//! Multi-agent AI intelligence system providing the core AI functionality
//! for HAI-Net nodes.

// Core modules
pub mod config;
pub mod prompts;
pub mod messaging;
pub mod ai_providers;
pub mod guardian;
pub mod tools;
pub mod agents;
pub mod projects;
pub mod test_utils;
pub mod user_settings;

// TODO: Implement these modules in later cycles
// pub mod memory;
// pub mod blockchain;

use anyhow::Result;
use tracing::info;

// Re-export core types for convenience
pub use config::{
    HaiNetConfig, ModelDefaults, GenerationDefaults, ReliabilityDefaults, PathDefaults,
};

pub use prompts::{
    PromptManager, PromptLoader, PromptRenderer, PromptCache,
    AgentType, AgentState, PromptContext,
};

pub use messaging::{
    AgentId, Message, MessageContent, MessageBus, Priority,
};

pub use projects::{
    ProjectManager, Project, ProjectId, ProjectStatus,
    Task, TaskId, TaskStatus,
    Milestone, MilestoneId, MilestoneStatus,
};

pub use user_settings::{
    UserSettingsManager, SharedUserSettings, ModelPreference,
};

/// Initialize the persona system
pub async fn init() -> Result<()> {
    info!("🤖 Initializing HAI-Net Persona system...");
    
    // TODO: Initialize core components
    // - Prompt management system
    // - Agent type system  
    // - MCP client
    // - State machines
    // - Memory systems
    // - Blockchain links
    
    info!("✅ HAI-Net Persona system initialized");
    Ok(())
}

/// Main persona service entry point
pub struct PersonaService {
    // TODO: Add core service components
}

impl PersonaService {
    pub async fn new() -> Result<Self> {
        init().await?;
        
        Ok(Self {
            // TODO: Initialize service components
        })
    }
    
    pub async fn start(&mut self) -> Result<()> {
        info!("🚀 Starting HAI-Net Persona service...");
        
        // TODO: Start service components
        // - Admin AI
        // - PM agents
        // - Worker agents
        // - MCP servers
        
        Ok(())
    }
    
    pub async fn shutdown(&mut self) -> Result<()> {
        info!("🛑 Shutting down HAI-Net Persona service...");
        
        // TODO: Graceful shutdown
        
        Ok(())
    }
}
