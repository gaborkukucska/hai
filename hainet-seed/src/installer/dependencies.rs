// START OF FILE hainet-seed/src/installer/dependencies.rs
//! System Dependencies Module
//! 
//! Checks and manages system dependencies for HAI-Net.

use anyhow::Result;
use tracing::{info, warn};
use std::process::Command;

use crate::installer::platform::Platform;

/// Dependency checker and installer
pub struct DependencyChecker {
    platform: Platform,
}

impl DependencyChecker {
    /// Create new dependency checker
    pub fn new(platform: Platform) -> Self {
        Self { platform }
    }
    
    /// Check all required dependencies
    pub async fn check_all(&self) -> Result<Vec<String>> {
        info!("🔍 Checking system dependencies...");
        
        let mut missing = Vec::new();
        
        // Check curl (needed for downloads)
        if !self.has_command("curl").await {
            missing.push("curl".to_string());
        }
        
        // Check git (needed for some installations)
        if !self.has_command("git").await {
            warn!("Git not found - some features may be limited");
        }
        
        Ok(missing)
    }
    
    /// Check if a command exists
    async fn has_command(&self, cmd: &str) -> bool {
        Command::new("which")
            .arg(cmd)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
    
    /// Install missing dependencies
    pub async fn install_missing(&self, dependencies: Vec<String>) -> Result<()> {
        if dependencies.is_empty() {
            info!("✅ All dependencies satisfied");
            return Ok(());
        }
        
        info!("📥 Installing missing dependencies: {:?}", dependencies);
        
        // Platform-specific package manager
        match &self.platform {
            Platform::Linux { .. } => {
                self.install_linux_deps(&dependencies).await?;
            }
            Platform::MacOS { .. } => {
                self.install_macos_deps(&dependencies).await?;
            }
            Platform::AndroidTermux { .. } => {
                self.install_termux_deps(&dependencies).await?;
            }
            Platform::Other(_) => {
                warn!("Automatic dependency installation not supported on this platform");
            }
        }
        
        Ok(())
    }
    
    async fn install_linux_deps(&self, _deps: &[String]) -> Result<()> {
        // Try apt-get (Debian/Ubuntu)
        if self.has_command("apt-get").await {
            info!("Using apt-get to install dependencies");
            // Note: Would need sudo in production
            // Command::new("sudo").args(&["apt-get", "install", "-y"]).args(deps).output()?;
        }
        // Try dnf (Fedora)
        else if self.has_command("dnf").await {
            info!("Using dnf to install dependencies");
        }
        // Try pacman (Arch)
        else if self.has_command("pacman").await {
            info!("Using pacman to install dependencies");
        }
        
        Ok(())
    }
    
    async fn install_macos_deps(&self, _deps: &[String]) -> Result<()> {
        if self.has_command("brew").await {
            info!("Using Homebrew to install dependencies");
            // Command::new("brew").arg("install").args(deps).output()?;
        }
        Ok(())
    }
    
    async fn install_termux_deps(&self, _deps: &[String]) -> Result<()> {
        if self.has_command("pkg").await {
            info!("Using pkg to install dependencies");
            // Command::new("pkg").args(&["install", "-y"]).args(deps).output()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_dependency_checker_creation() {
        let platform = Platform::detect().unwrap();
        let checker = DependencyChecker::new(platform);
        
        let result = checker.check_all().await;
        assert!(result.is_ok());
    }
}
