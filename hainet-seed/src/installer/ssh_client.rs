//! # START OF FILE hainet-seed/src/installer/ssh_client.rs
//! SSH client for connecting to and assessing remote devices.
//! Handles authentication, remote command execution, and capability assessment.

use anyhow::{Result, bail, Context};
use ssh2::Session;
use std::io::Read;
use std::net::TcpStream;
use std::path::Path;
use std::time::Duration;

// A trait for SSH client operations, to allow for mocking in tests.
pub trait SSHClientTrait {
    fn connect(&mut self) -> Result<()>;
    fn authenticate_password(&mut self) -> Result<()>;
    fn authenticate_pubkey(&mut self, private_key_path: &Path, passphrase: Option<&str>) -> Result<()>;
    fn disconnect(&mut self) -> Result<()>;
    fn is_connected(&self) -> bool;
    fn assess_capabilities(&self) -> Result<DeviceCapabilities>;
    fn execute_command(&self, command: &str) -> Result<String>;
    fn upload_file(&self, local_path: &Path, remote_path: &str) -> Result<()>;
    fn create_remote_directory(&self, path: &str) -> Result<()>;
    fn set_permissions(&self, path: &str, mode: u32) -> Result<()>;
}


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
}

impl SSHClientTrait for SSHClient {
    /// Establish SSH connection to the device
    /// 
    /// # Errors
    /// Returns an error if:
    /// - Cannot connect to SSH port (22)
    /// - Connection times out (5 seconds)
    /// - SSH handshake fails
    fn connect(&mut self) -> Result<()> {
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
    fn authenticate_password(&mut self) -> Result<()> {
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
    fn authenticate_pubkey(&mut self, private_key_path: &Path, passphrase: Option<&str>) -> Result<()> {
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
        
    /// Disconnect SSH session
    fn disconnect(&mut self) -> Result<()> {
        if let Some(session) = self.session.take() {
            session.disconnect(None, "Client disconnecting", None)?;
            println!("✓ Disconnected from {}", self.ip);
        }
        Ok(())
    }
    
    /// Check if client is connected and authenticated
    fn is_connected(&self) -> bool {
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
    fn assess_capabilities(&self) -> Result<DeviceCapabilities> {
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
    fn execute_command(&self, command: &str) -> Result<String> {
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
    fn upload_file(&self, local_path: &Path, remote_path: &str) -> Result<()> {
        if !self.is_connected() {
            bail!("Not connected to device. Call connect() and authenticate first.");
        }

        println!("Uploading {} to {}:{}...", local_path.display(), self.ip, remote_path);

        let session = self.session.as_ref().unwrap();
        let sftp = session.sftp().context("Failed to create SFTP session")?;

        let local_content =
            std::fs::read(local_path).context(format!("Failed to read local file: {}", local_path.display()))?;

        let remote_filename = Path::new(remote_path)
            .file_name()
            .and_then(|s| s.to_str())
            .context("Could not get remote filename")?;
        let temp_remote_path = format!("/tmp/{}", remote_filename);

        // Write to temp remote file
        let mut remote_file = sftp
            .create(Path::new(&temp_remote_path))
            .context(format!("Failed to create temporary remote file: {}", temp_remote_path))?;

        std::io::copy(&mut local_content.as_slice(), &mut remote_file)
            .context("Failed to write file content to temp file")?;
        drop(remote_file);

        // Ensure parent directory exists on remote
        if let Some(parent) = Path::new(remote_path).parent() {
            if let Some(parent_str) = parent.to_str() {
                if !parent_str.is_empty() {
                    self.create_remote_directory(parent_str)?;
                }
            }
        }
        
        // Move the file to the destination (no sudo needed for user-owned files)
        self.execute_command(&format!("mv {} {}", temp_remote_path, remote_path))?;

        println!("✓ Uploaded {} ({} bytes)", remote_path, local_content.len());

        Ok(())
    }
        
    /// Create a directory on the remote device
    /// 
    /// # Arguments
    /// * `path` - Directory path to create
    /// 
    /// # Errors
    /// Returns an error if directory creation fails
    fn create_remote_directory(&self, path: &str) -> Result<()> {
        // Create user-owned directories without sudo
        self.execute_command(&format!("mkdir -p {}", path))?;
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
    fn set_permissions(&self, path: &str, mode: u32) -> Result<()> {
        let mode_octal = format!("{:o}", mode);
        // User-owned files don't need sudo for chmod
        self.execute_command(&format!("chmod {} {}", mode_octal, path))?;
        println!("✓ Set permissions {} on {}", mode_octal, path);
        Ok(())
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

    // A mock SSH client for testing the deployment orchestrator.
    #[derive(Clone)]
    pub struct MockSSHClient {
        pub ip: String,
        connected: bool,
        commands: std::collections::HashMap<String, String>,
        mock_capabilities: Option<DeviceCapabilities>,
    }

    impl MockSSHClient {
        pub fn new(ip: String) -> Self {
            Self {
                ip,
                connected: false,
                commands: std::collections::HashMap::new(),
                mock_capabilities: None,
            }
        }

        #[allow(dead_code)]
        pub fn expect_command(&mut self, command: &str, output: &str) {
            self.commands.insert(command.to_string(), output.to_string());
        }

        #[allow(dead_code)]
        pub fn set_capabilities(&mut self, caps: DeviceCapabilities) {
            self.mock_capabilities = Some(caps);
        }
    }

    impl SSHClientTrait for MockSSHClient {
        fn connect(&mut self) -> Result<()> {
            self.connected = true;
            Ok(())
        }

        fn authenticate_password(&mut self) -> Result<()> {
            if !self.connected {
                bail!("Not connected");
            }
            Ok(())
        }

        fn authenticate_pubkey(&mut self, _private_key_path: &Path, _passphrase: Option<&str>) -> Result<()> {
            if !self.connected {
                bail!("Not connected");
            }
            Ok(())
        }

        fn disconnect(&mut self) -> Result<()> {
            self.connected = false;
            Ok(())
        }

        fn is_connected(&self) -> bool {
            self.connected
        }

        fn assess_capabilities(&self) -> Result<DeviceCapabilities> {
            if !self.connected {
                bail!("Not connected");
            }
            self.mock_capabilities.clone().context("Mock capabilities not set")
        }

        fn execute_command(&self, command: &str) -> Result<String> {
            if !self.connected {
                bail!("Not connected");
            }
            self.commands.get(command).cloned().context(format!("Unexpected command: {}", command))
        }

        fn upload_file(&self, _local_path: &Path, _remote_path: &str) -> Result<()> {
            if !self.connected {
                bail!("Not connected");
            }
            Ok(())
        }

        fn create_remote_directory(&self, _path: &str) -> Result<()> {
            if !self.connected {
                bail!("Not connected");
            }
            Ok(())
        }

        fn set_permissions(&self, _path: &str, _mode: u32) -> Result<()> {
            if !self.connected {
                bail!("Not connected");
            }
            Ok(())
        }
    }

    #[test]
    fn test_mock_ssh_client() {
        let mut mock_client = MockSSHClient::new("127.0.0.1".to_string());
        assert!(!mock_client.is_connected());

        mock_client.connect().unwrap();
        assert!(mock_client.is_connected());

        mock_client.expect_command("hostname", "mock-device");
        let hostname = mock_client.execute_command("hostname").unwrap();
        assert_eq!(hostname, "mock-device");

        let caps = DeviceCapabilities {
            ip: "127.0.0.1".to_string(),
            hostname: "mock-device".to_string(),
            cpu_cores: 4,
            ram_gb: 8.0,
            gpu: None,
            disk_gb: 100.0,
            os: "Linux".to_string(),
            arch: "x86_64".to_string(),
            score: 61.0,
        };
        mock_client.set_capabilities(caps.clone());
        let assessed_caps = mock_client.assess_capabilities().unwrap();
        assert_eq!(assessed_caps.hostname, caps.hostname);

        mock_client.disconnect().unwrap();
        assert!(!mock_client.is_connected());
    }
}
