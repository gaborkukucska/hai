//! # START OF FILE hainet-seed/src/installer/uninstaller.rs
//! Uninstallation logic for removing HAI-Net from deployed devices.

use anyhow::{Result, bail};
use tracing::info;
use crate::installer::network_scanner::NetworkScanner;
use crate::installer::ssh_client::{SSHClient, SSHCredentials, SSHClientTrait};
use crate::installer::nmap_installer::ensure_nmap_installed;
use crate::installer::platform::Platform;
use std::path::Path;

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

        if devices.is_empty() {
            info!("No devices found on the network. Nothing to do.");
            return Ok(());
        }

        info!("Discovered {} devices. Will attempt to uninstall from each.", devices.len());

        // 2. Get credentials and confirm uninstallation
        use std::io::{self, Write};
        print!("Enter the SSH username for these devices: ");
        io::stdout().flush()?;
        let mut username = String::new();
        io::stdin().read_line(&mut username)?;
        let username = username.trim().to_string();

        print!("Are you sure you want to remove HAI-Net from all discovered devices? [y/N]: ");
        io::stdout().flush()?;
        let mut confirmation = String::new();
        io::stdin().read_line(&mut confirmation)?;
        if confirmation.trim().to_lowercase() != "y" {
            bail!("Uninstallation cancelled by user.");
        }

        // 3. Iterate over devices and uninstall
        for device in devices {
            info!("Uninstalling from {}...", device.ip);

            let credentials = SSHCredentials {
                username: username.clone(),
                password: String::new(), // Using key auth
            };

            let mut client = SSHClient::new(device.ip.clone(), credentials);

            if let Err(e) = self.uninstall_from_device(&mut client).await {
                info!("Failed to uninstall from {}: {}", device.ip, e);
            }
        }

        info!("Uninstallation process complete.");
        Ok(())
    }

    pub async fn uninstall_from_device<C: SSHClientTrait>(&self, client: &mut C) -> Result<()> {
        client.connect()?;

        let key_path = dirs::home_dir()
            .unwrap_or_else(|| Path::new("/root").to_path_buf())
            .join(".ssh/id_ed25519");

        client.authenticate_pubkey(&key_path, None)?;

        self.run_uninstall_commands(client).await?;

        client.disconnect()?;
        info!("Uninstallation from device complete.");
        Ok(())
    }

    async fn run_uninstall_commands<C: SSHClientTrait>(&self, client: &mut C) -> Result<()> {
        let services = ["hainet-core", "hainet-chain", "hainet-bridge", "hainet-portal"];
        for service in &services {
            info!("Stopping and disabling {}...", service);
            let _ = client.execute_command(&format!("sudo systemctl stop {}.service", service));
            let _ = client.execute_command(&format!("sudo systemctl disable {}.service", service));
            let _ = client.execute_command(&format!("sudo rm /etc/systemd/system/{}.service", service));
        }

        info!("Reloading systemd...");
        let _ = client.execute_command("sudo systemctl daemon-reload");

        info!("Removing directories...");
        let _ = client.execute_command("sudo rm -rf /opt/hainet");
        let _ = client.execute_command("sudo rm -rf /etc/hainet");

        info!("Removing user and group...");
        let _ = client.execute_command("sudo userdel hainet");
        let _ = client.execute_command("sudo groupdel hainet");

        info!("Cleaning up authorized_keys...");
        let pub_key_path = dirs::home_dir()
            .unwrap_or_else(|| Path::new("/root").to_path_buf())
            .join(".ssh/id_ed25519.pub");
        let public_key = std::fs::read_to_string(pub_key_path)?;
        let command = format!("grep -v '{}' ~/.ssh/authorized_keys > ~/.ssh/authorized_keys.tmp && mv ~/.ssh/authorized_keys.tmp ~/.ssh/authorized_keys", public_key.trim());
        let _ = client.execute_command(&command);

        Ok(())
    }
}
