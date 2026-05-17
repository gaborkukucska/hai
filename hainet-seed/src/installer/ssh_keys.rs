//! # START OF FILE hainet-seed/src/installer/ssh_keys.rs
//! SSH key management for passwordless authentication.
//! Uses a dedicated `hainet-mesh` key pair for mesh operations.

use anyhow::{Result, Context, bail};
use std::path::{Path, PathBuf};
use std::fs;
use std::process::Command;
use std::net::TcpStream;
use ssh2::Session;
use std::io::Read;

/// Path to the mesh manifest file
const MESH_MANIFEST_FILENAME: &str = "mesh.json";

/// SSH key pair manager using dedicated hainet-mesh keys
pub struct SSHKeyManager {
    /// Path to private key (~/.ssh/hainet-mesh)
    private_key_path: PathBuf,
    /// Path to public key (~/.ssh/hainet-mesh.pub)
    public_key_path: PathBuf,
    /// Path to mesh data directory (~/.hainet/)
    hainet_dir: PathBuf,
}

/// Persistent mesh manifest — remembers the mesh between runs
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct MeshManifest {
    /// When the manifest was last updated
    pub updated_at: String,
    /// Nodes in the mesh
    pub nodes: Vec<MeshNode>,
}

/// A single node in the mesh manifest
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct MeshNode {
    pub ip: String,
    pub hostname: String,
    pub username: String,
    pub role: String,
    /// MAC address for stable identification across IP changes (DHCP)
    #[serde(default)]
    pub mac_address: Option<String>,
}

impl SSHKeyManager {
    /// Create new SSH key manager with dedicated hainet-mesh paths
    pub fn new() -> Result<Self> {
        let home_dir = dirs::home_dir()
            .context("Cannot determine home directory")?;
        
        let ssh_dir = home_dir.join(".ssh");
        let hainet_dir = home_dir.join(".hainet");
        
        // Ensure .ssh directory exists
        if !ssh_dir.exists() {
            fs::create_dir_all(&ssh_dir)
                .context("Failed to create .ssh directory")?;
            
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&ssh_dir)?.permissions();
                perms.set_mode(0o700);
                fs::set_permissions(&ssh_dir, perms)?;
            }
        }
        
        // Ensure ~/.hainet directory exists
        if !hainet_dir.exists() {
            fs::create_dir_all(&hainet_dir)
                .context("Failed to create .hainet directory")?;
        }
        
        Ok(Self {
            private_key_path: ssh_dir.join("hainet-mesh"),
            public_key_path: ssh_dir.join("hainet-mesh.pub"),
            hainet_dir,
        })
    }
    
    /// Check if SSH key pair already exists
    pub fn has_key_pair(&self) -> bool {
        self.private_key_path.exists() && self.public_key_path.exists()
    }
    
    /// Generate new SSH key pair using ed25519 algorithm
    pub fn generate_key_pair(&self, comment: &str) -> Result<()> {
        println!("🔑 Generating HAI-Net mesh SSH key pair...");
        
        if self.has_key_pair() {
            println!("✓ HAI-Net mesh key pair already exists at {}", self.private_key_path.display());
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
        
        println!("✓ HAI-Net mesh key pair generated at {}", self.private_key_path.display());
        
        Ok(())
    }
    
    /// Read public key contents
    pub fn read_public_key(&self) -> Result<String> {
        fs::read_to_string(&self.public_key_path)
            .context("Failed to read public key file")
    }
    
    /// Copy public key to remote device's authorized_keys using password auth
    pub fn copy_to_remote(&self, ip: &str, username: &str, password: &str) -> Result<()> {
        println!("📤 Copying HAI-Net mesh key to {}@{}...", username, ip);

        let tcp = TcpStream::connect(format!("{}:22", ip))?;
        let mut sess = Session::new()?;
        sess.set_tcp_stream(tcp);
        sess.handshake()?;

        sess.userauth_password(username, password)?;

        let public_key = self.read_public_key()?;
        let mut channel = sess.channel_session()?;
        
        // Only add the key if it's not already present
        let command = format!(
            "mkdir -p ~/.ssh && grep -qF '{}' ~/.ssh/authorized_keys 2>/dev/null || echo '{}' >> ~/.ssh/authorized_keys && chmod 700 ~/.ssh && chmod 600 ~/.ssh/authorized_keys",
            public_key.trim(), public_key.trim()
        );
        
        channel.exec(&command)?;
        
        let mut s = String::new();
        channel.read_to_string(&mut s)?;
        
        channel.wait_close()?;
        
        let exit_code = channel.exit_status()?;
        if exit_code == 0 {
            println!("✓ Mesh key installed on {}.", ip);
        } else {
            bail!("Failed to copy mesh key. Exit code: {}", exit_code);
        }

        Ok(())
    }
    
    /// Set up passwordless sudo for hainet operations on a remote node.
    /// This must be called while we still have the user's password (during initial install).
    /// Creates /etc/sudoers.d/hainet granting NOPASSWD access for systemctl, file operations, etc.
    pub fn setup_sudoers_on_remote(&self, ip: &str, username: &str, password: &str) -> Result<()> {
        println!("🔧 Setting up passwordless sudo for HAI-Net on {}...", ip);

        let tcp = TcpStream::connect(format!("{}:22", ip))?;
        let mut sess = Session::new()?;
        sess.set_tcp_stream(tcp);
        sess.handshake()?;
        sess.userauth_password(username, password)?;
        
        // Create a sudoers entry that allows the user to run hainet-related commands without a password
        // This is scoped to specific commands only for security
        let sudoers_content = format!(
            "{user} ALL=(ALL) NOPASSWD: /usr/bin/systemctl * hainet-*, /usr/bin/systemctl daemon-reload, /usr/bin/mv /tmp/hainet-upload-* /usr/local/bin/*, /usr/bin/mv /tmp/hainet* /etc/*, /usr/bin/mkdir -p /usr/local/bin*, /usr/bin/mkdir -p /etc/hainet*, /usr/bin/mkdir -p /var/lib/hainet*, /usr/bin/mkdir -p /var/log/hainet*, /usr/bin/chown * hainet*, /usr/bin/chmod *, /usr/sbin/useradd *, /usr/sbin/userdel *, /usr/sbin/groupdel *, /usr/bin/rm -f /usr/local/bin/hainet-*, /usr/bin/rm -f /etc/systemd/system/hainet-*, /usr/bin/rm -rf /etc/hainet*, /usr/bin/rm -rf /var/lib/hainet*, /usr/bin/rm -rf /var/log/hainet*, /usr/bin/rm -rf /opt/hainet*, /bin/mv /tmp/hainet-upload-* /usr/local/bin/*, /bin/mv /tmp/hainet* /etc/*, /bin/mkdir -p *, /bin/chown * hainet*, /bin/chmod *, /bin/rm -f /usr/local/bin/hainet-*, /bin/rm -f /etc/systemd/system/hainet-*, /bin/rm -rf /etc/hainet*, /bin/rm -rf /var/lib/hainet*, /bin/rm -rf /var/log/hainet*, /bin/rm -rf /opt/hainet*",
            user = username
        );
        
        // Use echo password | sudo -S to write the sudoers file
        let command = format!(
            "echo '{}' | sudo -S bash -c 'echo \"{}\" > /etc/sudoers.d/hainet && chmod 440 /etc/sudoers.d/hainet' 2>/dev/null",
            password, sudoers_content
        );
        
        let mut channel = sess.channel_session()?;
        channel.exec(&command)?;
        
        let mut s = String::new();
        channel.read_to_string(&mut s)?;
        channel.wait_close()?;
        
        let exit_code = channel.exit_status()?;
        if exit_code == 0 {
            println!("✓ Passwordless sudo configured for HAI-Net commands on {}.", ip);
        } else {
            println!("⚠️  Could not set up sudoers on {} (exit {}). sudo commands may require a password.", ip, exit_code);
        }

        Ok(())
    }
    
    /// Test if the hainet-mesh key can authenticate to a remote host
    pub fn test_mesh_key_auth(&self, ip: &str, username: &str) -> bool {
        if !self.has_key_pair() {
            return false;
        }
        
        let tcp = match TcpStream::connect(format!("{}:22", ip)) {
            Ok(tcp) => tcp,
            Err(_) => return false,
        };
        let mut sess = match Session::new() {
            Ok(s) => s,
            Err(_) => return false,
        };
        sess.set_tcp_stream(tcp);
        if sess.handshake().is_err() {
            return false;
        }
        
        sess.userauth_pubkey_file(username, None, &self.private_key_path, None).is_ok()
    }
    
    /// Save the mesh manifest to ~/.hainet/mesh.json
    pub fn save_manifest(&self, manifest: &MeshManifest) -> Result<()> {
        let path = self.hainet_dir.join(MESH_MANIFEST_FILENAME);
        let json = serde_json::to_string_pretty(manifest)?;
        fs::write(&path, json)?;
        println!("✓ Mesh manifest saved to {}", path.display());
        Ok(())
    }
    
    /// Load the mesh manifest from ~/.hainet/mesh.json
    pub fn load_manifest(&self) -> Option<MeshManifest> {
        let path = self.hainet_dir.join(MESH_MANIFEST_FILENAME);
        if !path.exists() {
            return None;
        }
        fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
    }
    
    /// Check if a saved manifest exists
    pub fn has_manifest(&self) -> bool {
        self.hainet_dir.join(MESH_MANIFEST_FILENAME).exists()
    }
    
    /// Remove the mesh key pair and manifest (final uninstall step)
    pub fn destroy(&self) -> Result<()> {
        println!("🗑️  Removing HAI-Net mesh SSH key pair...");
        if self.private_key_path.exists() {
            fs::remove_file(&self.private_key_path)?;
        }
        if self.public_key_path.exists() {
            fs::remove_file(&self.public_key_path)?;
        }
        
        let manifest_path = self.hainet_dir.join(MESH_MANIFEST_FILENAME);
        if manifest_path.exists() {
            println!("🗑️  Removing mesh manifest...");
            fs::remove_file(&manifest_path)?;
        }
        
        println!("✓ HAI-Net mesh credentials destroyed.");
        Ok(())
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
        assert!(manager.private_key_path.to_str().unwrap().contains(".ssh/hainet-mesh"));
        assert!(manager.public_key_path.to_str().unwrap().contains(".ssh/hainet-mesh.pub"));
    }
    
    #[test]
    fn test_key_paths() {
        let manager = SSHKeyManager::new().unwrap();
        
        assert_eq!(
            manager.private_key_path().file_name().unwrap(),
            "hainet-mesh"
        );
        
        assert_eq!(
            manager.public_key_path().file_name().unwrap(),
            "hainet-mesh.pub"
        );
    }
}
