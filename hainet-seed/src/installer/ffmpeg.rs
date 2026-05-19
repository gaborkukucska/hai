// START OF FILE hainet-seed/src/installer/ffmpeg.rs
//! FFmpeg Installation and Management Module
//! 
//! Handles automatic detection and installation of FFmpeg for media processing.

use anyhow::{Result, anyhow};
use tracing::{info, debug, warn};
use std::process::Command;

use crate::installer::platform::Platform;

/// FFmpeg installer and manager
pub struct FFmpegInstaller {
    platform: Platform,
}

impl FFmpegInstaller {
    /// Create new FFmpeg installer
    pub fn new(platform: Platform) -> Self {
        Self { platform }
    }
    
    /// Check if FFmpeg is installed
    pub async fn is_installed(&self) -> Result<bool> {
        debug!("Checking if FFmpeg is installed...");
        
        let result = Command::new("which")
            .arg("ffmpeg")
            .output();
            
        match result {
            Ok(output) => Ok(output.status.success()),
            Err(_) => Ok(false),
        }
    }
    
    /// Install FFmpeg
    pub async fn install(&self) -> Result<()> {
        info!("Installing FFmpeg for platform: {}", self.platform);
        
        match &self.platform {
            Platform::Linux { .. } => self.install_linux().await,
            _ => {
                warn!("Automated FFmpeg installation is only supported on Linux.");
                Err(anyhow!("Unsupported platform for FFmpeg automatic install."))
            }
        }
    }
    
    /// Install FFmpeg on Linux
    async fn install_linux(&self) -> Result<()> {
        info!("📥 Installing FFmpeg...");
        
        // Try apt-get
        if Command::new("which").arg("apt-get").output().map_or(false, |o| o.status.success()) {
            let output = Command::new("sudo")
                .args(&["apt-get", "install", "-y", "ffmpeg"])
                .output()?;
                
            if output.status.success() {
                info!("✅ FFmpeg installed successfully via apt-get");
                return Ok(());
            }
        }
        
        // Try dnf
        if Command::new("which").arg("dnf").output().map_or(false, |o| o.status.success()) {
            let output = Command::new("sudo")
                .args(&["dnf", "install", "-y", "ffmpeg"])
                .output()?;
                
            if output.status.success() {
                info!("✅ FFmpeg installed successfully via dnf");
                return Ok(());
            }
        }
        
        // Try pacman
        if Command::new("which").arg("pacman").output().map_or(false, |o| o.status.success()) {
            let output = Command::new("sudo")
                .args(&["pacman", "-S", "--noconfirm", "ffmpeg"])
                .output()?;
                
            if output.status.success() {
                info!("✅ FFmpeg installed successfully via pacman");
                return Ok(());
            }
        }
        
        Err(anyhow!("Failed to install FFmpeg: package manager not supported or installation failed."))
    }
}
