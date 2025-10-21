// START OF FILE hainet-seed/src/installer/mod.rs
//! HAI-Net Seed Installer Module
//! 
//! Handles platform detection, dependency installation, and Ollama setup.

pub mod platform;
pub mod ollama;
pub mod dependencies;

use anyhow::Result;
use tracing::info;

use crate::installer::platform::{Platform, SystemTier};
use crate::installer::ollama::OllamaInstaller;

/// Main installer orchestrator
pub struct Installer {
    platform: Platform,
    tier: SystemTier,
    ollama: OllamaInstaller,
}

impl Installer {
    /// Create new installer with platform detection
    pub async fn new() -> Result<Self> {
        info!("🔍 Detecting platform and system capabilities...");
        
        let platform = Platform::detect()?;
        let tier = SystemTier::detect()?;
        
        info!("Platform: {}", platform);
        info!("System Tier: {}", tier);
        
        let ollama = OllamaInstaller::new(platform.clone());
        
        Ok(Self {
            platform,
            tier,
            ollama,
        })
    }
    
    /// Run complete installation workflow
    pub async fn install(&mut self) -> Result<()> {
        info!("🚀 Starting HAI-Net installation workflow...");
        
        // Step 1: Check and install Ollama
        self.install_ollama().await?;
        
        // Step 2: Download default model based on tier
        self.download_default_model().await?;
        
        info!("✅ Installation complete!");
        Ok(())
    }
    
    /// Install Ollama if not present
    async fn install_ollama(&mut self) -> Result<()> {
        info!("🦙 Checking Ollama installation...");
        
        if self.ollama.is_installed().await? {
            info!("✅ Ollama already installed");
            
            // Ensure it's running
            if !self.ollama.is_running().await? {
                info!("Starting Ollama service...");
                self.ollama.start_service().await?;
            }
        } else {
            info!("📥 Ollama not found, installing...");
            self.ollama.install().await?;
            info!("✅ Ollama installed successfully");
            
            // Start the service
            self.ollama.start_service().await?;
        }
        
        Ok(())
    }
    
    /// Download default model based on system tier
    async fn download_default_model(&mut self) -> Result<()> {
        let model_name = match self.tier {
            SystemTier::Tier1 => {
                info!("📦 Tier 1 system detected - downloading gemma2:2b");
                "gemma2:2b"
            }
            SystemTier::Tier2 => {
                info!("📦 Tier 2 system detected - downloading gemma2:4b");
                "gemma2:4b"
            }
            SystemTier::Tier3 | SystemTier::Tier4 => {
                info!("📦 Tier 3/4 system detected - downloading gemma3:12b-it");
                "gemma3:12b-it"
            }
        };
        
        // Check if model already exists
        if self.ollama.has_model(model_name).await? {
            info!("✅ Model {} already available", model_name);
            return Ok(());
        }
        
        info!("📥 Downloading model: {}", model_name);
        info!("⚠️  This may take several minutes depending on your connection...");
        
        self.ollama.pull_model(model_name).await?;
        
        info!("✅ Model {} downloaded successfully", model_name);
        Ok(())
    }
    
    /// Get platform information
    pub fn platform(&self) -> &Platform {
        &self.platform
    }
    
    /// Get system tier
    pub fn tier(&self) -> &SystemTier {
        &self.tier
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_installer_creation() {
        let result = Installer::new().await;
        assert!(result.is_ok());
    }
}
