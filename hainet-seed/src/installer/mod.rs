// START OF FILE hainet-seed/src/installer/mod.rs
//! HAI-Net Seed Installer Module
//! 
//! Handles platform detection, dependency installation, and Ollama setup.

pub mod platform;
pub mod ollama;
pub mod whisper;
pub mod piper;
pub mod dependencies;
pub mod network_scanner;
pub mod nmap_installer;
pub mod ssh_client;
pub mod ssh_keys;
pub mod deployment;

use anyhow::Result;
use tracing::info;

use crate::installer::platform::{Platform, SystemTier};
use crate::installer::ollama::OllamaInstaller;
use crate::installer::whisper::WhisperInstaller;
use crate::installer::piper::PiperInstaller;
use crate::installer::network_scanner::{NetworkScanner, DeviceCandidate};
use crate::installer::nmap_installer::ensure_nmap_installed;
use crate::installer::ssh_client::{SSHClient, SSHCredentials, DeviceCapabilities, SSHClientTrait};
use crate::installer::ssh_keys::SSHKeyManager;
use crate::installer::deployment::DeploymentOrchestrator;

/// Main installer orchestrator
pub struct Installer {
    platform: Platform,
    tier: SystemTier,
    ollama: OllamaInstaller,
    whisper: WhisperInstaller,
    piper: PiperInstaller,
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
        let whisper = WhisperInstaller::new(platform.clone());
        let piper = PiperInstaller::new(platform.clone());
        
        Ok(Self {
            platform,
            tier,
            ollama,
            whisper,
            piper,
        })
    }
    
    /// Run complete installation workflow
    pub async fn install(&mut self) -> Result<()> {
        info!("🚀 Starting HAI-Net installation workflow...");
        
        // Step 1: Check and install Ollama
        self.install_ollama().await?;
        
        // Step 2: Download default Ollama model based on tier
        self.download_default_model().await?;
        
        // Step 3: Check and install whisper.cpp
        self.install_whisper().await?;
        
        // Step 4: Download default Whisper model based on tier
        self.download_whisper_model().await?;
        
        // Step 5: Check and install Piper TTS
        self.install_piper().await?;
        
        // Step 6: Download default Piper voice model based on tier
        self.download_piper_model().await?;
        
        // Step 7: Optionally set up multi-device mesh
        if self.prompt_mesh_setup()? {
            self.discover_mesh_devices().await?;
        }
        
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
    
    /// Install whisper.cpp if not present
    async fn install_whisper(&mut self) -> Result<()> {
        info!("🎤 Checking whisper.cpp installation...");
        
        if self.whisper.is_installed().await? {
            info!("✅ whisper.cpp already installed");
            
            // Verify it works
            if let Err(e) = self.whisper.verify_installation().await {
                info!("⚠️  whisper.cpp verification failed: {}", e);
                info!("Reinstalling whisper.cpp...");
                self.whisper.install().await?;
            }
        } else {
            info!("📥 whisper.cpp not found, installing...");
            self.whisper.install().await?;
            info!("✅ whisper.cpp installed successfully");
        }
        
        Ok(())
    }
    
    /// Download Whisper model based on system tier
    async fn download_whisper_model(&mut self) -> Result<()> {
        use crate::installer::platform::SystemTier;
        
        let ram_gb = SystemTier::get_total_ram_gb()?;
        let model_name = self.whisper.recommended_model(ram_gb as usize);
        
        info!("📦 System RAM: {}GB - selecting Whisper model: {}", ram_gb, model_name);
        
        self.whisper.download_model(model_name).await?;
        
        Ok(())
    }
    
    /// Install Piper TTS if not present
    async fn install_piper(&mut self) -> Result<()> {
        info!("📢 Checking Piper TTS installation...");
        
        if self.piper.is_installed() {
            info!("✅ Piper TTS already installed");
            
            // Verify it's working
            if !self.piper.is_running() {
                info!("⚠️  Piper verification failed");
                info!("Reinstalling Piper TTS...");
                self.piper.install()?;
            }
        } else {
            info!("📥 Piper TTS not found, installing...");
            self.piper.install()?;
            info!("✅ Piper TTS installed successfully");
        }
        
        Ok(())
    }
    
    /// Download Piper voice model based on system tier
    async fn download_piper_model(&mut self) -> Result<()> {
        let voice_model = self.piper.recommended_model();
        
        info!("📦 System Tier: {:?} - selecting Piper voice: {}", self.tier, voice_model);
        
        // Check if model already exists
        let installed_models = self.piper.list_models()?;
        if installed_models.contains(&voice_model.to_string()) {
            info!("✅ Voice model {} already available", voice_model);
            return Ok(());
        }
        
        info!("📥 Downloading voice model: {}", voice_model);
        self.piper.download_model(voice_model)?;
        
        info!("✅ Voice model {} downloaded successfully", voice_model);
        Ok(())
    }
    
    /// Prompt user if they want to set up multi-device mesh
    fn prompt_mesh_setup(&self) -> Result<bool> {
        use std::io::{self, Write};
        
        print!("\n🌐 Set up multi-device mesh network? (Y/n): ");
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        let response = input.trim().to_lowercase();
        Ok(response.is_empty() || response == "y" || response == "yes")
    }
    
    /// Discover devices on local network with SSH enabled
    pub async fn discover_mesh_devices(&self) -> Result<Vec<DeviceCandidate>> {
        info!("🔍 Discovering devices on local network...");
        
        // Step 1: Ensure nmap is installed
        ensure_nmap_installed(&self.platform).await?;
        
        // Step 2: Scan local network
        let scanner = NetworkScanner::new()?;
        let devices = scanner.scan_local_network()?;
        
        // Step 3: Display discovered devices
        if devices.is_empty() {
            info!("⚠️  No devices with SSH found on local network");
            return Ok(devices);
        }
        
        info!("✅ Discovered {} devices with SSH enabled:", devices.len());
        for (i, device) in devices.iter().enumerate() {
            let hostname_display = device.hostname.as_deref().unwrap_or("unknown");
            info!("  [{}] {} ({})", i + 1, device.ip, hostname_display);
        }
        
        // Step 4: Assess device capabilities (if user wants to proceed)
        if self.prompt_assess_devices()? {
            let capabilities = self.assess_device_capabilities(&devices).await?;
            self.display_capabilities(&capabilities);
            
            // Step 5: Set up SSH keys and deploy (if user wants to proceed)
            if self.prompt_deploy_mesh()? {
                self.setup_and_deploy_mesh(&capabilities).await?;
            } else {
                info!("\n⚠️  Skipping mesh deployment");
                info!("📋 You can deploy later using the hainet-seed CLI");
            }
        } else {
            info!("\n⚠️  Skipping device assessment");
            info!("📋 Module 3 will handle remote deployment");
        }
        
        Ok(devices)
    }
    
    /// Prompt user if they want to deploy to mesh
    fn prompt_deploy_mesh(&self) -> Result<bool> {
        use std::io::{self, Write};
        
        print!("\n🚀 Deploy HAI-Net to discovered devices? (Y/n): ");
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        let response = input.trim().to_lowercase();
        Ok(response.is_empty() || response == "y" || response == "yes")
    }
    
    /// Set up SSH keys and deploy to mesh
    async fn setup_and_deploy_mesh(&self, capabilities: &[DeviceCapabilities]) -> Result<()> {
        info!("\n🔐 Setting up SSH keys and deploying to mesh...");
        
        // Step 1: Generate SSH key pair
        let key_manager = SSHKeyManager::new()?;
        key_manager.generate_key_pair("hainet-mesh")?;
        
        // Step 2: Get username for SSH
        use std::io::{self, Write};
        print!("\nSSH Username (default: current user): ");
        io::stdout().flush()?;
        let mut username = String::new();
        io::stdin().read_line(&mut username)?;
        let username = username.trim();
        let username = if username.is_empty() {
            std::env::var("USER").unwrap_or_else(|_| "root".to_string())
        } else {
            username.to_string()
        };
        
        // Step 3: Display manual key setup instructions (now automated)
        info!("\n📋 SSH Key Setup:");
        info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        info!("Public key location: {}", key_manager.public_key_path().display());
        info!("NOTE: Public key will be automatically distributed.");
        info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        
        // Step 4: Assign roles and deploy
        let mut orchestrator = DeploymentOrchestrator::new();
        orchestrator.assign_roles(capabilities.to_vec())?;
        
        // Ask for confirmation before deploying
        print!("\n⚠️  Ready to deploy. Continue? (Y/n): ");
        io::stdout().flush()?;
        let mut confirm = String::new();
        io::stdin().read_line(&mut confirm)?;
        let confirm = confirm.trim().to_lowercase();
        
        if confirm.is_empty() || confirm == "y" || confirm == "yes" {
            let client_factory = |ip: String, credentials: SSHCredentials| {
                SSHClient::new(ip, credentials)
            };
            orchestrator.deploy_all(&username, client_factory).await?;
            
            let summary = orchestrator.summary();
            info!("\n📊 Deployment Summary:");
            info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            info!("  Total Devices: {}", summary.total_devices);
            info!("  Master Nodes: {}", summary.master_count);
            info!("  Slave Nodes: {}", summary.slave_count);
            info!("  Standalone: {}", summary.standalone_count);
            info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        } else {
            info!("\n⚠️  Deployment cancelled by user");
        }
        
        Ok(())
    }
    
    /// Prompt user if they want to assess device capabilities
    fn prompt_assess_devices(&self) -> Result<bool> {
        use std::io::{self, Write};
        
        print!("\n🔍 Assess device capabilities via SSH? (Y/n): ");
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        let response = input.trim().to_lowercase();
        Ok(response.is_empty() || response == "y" || response == "yes")
    }
    
    /// Assess capabilities of discovered devices
    async fn assess_device_capabilities(&self, devices: &[DeviceCandidate]) -> Result<Vec<DeviceCapabilities>> {
        use std::io::{self, Write};
        
        let mut capabilities = Vec::new();
        
        // First, assess localhost capabilities (no SSH needed)
        info!("\n💻 Assessing localhost capabilities...");
        match self.assess_localhost_capabilities().await {
            Ok(localhost_caps) => {
                info!("✓ Localhost assessed: {} cores, {:.1}GB RAM, score: {:.1}", 
                      localhost_caps.cpu_cores, localhost_caps.ram_gb, localhost_caps.score);
                capabilities.push(localhost_caps);
            }
            Err(e) => {
                info!("⚠️  Failed to assess localhost: {}", e);
            }
        }
        
        // Then assess remote devices
        for device in devices {
            info!("\n🔍 Assessing device: {}", device.ip);
            
            // Retry loop for authentication
            loop {
                // Prompt for credentials per device (they might differ)
                print!("Username for {} (default: current user): ", device.ip);
                io::stdout().flush()?;
                let mut username = String::new();
                io::stdin().read_line(&mut username)?;
                let username = username.trim();
                let username = if username.is_empty() {
                    std::env::var("USER").unwrap_or_else(|_| "root".to_string())
                } else {
                    username.to_string()
                };
                
                print!("Password for {}@{}: ", username, device.ip);
                io::stdout().flush()?;
                let mut password = String::new();
                io::stdin().read_line(&mut password)?;
                let password = password.trim().to_string();
                
                let credentials = SSHCredentials { 
                    username: username.clone(), 
                    password: password.clone() 
                };
                
                // Create client and connect
                let mut client = SSHClient::new(device.ip.clone(), credentials);
                
                // Attempt to connect and authenticate
                let mut success = false;
                match client.connect() {
                    Ok(_) => {
                        // Authenticate with password
                        match client.authenticate_password() {
                            Ok(_) => {
                                info!("✓ Connected and authenticated successfully");

                                // Automatically copy SSH key after successful password auth
                                info!("✓ Distributing SSH key for passwordless access...");
                                let key_manager = SSHKeyManager::new()?;
                                match key_manager.copy_to_remote(&device.ip, &username, &password) {
                                    Ok(_) => {
                                        info!("✓ SSH key distributed successfully to {}", device.ip);
                                    }
                                    Err(e) => {
                                        info!("⚠️  Failed to distribute SSH key to {}: {}. Manual setup may be required.", device.ip, e);
                                        // We can still proceed, but deployment will likely fail.
                                    }
                                }
                                
                                // Now assess capabilities
                                match client.assess_capabilities() {
                                    Ok(caps) => {
                                        capabilities.push(caps);
                                        success = true;
                                    }
                                    Err(e) => {
                                        info!("⚠️  Failed to assess capabilities: {}", e);
                                    }
                                }
                                
                                // Disconnect
                                let _ = client.disconnect();
                            }
                            Err(e) => {
                                info!("⚠️  Authentication failed: {}", e);
                                
                                // Prompt: retry or skip?
                                print!("\nRetry with different credentials? (Y/n/s to skip): ");
                                io::stdout().flush()?;
                                let mut response = String::new();
                                io::stdin().read_line(&mut response)?;
                                let response = response.trim().to_lowercase();
                                
                                if response == "s" || response == "skip" {
                                    info!("⏭️  Skipping device {}", device.ip);
                                    break; // Skip this device
                                } else if response == "n" || response == "no" {
                                    info!("⏭️  Skipping device {}", device.ip);
                                    break; // Skip this device
                                }
                                // Otherwise loop to retry
                                continue;
                            }
                        }
                    }
                    Err(e) => {
                        info!("⚠️  Connection failed: {}", e);
                        
                        // Prompt: retry or skip?
                        print!("\nRetry connection? (Y/n/s to skip): ");
                        io::stdout().flush()?;
                        let mut response = String::new();
                        io::stdin().read_line(&mut response)?;
                        let response = response.trim().to_lowercase();
                        
                        if response == "s" || response == "skip" {
                            info!("⏭️  Skipping device {}", device.ip);
                            break; // Skip this device
                        } else if response == "n" || response == "no" {
                            info!("⏭️  Skipping device {}", device.ip);
                            break; // Skip this device
                        }
                        // Otherwise loop to retry
                        continue;
                    }
                }
                
                // If successful, break out of retry loop
                if success {
                    break;
                }
            }
        }
        
        Ok(capabilities)
    }
    
    /// Assess localhost capabilities without SSH
    async fn assess_localhost_capabilities(&self) -> Result<DeviceCapabilities> {
        use std::process::Command;
        use local_ip_address::local_ip;
        
        // Get local IP
        let local_ip = local_ip()
            .map(|ip| ip.to_string())
            .unwrap_or_else(|_| "localhost".to_string());
        
        // Get hostname
        let hostname = Command::new("hostname")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "localhost".to_string());
        
        // Get CPU cores
        let cpu_cores = num_cpus::get();
        
        // Get RAM in GB
        let ram_gb = SystemTier::get_total_ram_gb()? as f64;
        
        // Get GPU info (if available)
        let gpu = Command::new("lspci")
            .output()
            .ok()
            .and_then(|output| {
                let stdout = String::from_utf8_lossy(&output.stdout);
                stdout.lines()
                    .find(|line| line.to_lowercase().contains("vga") || 
                                 line.to_lowercase().contains("3d") ||
                                 line.to_lowercase().contains("display"))
                    .map(|s| s.to_string())
            });
        
        // Get available disk space in GB
        let disk_gb = Command::new("df")
            .args(&["-BG", "/"])
            .output()
            .ok()
            .and_then(|output| {
                let stdout = String::from_utf8_lossy(&output.stdout);
                stdout.lines()
                    .nth(1)
                    .and_then(|line| {
                        line.split_whitespace()
                            .nth(3)
                            .and_then(|s| s.trim_end_matches('G').parse::<f64>().ok())
                    })
            })
            .unwrap_or(100.0);
        
        // Get OS
        let os = Command::new("uname")
            .arg("-s")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "Unknown".to_string());
        
        // Get architecture
        let arch = Command::new("uname")
            .arg("-m")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "Unknown".to_string());
        
        let mut capabilities = DeviceCapabilities {
            ip: local_ip,
            hostname,
            cpu_cores,
            ram_gb,
            gpu,
            disk_gb,
            os,
            arch,
            score: 0.0,
        };
        
        capabilities.calculate_score();
        
        Ok(capabilities)
    }
    
    /// Display device capabilities and suggest master node
    fn display_capabilities(&self, capabilities: &[DeviceCapabilities]) {
        if capabilities.is_empty() {
            info!("\n⚠️  No device capabilities collected");
            return;
        }
        
        info!("\n📊 Device Capabilities Summary:");
        info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        
        for caps in capabilities {
            info!("Device: {} ({})", caps.hostname, caps.ip);
            info!("  CPU: {} cores", caps.cpu_cores);
            info!("  RAM: {:.1} GB", caps.ram_gb);
            info!("  GPU: {}", caps.gpu.as_deref().unwrap_or("None"));
            info!("  Disk: {:.1} GB available", caps.disk_gb);
            info!("  OS: {} ({})", caps.os, caps.arch);
            info!("  Score: {:.1}", caps.score);
            info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        }
        
        // Suggest master node (highest score)
        if let Some(master) = capabilities.iter().max_by(|a, b| a.score.partial_cmp(&b.score).unwrap()) {
            info!("\n🎯 Recommended Master Node: {} ({})", master.hostname, master.ip);
            info!("   Score: {:.1} (Best hardware for coordination)", master.score);
        }
        
        info!("\n⚠️  Remote deployment will be available in Module 3");
        info!("📋 Next: SSH key setup and automated deployment");
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
