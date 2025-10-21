// START OF FILE hainet-seed/src/installer/ollama.rs
//! Ollama Installation and Management Module
//! 
//! Handles automatic detection, installation, and management of Ollama.

use anyhow::{Result, anyhow};
use tracing::{info, debug, warn};
use std::process::Command;
use reqwest;
use indicatif::{ProgressBar, ProgressStyle};

use crate::installer::platform::Platform;

/// Ollama installer and manager
pub struct OllamaInstaller {
    platform: Platform,
    endpoint: String,
}

impl OllamaInstaller {
    /// Create new Ollama installer
    pub fn new(platform: Platform) -> Self {
        Self {
            platform,
            endpoint: "http://localhost:11434".to_string(),
        }
    }
    
    /// Check if Ollama is installed
    pub async fn is_installed(&self) -> Result<bool> {
        debug!("Checking if Ollama is installed...");
        
        // Try to find ollama binary
        let result = Command::new("which")
            .arg("ollama")
            .output();
        
        match result {
            Ok(output) => Ok(output.status.success()),
            Err(_) => Ok(false),
        }
    }
    
    /// Check if Ollama service is running
    pub async fn is_running(&self) -> Result<bool> {
        debug!("Checking if Ollama is running...");
        
        // Try to connect to Ollama API
        let client = reqwest::Client::new();
        let url = format!("{}/api/version", self.endpoint);
        
        match client.get(&url).timeout(std::time::Duration::from_secs(2)).send().await {
            Ok(response) => Ok(response.status().is_success()),
            Err(_) => Ok(false),
        }
    }
    
    /// Install Ollama
    pub async fn install(&self) -> Result<()> {
        info!("Installing Ollama for platform: {}", self.platform);
        
        match &self.platform {
            Platform::Linux { .. } => self.install_linux().await,
            Platform::MacOS { .. } => self.install_macos().await,
            Platform::AndroidTermux { .. } => self.install_termux().await,
            Platform::Other(name) => {
                Err(anyhow!("Ollama installation not supported on platform: {}", name))
            }
        }
    }
    
    /// Install Ollama on Linux
    async fn install_linux(&self) -> Result<()> {
        info!("📥 Downloading Ollama install script...");
        
        let client = reqwest::Client::new();
        let script = client.get("https://ollama.com/install.sh")
            .send()
            .await?
            .text()
            .await?;
        
        // Save script to temp file
        let script_path = "/tmp/ollama_install.sh";
        std::fs::write(script_path, script)?;
        
        info!("🔧 Running Ollama installer (may require sudo)...");
        
        let output = Command::new("sh")
            .arg(script_path)
            .output()?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Ollama installation failed: {}", stderr));
        }
        
        info!("✅ Ollama installed successfully");
        Ok(())
    }
    
    /// Install Ollama on macOS
    async fn install_macos(&self) -> Result<()> {
        info!("📥 Installing Ollama on macOS...");
        
        // Check if Homebrew is available
        let has_brew = Command::new("which")
            .arg("brew")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        
        if has_brew {
            info!("Using Homebrew to install Ollama...");
            
            let output = Command::new("brew")
                .args(&["install", "ollama"])
                .output()?;
            
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow!("Homebrew installation failed: {}", stderr));
            }
        } else {
            warn!("Homebrew not found. Please install Ollama manually from https://ollama.com/download");
            return Err(anyhow!("Homebrew required for macOS installation"));
        }
        
        info!("✅ Ollama installed successfully");
        Ok(())
    }
    
    /// Install Ollama on Android/Termux
    async fn install_termux(&self) -> Result<()> {
        info!("📥 Installing Ollama on Termux...");
        
        // Termux doesn't have official Ollama support yet
        // We'll need to use a community build or alternative
        warn!("Ollama native support on Termux is limited");
        warn!("Consider using a lightweight model server alternative");
        
        Err(anyhow!("Ollama installation on Termux requires manual setup"))
    }
    
    /// Start Ollama service
    pub async fn start_service(&self) -> Result<()> {
        info!("🚀 Starting Ollama service...");
        
        // Check if already running
        if self.is_running().await? {
            info!("✅ Ollama is already running");
            return Ok(());
        }
        
        // Start Ollama in background
        match &self.platform {
            Platform::Linux { .. } | Platform::MacOS { .. } => {
                // Start Ollama serve in background
                Command::new("ollama")
                    .arg("serve")
                    .spawn()?;
                
                // Wait a bit for service to start
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                
                // Verify it started
                if !self.is_running().await? {
                    return Err(anyhow!("Ollama service failed to start"));
                }
                
                info!("✅ Ollama service started");
                Ok(())
            }
            _ => {
                Err(anyhow!("Service start not supported on this platform"))
            }
        }
    }
    
    /// Check if model is available
    pub async fn has_model(&self, model_name: &str) -> Result<bool> {
        debug!("Checking if model {} is available", model_name);
        
        let client = reqwest::Client::new();
        let url = format!("{}/api/tags", self.endpoint);
        
        let response = client.get(&url).send().await?;
        
        if !response.status().is_success() {
            return Ok(false);
        }
        
        let json: serde_json::Value = response.json().await?;
        
        if let Some(models) = json.get("models").and_then(|m| m.as_array()) {
            for model in models {
                if let Some(name) = model.get("name").and_then(|n| n.as_str()) {
                    if name.starts_with(model_name) {
                        return Ok(true);
                    }
                }
            }
        }
        
        Ok(false)
    }
    
    /// Pull/download a model
    pub async fn pull_model(&self, model_name: &str) -> Result<()> {
        info!("📥 Pulling model: {}", model_name);
        
        // Create progress bar
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} {msg}")
                .unwrap()
        );
        pb.set_message(format!("Downloading {}...", model_name));
        
        // Use ollama CLI to pull model
        let output = Command::new("ollama")
            .args(&["pull", model_name])
            .output()?;
        
        pb.finish_with_message(format!("✅ Model {} downloaded", model_name));
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Model pull failed: {}", stderr));
        }
        
        Ok(())
    }
    
    /// List available models
    pub async fn list_models(&self) -> Result<Vec<String>> {
        debug!("Listing available models");
        
        let client = reqwest::Client::new();
        let url = format!("{}/api/tags", self.endpoint);
        
        let response = client.get(&url).send().await?;
        let json: serde_json::Value = response.json().await?;
        
        let mut models = Vec::new();
        
        if let Some(model_list) = json.get("models").and_then(|m| m.as_array()) {
            for model in model_list {
                if let Some(name) = model.get("name").and_then(|n| n.as_str()) {
                    models.push(name.to_string());
                }
            }
        }
        
        Ok(models)
    }
    
    /// Get Ollama version
    pub async fn version(&self) -> Result<String> {
        let client = reqwest::Client::new();
        let url = format!("{}/api/version", self.endpoint);
        
        let response = client.get(&url).send().await?;
        let json: serde_json::Value = response.json().await?;
        
        let version = json.get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        
        Ok(version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_ollama_installer_creation() {
        let platform = Platform::detect().unwrap();
        let installer = OllamaInstaller::new(platform);
        
        // Just check creation works
        assert_eq!(installer.endpoint, "http://localhost:11434");
    }
    
    #[tokio::test]
    async fn test_is_installed_check() {
        let platform = Platform::detect().unwrap();
        let installer = OllamaInstaller::new(platform);
        
        // Should not error (may be true or false)
        let result = installer.is_installed().await;
        assert!(result.is_ok());
    }
}
