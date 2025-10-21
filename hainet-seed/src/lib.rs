//! HAI-Net Seed Library
//! 
//! AI-guided installer and bootstrap system for setting up new HAI-Net nodes.

// Installer module (Cycle 0.5 - Phase B)
pub mod installer;

// TODO: Implement these modules in later cycles
// pub mod setup;
// pub mod onboarding;
// pub mod bootstrap;
// pub mod identity;

use anyhow::Result;
use tracing::info;

// Re-export key installer types
pub use installer::Installer;
pub use installer::platform::{Platform, SystemTier, Architecture};

/// Initialize the seed system
pub async fn init() -> Result<()> {
    info!("🌱 Initializing HAI-Net Seed system...");
    
    // TODO: Initialize core components
    // - System requirements checker
    // - Identity generation
    // - Membership application system
    // - Model downloader
    // - Hub configuration
    
    info!("✅ HAI-Net Seed system initialized");
    Ok(())
}

/// Main seed service entry point
pub struct SeedService {
    installer: Installer,
}

impl SeedService {
    pub async fn new() -> Result<Self> {
        init().await?;
        
        let installer = Installer::new().await?;
        
        Ok(Self {
            installer,
        })
    }
    
    pub async fn install(&mut self) -> Result<()> {
        info!("🚀 Starting HAI-Net installation...");
        
        // Run installation workflow
        self.installer.install().await?;
        
        // TODO: Additional setup steps
        // - Generate identity
        // - Apply for membership
        // - Configure hub
        // - Bootstrap network
        
        Ok(())
    }
    
    pub async fn check_requirements(&self) -> Result<()> {
        info!("🔍 Checking system requirements...");
        
        info!("Platform: {}", self.installer.platform());
        info!("System Tier: {}", self.installer.tier());
        
        Ok(())
    }
}
