//! # START OF FILE hainet-seed/src/installer/ssh_keys.rs
//! SSH key management for passwordless authentication.
//! Handles key generation, distribution, and verification.

use anyhow::{Result, Context, bail};
use std::path::{Path, PathBuf};
use std::fs;
use std::process::Command;
use std::net::TcpStream;
use ssh2::Session;
use std::io::{Read, Write};

/// SSH key pair manager
pub struct SSHKeyManager {
    /// Path to private key (~/.ssh/id_ed25519)
    private_key_path: PathBuf,
    /// Path to public key (~/.ssh/id_ed25519.pub)
    public_key_path: PathBuf,
}

impl SSHKeyManager {
    /// Create new SSH key manager with default paths
    pub fn new() -> Result<Self> {
        let home_dir = dirs::home_dir()
            .context("Cannot determine home directory")?;
        
        let ssh_dir = home_dir.join(".ssh");
        
        // Ensure .ssh directory exists
        if !ssh_dir.exists() {
            fs::create_dir_all(&ssh_dir)
                .context("Failed to create .ssh directory")?;
            
            // Set proper permissions (700)
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&ssh_dir)?.permissions();
                perms.set_mode(0o700);
                fs::set_permissions(&ssh_dir, perms)?;
            }
        }
        
        Ok(Self {
            private_key_path: ssh_dir.join("id_ed25519"),
            public_key_path: ssh_dir.join("id_ed25519.pub"),
        })
    }
    
    /// Check if SSH key pair already exists
    pub fn has_key_pair(&self) -> bool {
        self.private_key_path.exists() && self.public_key_path.exists()
    }
    
    /// Generate new SSH key pair using ed25519 algorithm
    /// 
    /// # Errors
    /// Returns an error if:
    /// - ssh-keygen command fails
    /// - Cannot set proper file permissions
    pub fn generate_key_pair(&self, comment: &str) -> Result<()> {
        println!("🔑 Generating SSH key pair...");
        
        if self.has_key_pair() {
            println!("✓ SSH key pair already exists");
            return Ok(());
        }
        
        // Generate key using ssh-keygen
        let output = Command::new("ssh-keygen")
            .arg("-t").arg("ed25519")
            .arg("-f").arg(&self.private_key_path)
            .arg("-N").arg("") // Empty passphrase
            .arg("-C").arg(comment)
            .output()
            .context("Failed to execute ssh-keygen. Please install openssh-client.")?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("ssh-keygen failed: {}", stderr);
        }
        
        // Set proper permissions on private key (600)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&self.private_key_path)?.permissions();
            perms.set_mode(0o600);
            fs::set_permissions(&self.private_key_path, perms)?;
        }
        
        println!("✓ SSH key pair generated at {}", self.private_key_path.display());
        
        Ok(())
    }
    
    /// Read public key contents
    pub fn read_public_key(&self) -> Result<String> {
        fs::read_to_string(&self.public_key_path)
            .context("Failed to read public key file")
    }
    
    /// Copy public key to remote device's authorized_keys
    /// 
    /// This enables passwordless SSH authentication.
    /// 
    /// # Note
    /// This is a placeholder. Actual implementation requires ssh2 crate
    /// to handle the file transfer over SSH.
    pub fn copy_to_remote(&self, ip: &str, username: &str, password: &str) -> Result<()> {
        println!("📤 Copying public key to {}@{}...", username, ip);

        let tcp = TcpStream::connect(format!("{}:22", ip))?;
        let mut sess = Session::new()?;
        sess.set_tcp_stream(tcp);
        sess.handshake()?;

        sess.userauth_password(username, password)?;

        let public_key = self.read_public_key()?;
        let mut channel = sess.channel_session()?;
        
        let command = format!(
            "mkdir -p ~/.ssh && echo '{}' >> ~/.ssh/authorized_keys && chmod 700 ~/.ssh && chmod 600 ~/.ssh/authorized_keys",
            public_key.trim()
        );
        
        channel.exec(&command)?;
        
        let mut s = String::new();
        channel.read_to_string(&mut s)?;
        
        channel.wait_close()?;
        
        let exit_code = channel.exit_status()?;
        if exit_code == 0 {
            println!("✓ Public key copied successfully.");
        } else {
            bail!("Failed to copy public key. Exit code: {}", exit_code);
        }

        Ok(())
    }
    
    /// Test SSH connection using key-based authentication
    /// 
    /// # Errors
    /// Returns an error if connection fails
    pub fn test_key_auth(&self, ip: &str, username: &str) -> Result<bool> {
        println!("🔐 Testing key-based authentication to {}@{}...", username, ip);
        
        let output = Command::new("ssh")
            .arg("-o").arg("BatchMode=yes") // Disable password prompt
            .arg("-o").arg("ConnectTimeout=5")
            .arg("-o").arg("StrictHostKeyChecking=no")
            .arg(format!("{}@{}", username, ip))
            .arg("echo 'Connection successful'")
            .output()
            .context("Failed to execute ssh command")?;
        
        if output.status.success() {
            println!("✓ Key-based authentication working");
            Ok(true)
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            println!("⚠️  Key-based authentication failed: {}", stderr);
            Ok(false)
        }
    }
    
    /// Get path to private key
    pub fn private_key_path(&self) -> &Path {
        &self.private_key_path
    }
    
    /// Get path to public key
    pub fn public_key_path(&self) -> &Path {
        &self.public_key_path
    }
}

impl Default for SSHKeyManager {
    fn default() -> Self {
        Self::new().expect("Failed to create SSHKeyManager")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_ssh_key_manager_creation() {
        let manager = SSHKeyManager::new();
        assert!(manager.is_ok());
        
        let manager = manager.unwrap();
        assert!(manager.private_key_path.to_str().unwrap().contains(".ssh/id_ed25519"));
        assert!(manager.public_key_path.to_str().unwrap().contains(".ssh/id_ed25519.pub"));
    }
    
    #[test]
    fn test_key_paths() {
        let manager = SSHKeyManager::new().unwrap();
        
        assert_eq!(
            manager.private_key_path().file_name().unwrap(),
            "id_ed25519"
        );
        
        assert_eq!(
            manager.public_key_path().file_name().unwrap(),
            "id_ed25519.pub"
        );
    }
}
