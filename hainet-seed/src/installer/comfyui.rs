// START OF FILE hainet-seed/src/installer/comfyui.rs
//! ComfyUI Installation and Management Module
//! 
//! Handles automatic detection, installation, and management of ComfyUI.

use anyhow::{Result, anyhow};
use tracing::{info, debug, warn};
use std::process::Command;
use std::path::Path;

use crate::installer::platform::Platform;

/// ComfyUI installer and manager
pub struct ComfyUIInstaller {
    platform: Platform,
    endpoint: String,
    install_dir: String,
}

impl ComfyUIInstaller {
    /// Create new ComfyUI installer
    pub fn new(platform: Platform) -> Self {
        Self {
            platform,
            endpoint: "http://localhost:8188".to_string(),
            install_dir: "/var/lib/hainet/ComfyUI".to_string(), // Default install dir
        }
    }
    
    /// Check if ComfyUI is installed
    pub async fn is_installed(&self) -> Result<bool> {
        debug!("Checking if ComfyUI is installed...");
        
        let path = Path::new(&self.install_dir);
        if path.exists() && path.join("main.py").exists() {
            return Ok(true);
        }
        
        // Check if it's running somewhere else
        let output = Command::new("pgrep")
            .arg("-f")
            .arg("main.py.*comfyui")
            .output();
            
        match output {
            Ok(out) => Ok(out.status.success()),
            Err(_) => Ok(false),
        }
    }
    
    /// Check if ComfyUI service is running
    pub async fn is_running(&self) -> Result<bool> {
        debug!("Checking if ComfyUI is running...");
        
        let client = reqwest::Client::new();
        let url = format!("{}/system_stats", self.endpoint);
        
        match client.get(&url).timeout(std::time::Duration::from_secs(2)).send().await {
            Ok(response) => Ok(response.status().is_success()),
            Err(_) => Ok(false),
        }
    }
    
    /// Install ComfyUI
    pub async fn install(&self) -> Result<()> {
        info!("Installing ComfyUI for platform: {}", self.platform);
        
        match &self.platform {
            Platform::Linux { .. } => self.install_linux().await,
            _ => {
                warn!("Automated ComfyUI installation is only supported on Linux.");
                Err(anyhow!("Unsupported platform for ComfyUI automatic install."))
            }
        }
    }
    
    /// Install ComfyUI on Linux
    async fn install_linux(&self) -> Result<()> {
        info!("📥 Installing dependencies for ComfyUI...");
        
        // We assume git and python3-venv are available, but we can try to install them if needed.
        let output = Command::new("sudo")
            .args(&["apt-get", "install", "-y", "git", "python3-venv", "python3-pip"])
            .output()?;
            
        if !output.status.success() {
            warn!("Failed to install apt dependencies, continuing anyway...");
        }
        
        info!("📥 Cloning ComfyUI repository...");
        let parent_dir = Path::new(&self.install_dir).parent().unwrap();
        if !parent_dir.exists() {
            Command::new("sudo").args(&["mkdir", "-p", parent_dir.to_str().unwrap()]).output()?;
            Command::new("sudo").args(&["chown", "-R", &std::env::var("USER").unwrap_or_else(|_| "root".to_string()), parent_dir.to_str().unwrap()]).output()?;
        }
        
        if !Path::new(&self.install_dir).exists() {
            let clone_status = Command::new("git")
                .args(&["clone", "https://github.com/comfyanonymous/ComfyUI.git", &self.install_dir])
                .status()?;
                
            if !clone_status.success() {
                return Err(anyhow!("Failed to clone ComfyUI repository"));
            }
        }
        
        info!("🔧 Setting up Python virtual environment...");
        let venv_path = format!("{}/venv", self.install_dir);
        Command::new("python3")
            .args(&["-m", "venv", &venv_path])
            .status()?;
            
        let pip_path = format!("{}/bin/pip", venv_path);
        
        info!("📦 Installing PyTorch...");
        Command::new(&pip_path)
            .args(&["install", "torch", "torchvision", "torchaudio", "--extra-index-url", "https://download.pytorch.org/whl/cu121"])
            .status()?;
            
        info!("📦 Installing ComfyUI requirements...");
        let req_path = format!("{}/requirements.txt", self.install_dir);
        Command::new(&pip_path)
            .args(&["install", "-r", &req_path])
            .status()?;
            
        info!("✅ ComfyUI installed successfully");
        Ok(())
    }
}
