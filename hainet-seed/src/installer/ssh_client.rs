//! # START OF FILE hainet-seed/src/installer/ssh_client.rs
//! SSH client for connecting to and assessing remote devices.
//! Handles authentication, remote command execution, and capability assessment.

use anyhow::{Result, bail};
use std::net::TcpStream;
use std::time::Duration;

/// Device capabilities assessment result
#[derive(Debug, Clone)]
pub struct DeviceCapabilities {
    /// IP address
    pub ip: String,
    /// Hostname
    pub hostname: String,
    /// Number of CPU cores
    pub cpu_cores: usize,
    /// Total RAM in GB
    pub ram_gb: f64,
    /// GPU description (if available)
    pub gpu: Option<String>,
    /// Available disk space in GB
    pub disk_gb: f64,
    /// Operating system
    pub os: String,
    /// CPU architecture (x86_64, aarch64, armv7l)
    pub arch: String,
    /// Capability score (for master election)
    pub score: f64,
}

impl DeviceCapabilities {
    /// Calculate a capability score for master election.
    /// Higher score = better candidate for master.
    /// 
    /// Scoring formula:
    /// - RAM: 40% weight (most important)
    /// - GPU: 30% weight (important for AI workloads)
    /// - CPU: 20% weight
    /// - Disk: 10% weight
    pub fn calculate_score(&mut self) {
        let ram_score = self.ram_gb * 10.0; // 16GB = 160 points
        let gpu_score = if self.gpu.is_some() { 100.0 } else { 0.0 };
        let cpu_score = self.cpu_cores as f64 * 5.0; // 8 cores = 40 points
        let disk_score = self.disk_gb; // 100GB = 100 points
        
        // Weighted sum
        self.score = (ram_score * 0.4) + (gpu_score * 0.3) + (cpu_score * 0.2) + (disk_score * 0.1);
    }
}

/// SSH connection credentials
#[derive(Debug, Clone)]
pub struct SSHCredentials {
    pub username: String,
    pub password: String,
}

/// SSH client for remote device access
pub struct SSHClient {
    ip: String,
    #[allow(dead_code)] // Will be used when ssh2 crate is integrated
    credentials: SSHCredentials,
}

impl SSHClient {
    /// Create a new SSH client for a device
    pub fn new(ip: String, credentials: SSHCredentials) -> Self {
        Self { ip, credentials }
    }
    
    /// Test SSH connection to the device
    /// 
    /// # Errors
    /// Returns an error if:
    /// - Cannot connect to SSH port (22)
    /// - Connection times out (5 seconds)
    pub fn test_connection(&self) -> Result<bool> {
        println!("Testing SSH connection to {}...", self.ip);
        
        let addr = format!("{}:22", self.ip);
        let timeout = Duration::from_secs(5);
        
        match TcpStream::connect_timeout(&addr.parse()?, timeout) {
            Ok(_) => {
                println!("✓ SSH port is reachable on {}", self.ip);
                Ok(true)
            }
            Err(e) => {
                bail!("Cannot connect to SSH on {}: {}", self.ip, e);
            }
        }
    }
    
    /// Assess device capabilities via SSH
    /// 
    /// Runs remote commands to gather:
    /// - CPU info (cores)
    /// - RAM info (total GB)
    /// - GPU info (lspci)
    /// - Disk space (df)
    /// - OS and architecture (uname)
    /// 
    /// # Note
    /// This is a placeholder implementation. Actual SSH command execution
    /// requires the `ssh2` crate integration in the next iteration.
    pub async fn assess_capabilities(&self) -> Result<DeviceCapabilities> {
        println!("Assessing capabilities of {}...", self.ip);
        
        // Placeholder: In production, these would be actual SSH commands
        // For now, return mock data to demonstrate the structure
        
        // TODO: Execute actual SSH commands:
        // - lscpu | grep "^CPU(s):" | awk '{print $2}'
        // - free -g | grep Mem | awk '{print $2}'
        // - lspci | grep VGA
        // - df -h / | tail -1 | awk '{print $4}'
        // - uname -a
        
        let mut capabilities = DeviceCapabilities {
            ip: self.ip.clone(),
            hostname: format!("device-{}", self.ip.split('.').last().unwrap_or("unknown")),
            cpu_cores: 4, // Mock data
            ram_gb: 8.0,  // Mock data
            gpu: None,
            disk_gb: 100.0, // Mock data
            os: "Linux".to_string(),
            arch: "x86_64".to_string(),
            score: 0.0,
        };
        
        capabilities.calculate_score();
        
        println!("✓ Device {} assessed: {} cores, {:.1}GB RAM, score: {:.1}", 
                 self.ip, capabilities.cpu_cores, capabilities.ram_gb, capabilities.score);
        
        Ok(capabilities)
    }
    
    /// Execute a command on the remote device via SSH
    /// 
    /// # Errors
    /// Returns an error if command execution fails
    /// 
    /// # Note
    /// This is a placeholder. Actual implementation requires `ssh2` crate.
    pub async fn execute_command(&self, _command: &str) -> Result<String> {
        // TODO: Implement actual SSH command execution
        // Using ssh2 crate: Session::connect, authenticate, channel_session, exec
        bail!("SSH command execution not yet implemented. Requires ssh2 crate integration.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_capability_score_calculation() {
        let mut caps = DeviceCapabilities {
            ip: "192.168.1.10".to_string(),
            hostname: "test-device".to_string(),
            cpu_cores: 8,
            ram_gb: 16.0,
            gpu: Some("NVIDIA RTX3060".to_string()),
            disk_gb: 500.0,
            os: "Linux".to_string(),
            arch: "x86_64".to_string(),
            score: 0.0,
        };
        
        caps.calculate_score();
        
        // Expected score:
        // RAM: 16 * 10 * 0.4 = 64
        // GPU: 100 * 0.3 = 30
        // CPU: 8 * 5 * 0.2 = 8
        // Disk: 500 * 0.1 = 50
        // Total: 152
        
        assert!(caps.score > 150.0 && caps.score < 155.0);
    }
    
    #[test]
    fn test_capability_score_no_gpu() {
        let mut caps = DeviceCapabilities {
            ip: "192.168.1.20".to_string(),
            hostname: "laptop".to_string(),
            cpu_cores: 4,
            ram_gb: 8.0,
            gpu: None,
            disk_gb: 250.0,
            os: "Linux".to_string(),
            arch: "x86_64".to_string(),
            score: 0.0,
        };
        
        caps.calculate_score();
        
        // Expected score:
        // RAM: 8 * 10 * 0.4 = 32
        // GPU: 0 * 0.3 = 0
        // CPU: 4 * 5 * 0.2 = 4
        // Disk: 250 * 0.1 = 25
        // Total: 61
        
        assert!(caps.score > 60.0 && caps.score < 62.0);
    }
    
    #[tokio::test]
    async fn test_ssh_client_creation() {
        let creds = SSHCredentials {
            username: "testuser".to_string(),
            password: "testpass".to_string(),
        };
        
        let client = SSHClient::new("192.168.1.10".to_string(), creds);
        
        assert_eq!(client.ip, "192.168.1.10");
    }
    
    #[tokio::test]
    async fn test_assess_capabilities_mock() {
        let creds = SSHCredentials {
            username: "testuser".to_string(),
            password: "testpass".to_string(),
        };
        
        let client = SSHClient::new("192.168.1.10".to_string(), creds);
        let result = client.assess_capabilities().await;
        
        // Should succeed with mock data
        assert!(result.is_ok());
        
        let caps = result.unwrap();
        assert_eq!(caps.ip, "192.168.1.10");
        assert!(caps.cpu_cores > 0);
        assert!(caps.ram_gb > 0.0);
        assert!(caps.score > 0.0);
    }
}
