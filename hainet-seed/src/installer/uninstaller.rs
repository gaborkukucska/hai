//! # START OF FILE hainet-seed/src/installer/uninstaller.rs
//! Uninstallation logic for removing HAI-Net from deployed devices.

use anyhow::Result;
use tracing::info;
use crate::installer::network_scanner::NetworkScanner;
use crate::installer::ssh_client::{SSHClient, SSHCredentials, SSHClientTrait};
use crate::installer::nmap_installer::ensure_nmap_installed;
use crate::installer::platform::Platform;
use std::path::Path;

const HAINET_SERVICES: &[&str] = &["hainet-core", "hainet-chain", "hainet-bridge", "hainet-portal"];
const HAINET_DIRS: &[&str] = &["/opt/hainet", "/etc/hainet", "/var/lib/hainet", "/var/log/hainet"];

pub struct Uninstaller {
    platform: Platform,
}

impl Uninstaller {
    pub fn new() -> Result<Self> {
        let platform = Platform::detect()?;
        Ok(Self { platform })
    }

    pub async fn uninstall(&self) -> Result<()> {
        info!("🗑️ Starting HAI-Net uninstallation...");

        // 1. Discover devices on the network
        ensure_nmap_installed(&self.platform).await?;
        let scanner = NetworkScanner::new()?;
        let devices = scanner.scan_local_network()?;

        // Build full list: localhost + discovered remote devices
        let local_ips: Vec<String> = local_ip_address::list_afinet_netifas()
            .unwrap_or_default()
            .into_iter()
            .map(|(_, ip)| ip.to_string())
            .collect();

        let (local_devices, remote_devices): (Vec<_>, Vec<_>) = devices.into_iter().partition(|d| {
            d.ip == "127.0.0.1" || d.ip == "localhost" || local_ips.contains(&d.ip)
        });

        let has_local = !local_devices.is_empty();

        if !has_local && remote_devices.is_empty() {
            info!("No devices found on the network. Nothing to uninstall.");
            return Ok(());
        }

        // 2. Show what will be uninstalled
        println!("\n📋 HAI-Net Uninstallation Plan:");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        if has_local {
            let hostname = std::process::Command::new("hostname")
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|| "localhost".to_string());
            println!("  🏠 Localhost: {}", hostname);
        }
        for device in &remote_devices {
            let hostname_display = device.hostname.as_deref().unwrap_or("unknown");
            println!("  🌐 Remote: {} ({})", device.ip, hostname_display);
        }
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("\nThis will:");
        println!("  • Stop and remove all HAI-Net systemd services");
        println!("  • Remove binaries from /usr/local/bin");
        println!("  • Remove config from /etc/hainet");
        println!("  • Remove data from /var/lib/hainet and /var/log/hainet");
        println!("  • Remove the hainet system user and group");

        // 3. Confirm
        use std::io::{self, Write};
        print!("\n⚠️  Are you sure you want to uninstall HAI-Net from ALL listed devices? [y/N]: ");
        io::stdout().flush()?;
        let mut confirmation = String::new();
        io::stdin().read_line(&mut confirmation)?;
        if confirmation.trim().to_lowercase() != "y" {
            info!("Uninstallation cancelled by user.");
            return Ok(());
        }

        // 4. Uninstall from localhost first
        if has_local {
            info!("\n🏠 Uninstalling from localhost...");
            match self.uninstall_localhost().await {
                Ok(_) => info!("✓ Localhost uninstallation complete."),
                Err(e) => info!("⚠️  Localhost uninstallation had errors: {}", e),
            }
        }

        // 5. Uninstall from remote devices
        if !remote_devices.is_empty() {
            info!("\n🌐 Uninstalling from {} remote devices...", remote_devices.len());

            for device in &remote_devices {
                let hostname_display = device.hostname.as_deref().unwrap_or("unknown");
                info!("\n🔍 Uninstalling from {} ({})...", device.ip, hostname_display);

                // Try SSH key auth first (keys were distributed during install)
                let key_path = dirs::home_dir()
                    .unwrap_or_else(|| Path::new("/root").to_path_buf())
                    .join(".ssh/id_ed25519");

                let username = std::env::var("USER").unwrap_or_else(|_| "root".to_string());

                let credentials = SSHCredentials {
                    username: username.clone(),
                    password: String::new(),
                };

                let mut client = SSHClient::new(device.ip.clone(), credentials);

                match client.connect() {
                    Ok(_) => {
                        // Try key auth
                        let auth_ok = if key_path.exists() {
                            client.authenticate_pubkey(&key_path, None).is_ok()
                        } else {
                            false
                        };

                        if !auth_ok {
                            // Fall back to password
                            let username_input: String = dialoguer::Input::new()
                                .with_prompt(format!("Username for {}", device.ip))
                                .default(username.clone())
                                .interact_text()?;

                            let password = dialoguer::Password::new()
                                .with_prompt(format!("Password for {}@{}", username_input, device.ip))
                                .interact()?;

                            let credentials = SSHCredentials {
                                username: username_input,
                                password,
                            };
                            let mut client = SSHClient::new(device.ip.clone(), credentials);
                            client.connect()?;
                            client.authenticate_password()?;

                            match self.uninstall_remote(&mut client).await {
                                Ok(_) => info!("✓ Uninstalled from {} ({})", device.ip, hostname_display),
                                Err(e) => info!("⚠️  Failed to uninstall from {}: {}", device.ip, e),
                            }
                            let _ = client.disconnect();
                            continue;
                        }

                        match self.uninstall_remote(&mut client).await {
                            Ok(_) => info!("✓ Uninstalled from {} ({})", device.ip, hostname_display),
                            Err(e) => info!("⚠️  Failed to uninstall from {}: {}", device.ip, e),
                        }
                        let _ = client.disconnect();
                    }
                    Err(e) => {
                        info!("⚠️  Could not connect to {}: {}. Skipping.", device.ip, e);
                    }
                }
            }
        }

        println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        info!("✅ HAI-Net uninstallation process complete.");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        Ok(())
    }

    /// Uninstall HAI-Net from the local machine
    async fn uninstall_localhost(&self) -> Result<()> {
        use std::process::Command;

        // Stop and disable services
        for service in HAINET_SERVICES {
            let service_name = format!("{}.service", service);
            info!("  Stopping {}...", service_name);
            let _ = Command::new("sudo").args(&["systemctl", "stop", &service_name]).status();
            let _ = Command::new("sudo").args(&["systemctl", "disable", &service_name]).status();
            let _ = Command::new("sudo").args(&["rm", "-f", &format!("/etc/systemd/system/{}", service_name)]).status();
        }

        // Reload systemd
        let _ = Command::new("sudo").args(&["systemctl", "daemon-reload"]).status();

        // Remove binaries
        for service in HAINET_SERVICES {
            let _ = Command::new("sudo").args(&["rm", "-f", &format!("/usr/local/bin/{}", service)]).status();
        }

        // Remove directories
        for dir in HAINET_DIRS {
            let _ = Command::new("sudo").args(&["rm", "-rf", dir]).status();
        }

        // Remove system user/group (ignore errors if they don't exist)
        let _ = Command::new("sudo").args(&["userdel", "hainet"]).status();
        let _ = Command::new("sudo").args(&["groupdel", "hainet"]).status();

        Ok(())
    }

    /// Uninstall HAI-Net from a remote device via SSH
    async fn uninstall_remote<C: SSHClientTrait>(&self, client: &mut C) -> Result<()> {
        // Stop and disable services
        for service in HAINET_SERVICES {
            let service_name = format!("{}.service", service);
            info!("  Stopping {}...", service_name);
            let _ = client.execute_command(&format!("sudo systemctl stop {}", service_name));
            let _ = client.execute_command(&format!("sudo systemctl disable {}", service_name));
            let _ = client.execute_command(&format!("sudo rm -f /etc/systemd/system/{}", service_name));
        }

        // Reload systemd
        let _ = client.execute_command("sudo systemctl daemon-reload");

        // Remove binaries
        for service in HAINET_SERVICES {
            let _ = client.execute_command(&format!("sudo rm -f /usr/local/bin/{}", service));
        }

        // Remove directories
        for dir in HAINET_DIRS {
            let _ = client.execute_command(&format!("sudo rm -rf {}", dir));
        }

        // Remove system user/group
        let _ = client.execute_command("sudo userdel hainet");
        let _ = client.execute_command("sudo groupdel hainet");

        // Clean up SSH authorized_keys (remove HAI-Net keys)
        let pub_key_path = dirs::home_dir()
            .unwrap_or_else(|| Path::new("/root").to_path_buf())
            .join(".ssh/id_ed25519.pub");

        if let Ok(public_key) = std::fs::read_to_string(&pub_key_path) {
            let key_trimmed = public_key.trim();
            // Safely remove the key from authorized_keys without breaking the file
            let _ = client.execute_command(&format!(
                "if [ -f ~/.ssh/authorized_keys ]; then grep -v '{}' ~/.ssh/authorized_keys > ~/.ssh/authorized_keys.tmp && mv ~/.ssh/authorized_keys.tmp ~/.ssh/authorized_keys; fi",
                key_trimmed
            ));
        }

        Ok(())
    }
}
