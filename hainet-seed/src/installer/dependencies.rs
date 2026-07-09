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
        
        // Check tor
        if !self.has_command("tor").await {
            missing.push("tor".to_string());
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
    
    async fn install_linux_deps(&self, deps: &[String]) -> Result<()> {
        // Try apt-get (Debian/Ubuntu)
        if self.has_command("apt-get").await {
            info!("Using apt-get to install dependencies");
            let output = Command::new("sudo")
                .args(&["apt-get", "install", "-y"])
                .args(deps)
                .output()?;
            
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow::anyhow!("apt-get install failed: {}", stderr));
            }
        }
        // Try dnf (Fedora)
        else if self.has_command("dnf").await {
            info!("Using dnf to install dependencies");
            let output = Command::new("sudo")
                .args(&["dnf", "install", "-y"])
                .args(deps)
                .output()?;
            
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow::anyhow!("dnf install failed: {}", stderr));
            }
        }
        // Try pacman (Arch)
        else if self.has_command("pacman").await {
            info!("Using pacman to install dependencies");
            let output = Command::new("sudo")
                .args(&["pacman", "-S", "--noconfirm"])
                .args(deps)
                .output()?;
            
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow::anyhow!("pacman install failed: {}", stderr));
            }
        } else {
            return Err(anyhow::anyhow!("No supported package manager found (apt-get, dnf, or pacman)"));
        }
        
        Ok(())
    }
    
    async fn install_macos_deps(&self, deps: &[String]) -> Result<()> {
        if self.has_command("brew").await {
            info!("Using Homebrew to install dependencies");
            let output = Command::new("brew")
                .arg("install")
                .args(deps)
                .output()?;
            
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow::anyhow!("brew install failed: {}", stderr));
            }
        } else {
            return Err(anyhow::anyhow!("Homebrew not found - install from https://brew.sh"));
        }
        Ok(())
    }
    
    async fn install_termux_deps(&self, deps: &[String]) -> Result<()> {
        if self.has_command("pkg").await {
            info!("Using pkg to install dependencies");
            let output = Command::new("pkg")
                .args(&["install", "-y"])
                .args(deps)
                .output()?;
            
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow::anyhow!("pkg install failed: {}", stderr));
            }
        } else {
            return Err(anyhow::anyhow!("pkg not found - are you running in Termux?"));
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
