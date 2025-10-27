//! # START OF FILE hainet-seed/src/installer/ssh_client.rs
//! SSH client for connecting to and assessing remote devices.
//! Handles authentication, remote command execution, and capability assessment.

use anyhow::{Result, bail, Context};
use ssh2::Session;
use std::io::Read;
use std::net::TcpStream;
use std::path::Path;
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
    credentials: SSHCredentials,
    session: Option<Session>,
}

impl SSHClient {
    /// Create a new SSH client for a device
    pub fn new(ip: String, credentials: SSHCredentials) -> Self {
        Self { 
            ip, 
            credentials,
            session: None,
        }
    }
    
    /// Establish SSH connection to the device
    /// 
    /// # Errors
    /// Returns an error if:
    /// - Cannot connect to SSH port (22)
    /// - Connection times out (5 seconds)
    /// - SSH handshake fails
    pub fn connect(&mut self) -> Result<()> {
        println!("Connecting to {}...", self.ip);
        
        let addr = format!("{}:22", self.ip);
        let timeout = Duration::from_secs(5);
        
        let tcp = TcpStream::connect_timeout(&addr.parse()?, timeout)
            .context(format!("Cannot connect to SSH on {}", self.ip))?;
        
        // Set read timeout for the session
        tcp.set_read_timeout(Some(Duration::from_secs(30)))?;
        
        let mut session = Session::new()?;
        session.set_tcp_stream(tcp);
        session.handshake()
            .context("SSH handshake failed")?;
        
        println!("✓ SSH connection established to {}", self.ip);
        self.session = Some(session);
        
        Ok(())
    }
    
    /// Authenticate with password
    /// 
    /// # Errors
    /// Returns an error if authentication fails
    pub fn authenticate_password(&mut self) -> Result<()> {
        let session = self.session.as_mut()
            .context("No active session. Call connect() first")?;
        
        println!("Authenticating as {}...", self.credentials.username);
        
        session.userauth_password(&self.credentials.username, &self.credentials.password)
            .context("Password authentication failed")?;
        
        if !session.authenticated() {
            bail!("Authentication failed for user {}", self.credentials.username);
        }
        
        println!("✓ Authenticated successfully");
        Ok(())
    }
    
    /// Authenticate with SSH key
    /// 
    /// # Arguments
    /// * `private_key_path` - Path to private key file (e.g., ~/.ssh/id_ed25519)
    /// * `passphrase` - Optional passphrase for encrypted keys
    /// 
    /// # Errors
    /// Returns an error if authentication fails
    pub fn authenticate_pubkey(&mut self, private_key_path: &Path, passphrase: Option<&str>) -> Result<()> {
        let session = self.session.as_mut()
            .context("No active session. Call connect() first")?;
        
        println!("Authenticating with SSH key...");
        
        session.userauth_pubkey_file(
            &self.credentials.username,
            None, // Public key path (auto-detected from private key)
            private_key_path,
            passphrase,
        ).context("Public key authentication failed")?;
        
        if !session.authenticated() {
            bail!("Key authentication failed for user {}", self.credentials.username);
        }
        
        println!("✓ Authenticated successfully with SSH key");
        Ok(())
    }
    
    /// Test SSH connection to the device (legacy method for backward compatibility)
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
    
    /// Disconnect SSH session
    pub fn disconnect(&mut self) -> Result<()> {
        if let Some(session) = self.session.take() {
            session.disconnect(None, "Client disconnecting", None)?;
            println!("✓ Disconnected from {}", self.ip);
        }
        Ok(())
    }
    
    /// Check if client is connected and authenticated
    pub fn is_connected(&self) -> bool {
        self.session.as_ref()
            .map(|s| s.authenticated())
            .unwrap_or(false)
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
    /// # Errors
    /// Returns an error if SSH commands fail or device is not connected
    pub fn assess_capabilities(&self) -> Result<DeviceCapabilities> {
        if !self.is_connected() {
            bail!("Not connected to device. Call connect() and authenticate first.");
        }
        
        println!("Assessing capabilities of {}...", self.ip);
        
        // Get CPU cores (works on Linux, macOS, Termux)
        let cpu_cores = self.get_cpu_cores()?;
        
        // Get RAM in GB
        let ram_gb = self.get_ram_gb()?;
        
        // Get GPU info (may be None)
        let gpu = self.get_gpu_info().ok();
        
        // Get available disk space in GB
        let disk_gb = self.get_disk_space_gb()?;
        
        // Get OS and architecture
        let os = self.get_os()?;
        let arch = self.get_architecture()?;
        
        // Get hostname
        let hostname = self.get_hostname()?;
        
        let mut capabilities = DeviceCapabilities {
            ip: self.ip.clone(),
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
        
        println!("✓ Device {} assessed: {} cores, {:.1}GB RAM, score: {:.1}", 
                 self.ip, capabilities.cpu_cores, capabilities.ram_gb, capabilities.score);
        
        Ok(capabilities)
    }
    
    /// Get number of CPU cores
    fn get_cpu_cores(&self) -> Result<usize> {
        // Try nproc first (most reliable on Linux)
        let output = self.execute_command("nproc 2>/dev/null || grep -c processor /proc/cpuinfo 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 1")?;
        output.parse::<usize>()
            .context("Failed to parse CPU core count")
    }
    
    /// Get total RAM in GB
    fn get_ram_gb(&self) -> Result<f64> {
        // Try free -g (Linux), fall back to calculating from KB
        let output = self.execute_command(
            "free -g 2>/dev/null | awk '/^Mem:/ {print $2}' || \
             free -k 2>/dev/null | awk '/^Mem:/ {print int($2/1024/1024)}' || \
             sysctl -n hw.memsize 2>/dev/null | awk '{print int($1/1024/1024/1024)}' || \
             echo 1"
        )?;
        
        let ram_gb = output.parse::<f64>()
            .context("Failed to parse RAM size")?;
        
        // Ensure at least 1GB reported
        Ok(ram_gb.max(1.0))
    }
    
    /// Get GPU info (if available)
    fn get_gpu_info(&self) -> Result<String> {
        // Try lspci for VGA devices
        let output = self.execute_command("lspci 2>/dev/null | grep -i 'vga\\|3d\\|display' | head -1")?;
        
        if output.is_empty() {
            bail!("No GPU detected");
        }
        
        Ok(output)
    }
    
    /// Get available disk space in GB
    fn get_disk_space_gb(&self) -> Result<f64> {
        // Get available space on root filesystem
        let output = self.execute_command(
            "df -BG / 2>/dev/null | awk 'NR==2 {gsub(\"G\",\"\",$4); print $4}' || \
             df -k / 2>/dev/null | awk 'NR==2 {print int($4/1024/1024)}' || \
             echo 10"
        )?;
        
        output.parse::<f64>()
            .context("Failed to parse disk space")
    }
    
    /// Get operating system
    fn get_os(&self) -> Result<String> {
        let output = self.execute_command("uname -s")?;
        Ok(output)
    }
    
    /// Get CPU architecture
    fn get_architecture(&self) -> Result<String> {
        let output = self.execute_command("uname -m")?;
        Ok(output)
    }
    
    /// Get hostname
    fn get_hostname(&self) -> Result<String> {
        let output = self.execute_command("hostname")?;
        Ok(output)
    }
    
    /// Execute a command on the remote device via SSH
    /// 
    /// # Arguments
    /// * `command` - Shell command to execute
    /// 
    /// # Returns
    /// Returns stdout from the command as a String
    /// 
    /// # Errors
    /// Returns an error if:
    /// - No active session exists
    /// - Channel creation fails
    /// - Command execution fails
    /// - Reading output fails
    pub fn execute_command(&self, command: &str) -> Result<String> {
        let session = self.session.as_ref()
            .context("No active session. Call connect() and authenticate first")?;
        
        let mut channel = session.channel_session()
            .context("Failed to create SSH channel")?;
        
        channel.exec(command)
            .context(format!("Failed to execute command: {}", command))?;
        
        let mut output = String::new();
        channel.read_to_string(&mut output)
            .context("Failed to read command output")?;
        
        channel.wait_close()
            .context("Failed to close channel")?;
        
        let exit_status = channel.exit_status()?;
        if exit_status != 0 {
            bail!("Command failed with exit code {}: {}", exit_status, command);
        }
        
        Ok(output.trim().to_string())
    }
    
    /// Execute a command with timeout
    /// 
    /// # Arguments
    /// * `command` - Shell command to execute
    /// * `timeout` - Maximum execution time
    /// 
    /// # Returns
    /// Returns stdout from the command as a String
    /// 
    /// # Errors
    /// Returns an error if command fails or times out
    pub fn execute_command_with_timeout(&self, command: &str, _timeout: Duration) -> Result<String> {
        // Note: ssh2 doesn't have built-in timeout for command execution
        // For now, we use the session's read timeout set during connect()
        // In the future, we could spawn a thread with timeout handling
        self.execute_command(command)
    }
    
    /// Upload a file to the remote device via SFTP
    /// 
    /// # Arguments
    /// * `local_path` - Path to local file
    /// * `remote_path` - Destination path on remote device
    /// 
    /// # Errors
    /// Returns an error if:
    /// - Not connected/authenticated
    /// - Local file doesn't exist
    /// - SFTP session creation fails
    /// - File transfer fails
    pub fn upload_file(&self, local_path: &Path, remote_path: &str) -> Result<()> {
        if !self.is_connected() {
            bail!("Not connected to device. Call connect() and authenticate first.");
        }
        
        println!("Uploading {} to {}:{}...", local_path.display(), self.ip, remote_path);
        
        let session = self.session.as_ref().unwrap();
        let sftp = session.sftp()
            .context("Failed to create SFTP session")?;
        
        // Read local file
        let local_content = std::fs::read(local_path)
            .context(format!("Failed to read local file: {}", local_path.display()))?;
        
        // Create parent directory if needed
        if let Some(parent) = Path::new(remote_path).parent() {
            let parent_str = parent.to_str().unwrap_or("");
            if !parent_str.is_empty() {
                self.create_remote_directory(parent_str)?;
            }
        }
        
        // Write to remote file
        let mut remote_file = sftp.create(Path::new(remote_path))
            .context(format!("Failed to create remote file: {}", remote_path))?;
        
        std::io::copy(&mut local_content.as_slice(), &mut remote_file)
            .context("Failed to write file content")?;
        
        println!("✓ Uploaded {} ({} bytes)", remote_path, local_content.len());
        
        Ok(())
    }
    
    /// Download a file from the remote device via SFTP
    /// 
    /// # Arguments
    /// * `remote_path` - Path on remote device
    /// * `local_path` - Destination path on local machine
    /// 
    /// # Errors
    /// Returns an error if:
    /// - Not connected/authenticated
    /// - Remote file doesn't exist
    /// - SFTP session creation fails
    /// - File transfer fails
    pub fn download_file(&self, remote_path: &str, local_path: &Path) -> Result<()> {
        if !self.is_connected() {
            bail!("Not connected to device. Call connect() and authenticate first.");
        }
        
        println!("Downloading {}:{} to {}...", self.ip, remote_path, local_path.display());
        
        let session = self.session.as_ref().unwrap();
        let sftp = session.sftp()
            .context("Failed to create SFTP session")?;
        
        // Create parent directory if needed
        if let Some(parent) = local_path.parent() {
            std::fs::create_dir_all(parent)
                .context("Failed to create local directory")?;
        }
        
        // Read from remote file
        let mut remote_file = sftp.open(Path::new(remote_path))
            .context(format!("Failed to open remote file: {}", remote_path))?;
        
        // Write to local file
        let mut local_file = std::fs::File::create(local_path)
            .context(format!("Failed to create local file: {}", local_path.display()))?;
        
        std::io::copy(&mut remote_file, &mut local_file)
            .context("Failed to read file content")?;
        
        println!("✓ Downloaded {}", local_path.display());
        
        Ok(())
    }
    
    /// Create a directory on the remote device
    /// 
    /// # Arguments
    /// * `path` - Directory path to create
    /// 
    /// # Errors
    /// Returns an error if directory creation fails
    pub fn create_remote_directory(&self, path: &str) -> Result<()> {
        // Use mkdir -p to create parent directories recursively
        // Redirect errors to /dev/null and always succeed (directory might already exist)
        self.execute_command(&format!("mkdir -p {} 2>/dev/null || true", path))?;
        Ok(())
    }
    
    /// Set file permissions on the remote device
    /// 
    /// # Arguments
    /// * `path` - File path
    /// * `mode` - Unix permissions mode (e.g., 0o755 for rwxr-xr-x)
    /// 
    /// # Errors
    /// Returns an error if chmod fails
    pub fn set_permissions(&self, path: &str, mode: u32) -> Result<()> {
        let mode_octal = format!("{:o}", mode);
        self.execute_command(&format!("chmod {} {}", mode_octal, path))?;
        println!("✓ Set permissions {} on {}", mode_octal, path);
        Ok(())
    }
    
    /// Check if a file exists on the remote device
    /// 
    /// # Arguments
    /// * `path` - File path to check
    /// 
    /// # Returns
    /// Returns true if file exists, false otherwise
    pub fn remote_file_exists(&self, path: &str) -> Result<bool> {
        let result = self.execute_command(&format!("test -e {} && echo 1 || echo 0", path))?;
        Ok(result == "1")
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
    
    #[test]
    fn test_is_connected() {
        let creds = SSHCredentials {
            username: "testuser".to_string(),
            password: "testpass".to_string(),
        };
        
        let client = SSHClient::new("192.168.1.10".to_string(), creds);
        
        // Should not be connected initially
        assert!(!client.is_connected());
    }
}
