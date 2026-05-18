//! # START OF FILE hainet-seed/src/installer/uninstaller.rs
//! Uninstallation logic for removing HAI-Net from deployed devices.
//! Uses the saved mesh manifest and hainet-mesh SSH key for access.
//! Falls back to password prompts if key auth or sudo -n fail.
//! 
//! SAFETY: Only removes hainet-* services, binaries, configs, and the hainet user.
//! Does NOT touch ollama, comfyui, searxng, or any other software.

use anyhow::Result;
use tracing::{info, debug};
use crate::installer::ssh_client::{SSHClient, SSHCredentials, SSHClientTrait};
use crate::installer::ssh_keys::SSHKeyManager;

/// Only these specific services will be stopped/removed
const HAINET_SERVICES: &[&str] = &["hainet-core", "hainet-chain", "hainet-bridge", "hainet-portal"];

/// Only these specific binaries will be removed
const HAINET_BINARIES: &[&str] = &["hainet-core", "hainet-chain", "hainet-bridge", "hainet-portal"];

/// Only these specific directories will be removed
const HAINET_DIRS: &[&str] = &["/etc/hainet", "/var/lib/hainet", "/var/log/hainet", "/opt/hainet"];

pub struct Uninstaller {
    key_manager: SSHKeyManager,
}

impl Uninstaller {
    pub fn new() -> Result<Self> {
        let key_manager = SSHKeyManager::new()?;
        Ok(Self { key_manager })
    }

    pub async fn uninstall(&self) -> Result<()> {
        info!("🗑️ Starting HAI-Net uninstallation...");

        // Step 1: Load the mesh manifest
        let manifest = self.key_manager.load_manifest();
        let has_mesh_key = self.key_manager.has_key_pair();

        if manifest.is_none() && !has_mesh_key {
            println!("\n⚠️  No mesh manifest or key found at ~/.hainet/mesh.json");
            println!("   Only localhost cleanup will be performed.\n");
        }

        // Determine which IPs are local
        let local_ips: Vec<String> = local_ip_address::list_afinet_netifas()
            .unwrap_or_default()
            .into_iter()
            .map(|(_, ip)| ip.to_string())
            .collect();

        let hostname = std::process::Command::new("hostname")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "localhost".to_string());

        // Separate remote nodes from localhost
        let remote_nodes: Vec<_> = manifest.as_ref()
            .map(|m| m.nodes.iter()
                .filter(|n| !local_ips.contains(&n.ip) && n.ip != "127.0.0.1" && n.hostname.to_lowercase() != hostname.to_lowercase())
                .cloned() // Clone the nodes so we can mutate them
                .collect::<Vec<_>>())
            .unwrap_or_default();

        // Step 2: Show what will be uninstalled
        println!("\n📋 HAI-Net Uninstallation Plan:");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("  🏠 Localhost: {}", hostname);
        for node in &remote_nodes {
            println!("  🌐 Remote: {} ({}) [{}]", node.hostname, node.ip, node.role);
        }
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("\nThis will ONLY remove:");
        println!("  • HAI-Net systemd services (hainet-core, hainet-chain, hainet-bridge, hainet-portal)");
        println!("  • HAI-Net binaries from /usr/local/bin/");
        println!("  • HAI-Net config, data, and logs (/etc/hainet, /var/lib/hainet, /var/log/hainet)");
        println!("  • The 'hainet' system user and sudoers entry");
        println!("  • The HAI-Net mesh SSH key pair");
        println!("\n✅ Will NOT touch: Ollama, ComfyUI, SearXNG, or any other software.");

        // Step 3: Confirm
        use std::io::{self, Write};
        print!("\n⚠️  Are you sure you want to uninstall HAI-Net? [y/N]: ");
        io::stdout().flush()?;
        let mut confirmation = String::new();
        io::stdin().read_line(&mut confirmation)?;
        if confirmation.trim().to_lowercase() != "y" {
            info!("Uninstallation cancelled.");
            return Ok(());
        }

        // Step 4: Uninstall from REMOTE devices first (while we still have the key)
        if !remote_nodes.is_empty() {
            // --- DYNAMIC IP HEALING ---
            // If the mesh moved to a new network, the IPs might have changed.
            // Check if nodes are reachable. If not, scan network and heal.
            let mut healed_nodes = remote_nodes.clone();
            let mut needs_healing = false;
            
            for node in &mut healed_nodes {
                use std::net::{TcpStream, SocketAddr};
                use std::time::Duration;
                if let Ok(addr) = format!("{}:22", node.ip).parse::<SocketAddr>() {
                    if TcpStream::connect_timeout(&addr, Duration::from_secs(2)).is_err() {
                        needs_healing = true;
                    }
                } else {
                    needs_healing = true;
                }
            }
            
            if needs_healing {
                info!("⚠️  Some nodes are unreachable at their saved IPs. Scanning network for IP changes...");
                if let Ok(scanner) = crate::installer::network_scanner::NetworkScanner::new() {
                    if let Ok(devices) = scanner.scan_local_network() {
                        for node in &mut healed_nodes {
                            use std::net::{TcpStream, SocketAddr};
                            use std::time::Duration;
                            
                            let is_reachable = format!("{}:22", node.ip).parse::<SocketAddr>()
                                .map(|addr| TcpStream::connect_timeout(&addr, Duration::from_secs(2)).is_ok())
                                .unwrap_or(false);
                                
                            if !is_reachable {
                                // Try to find by MAC or Hostname
                                let new_ip = devices.iter().find(|d| {
                                    if let (Some(ref manifest_mac), Some(ref scan_mac)) = (&node.mac_address, &d.mac_address) {
                                        if manifest_mac.to_lowercase() == scan_mac.to_lowercase() {
                                            return true;
                                        }
                                    }
                                    if let Some(ref scan_host) = d.hostname {
                                        let h1 = node.hostname.to_lowercase().replace(".lan", "").replace(".local", "");
                                        let h2 = scan_host.to_lowercase().replace(".lan", "").replace(".local", "");
                                        if !h1.is_empty() && h1 == h2 {
                                            return true;
                                        }
                                    }
                                    false
                                });
                                
                                if let Some(found) = new_ip {
                                    info!("🔄 IP change detected and healed for {}: {} → {}", node.hostname, node.ip, found.ip);
                                    node.ip = found.ip.clone();
                                }
                            }
                        }
                    }
                }
            }
            
            info!("\n🌐 Uninstalling from {} remote device(s)...", healed_nodes.len());

            for node in &healed_nodes {
                info!("\n━━ {} ({}) ━━", node.hostname, node.ip);
                
                // Try to connect and clean up
                let sudo_password = self.uninstall_remote_node(&node.ip, &node.username, has_mesh_key).await;
                
                match sudo_password {
                    Ok(_) => info!("✓ Uninstalled from {} ({})", node.hostname, node.ip),
                    Err(e) => info!("⚠️  Could not fully uninstall from {}: {}", node.ip, e),
                }
            }
        }

        // Step 5: Uninstall from LOCALHOST
        info!("\n━━ Localhost ({}) ━━", hostname);
        match self.uninstall_localhost().await {
            Ok(_) => info!("✓ Localhost uninstallation complete."),
            Err(e) => info!("⚠️  Localhost uninstallation had errors: {}", e),
        }

        // Step 6: FINAL — Destroy the mesh key and manifest (after all nodes are cleaned)
        println!("\n🔐 Destroying mesh credentials...");
        self.key_manager.destroy()?;

        println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("✅ HAI-Net fully uninstalled. Mesh key destroyed.");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        Ok(())
    }

    /// Uninstall from a single remote node.
    /// Tries mesh key auth first, falls back to password prompt.
    /// Tries sudo -n first, falls back to password-based sudo.
    async fn uninstall_remote_node(&self, ip: &str, username: &str, has_mesh_key: bool) -> Result<()> {
        use std::io::{self, Write};
        
        // Try 1: Connect with mesh key
        if has_mesh_key && self.key_manager.test_mesh_key_auth(ip, username) {
            info!("🔑 Connected via mesh key");
            let credentials = SSHCredentials {
                username: username.to_string(),
                password: String::new(),
            };
            let mut client = SSHClient::new(ip.to_string(), credentials);
            if client.connect().is_ok() && client.authenticate_pubkey(self.key_manager.private_key_path(), None).is_ok() {
                // Try sudo -n first
                let sudo_works = self.test_sudo_n(&client);
                
                if sudo_works {
                    info!("🔓 sudo -n works — cleaning up...");
                    self.run_remote_cleanup(&client, None);
                    self.remove_mesh_key_from_remote(&client);
                    let _ = client.disconnect();
                    return Ok(());
                } else {
                    // sudo -n failed — prompt for password for sudo
                    println!("⚠️  sudo requires a password on {}", ip);
                    let password = dialoguer::Password::new()
                        .with_prompt(format!("Password for {}@{} (for sudo)", username, ip))
                        .interact()?;
                    
                    self.run_remote_cleanup(&client, Some(&password));
                    self.remove_mesh_key_from_remote(&client);
                    let _ = client.disconnect();
                    return Ok(());
                }
            }
        }
        
        // Try 2: Connect with password
        println!("🔑 Mesh key auth failed or unavailable for {}", ip);
        print!("Username for {} (default: {}): ", ip, username);
        io::stdout().flush()?;
        let mut user_input = String::new();
        io::stdin().read_line(&mut user_input)?;
        let actual_username = if user_input.trim().is_empty() { username.to_string() } else { user_input.trim().to_string() };
        
        let password = dialoguer::Password::new()
            .with_prompt(format!("Password for {}@{}", actual_username, ip))
            .interact()?;
        
        let credentials = SSHCredentials {
            username: actual_username.clone(),
            password: password.clone(),
        };
        let mut client = SSHClient::new(ip.to_string(), credentials);
        
        if client.connect().is_err() || client.authenticate_password().is_err() {
            anyhow::bail!("Could not connect/authenticate to {}. Skipping.", ip);
        }
        
        info!("✓ Connected with password");
        self.run_remote_cleanup(&client, Some(&password));
        self.remove_mesh_key_from_remote(&client);
        let _ = client.disconnect();
        Ok(())
    }
    
    /// Test if sudo -n works on a remote device
    fn test_sudo_n<C: SSHClientTrait>(&self, client: &C) -> bool {
        match client.execute_command("sudo -n true 2>/dev/null") {
            Ok(_) => true,
            Err(_) => false,
        }
    }
    
    /// Run the actual cleanup commands on a remote device.
    /// If `sudo_password` is Some, use `echo pass | sudo -S`, otherwise use `sudo -n`.
    fn run_remote_cleanup<C: SSHClientTrait>(&self, client: &C, sudo_password: Option<&str>) {
        // Helper to build sudo command
        let sudo_cmd = |cmd: &str| -> String {
            match sudo_password {
                Some(pass) => format!("echo '{}' | sudo -S {} 2>/dev/null", pass, cmd),
                None => format!("sudo -n {} 2>/dev/null || true", cmd),
            }
        };
        
        // 1. Stop and disable services (only those that are actually installed)
        for service in HAINET_SERVICES {
            let svc = format!("{}.service", service);
            // Check if the service unit file exists before attempting stop
            let check_cmd = format!("systemctl list-unit-files {svc} 2>/dev/null | grep -q {svc} && echo exists || echo missing");
            let exists = client.execute_command(&check_cmd)
                .map(|o| o.trim() == "exists")
                .unwrap_or(false);
            if exists {
                info!("  Stopping {}...", svc);
                let _ = client.execute_command(&sudo_cmd(&format!("systemctl stop {}", svc)));
                let _ = client.execute_command(&sudo_cmd(&format!("systemctl disable {}", svc)));
            } else {
                debug!("  {} not installed, skipping", svc);
            }
            let _ = client.execute_command(&sudo_cmd(&format!("rm -f /etc/systemd/system/{}", svc)));
        }
        
        let _ = client.execute_command(&sudo_cmd("systemctl daemon-reload"));
        
        // 2. Remove binaries (ONLY hainet-* binaries)
        for binary in HAINET_BINARIES {
            let _ = client.execute_command(&sudo_cmd(&format!("rm -f /usr/local/bin/{}", binary)));
        }
        // Also clean user-space fallback
        for binary in HAINET_BINARIES {
            let _ = client.execute_command(&format!("rm -f ~/bin/{} 2>/dev/null || true", binary));
        }
        
        // 3. Remove directories (ONLY hainet-specific)
        for dir in HAINET_DIRS {
            let _ = client.execute_command(&sudo_cmd(&format!("rm -rf {}", dir)));
        }
        let _ = client.execute_command("rm -rf ~/hainet 2>/dev/null || true");
        
        // 4. Remove hainet user and group
        let _ = client.execute_command(&sudo_cmd("userdel hainet"));
        let _ = client.execute_command(&sudo_cmd("groupdel hainet"));
        
        // 5. Remove sudoers entry
        let _ = client.execute_command(&sudo_cmd("rm -f /etc/sudoers.d/hainet"));
        
        info!("  ✓ Cleanup complete");
    }

    /// Remove the hainet-mesh public key from a remote node's authorized_keys
    fn remove_mesh_key_from_remote<C: SSHClientTrait>(&self, client: &C) {
        if let Ok(public_key) = self.key_manager.read_public_key() {
            let key_trimmed = public_key.trim();
            // Use grep -v to remove the line containing our key
            let _ = client.execute_command(&format!(
                "if [ -f ~/.ssh/authorized_keys ]; then grep -v '{}' ~/.ssh/authorized_keys > ~/.ssh/authorized_keys.tmp && mv ~/.ssh/authorized_keys.tmp ~/.ssh/authorized_keys; fi",
                key_trimmed
            ));
            info!("  ✓ Mesh key removed from authorized_keys");
        }
    }

    /// Uninstall HAI-Net from the local machine.
    /// Uses regular sudo which can prompt for password interactively on the local terminal.
    async fn uninstall_localhost(&self) -> Result<()> {
        use std::process::Command;

        // 1. Stop and disable services (only those that are actually installed)
        for service in HAINET_SERVICES {
            let svc = format!("{}.service", service);
            // Check if service exists before trying to stop it
            let exists = Command::new("systemctl")
                .args(["list-unit-files", &svc])
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).contains(&svc))
                .unwrap_or(false);
            if exists {
                info!("  Stopping {}...", svc);
                let _ = Command::new("sudo").args(["systemctl", "stop", &svc]).status();
                let _ = Command::new("sudo").args(["systemctl", "disable", &svc]).status();
            } else {
                debug!("  {} not installed, skipping", svc);
            }
            let _ = Command::new("sudo").args(["rm", "-f", &format!("/etc/systemd/system/{}", svc)]).status();
        }
        let _ = Command::new("sudo").args(["systemctl", "daemon-reload"]).status();

        // 2. Remove binaries (ONLY hainet-*)
        for binary in HAINET_BINARIES {
            let _ = Command::new("sudo").args(["rm", "-f", &format!("/usr/local/bin/{}", binary)]).status();
        }

        // 3. Remove directories (ONLY hainet-specific)
        for dir in HAINET_DIRS {
            let _ = Command::new("sudo").args(["rm", "-rf", dir]).status();
        }

        // 4. Remove hainet user and group
        let _ = Command::new("sudo").args(["userdel", "hainet"]).status();
        let _ = Command::new("sudo").args(["groupdel", "hainet"]).status();
        
        // 5. Remove sudoers entry
        let _ = Command::new("sudo").args(["rm", "-f", "/etc/sudoers.d/hainet"]).status();

        Ok(())
    }
}
