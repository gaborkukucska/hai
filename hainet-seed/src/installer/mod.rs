// START OF FILE hainet-seed/src/installer/mod.rs
//! HAI-Net Seed Installer Module
//! 
//! Handles platform detection, dependency installation, and Ollama setup.

pub mod platform;
pub mod ollama;
pub mod whisper;
pub mod piper;
pub mod comfyui;
pub mod ffmpeg;
pub mod dependencies;
pub mod network_scanner;
pub mod nmap_installer;
pub mod ssh_client;
pub mod ssh_keys;
pub mod deployment;
pub mod uninstaller;
use anyhow::{Result, Context};
use tracing::{info, warn};
use std::io::{self, Write};

use crate::installer::platform::{Platform, SystemTier};
use crate::installer::ollama::OllamaInstaller;
use crate::installer::whisper::WhisperInstaller;
use crate::installer::piper::PiperInstaller;
use crate::installer::comfyui::ComfyUIInstaller;
use crate::installer::ffmpeg::FFmpegInstaller;
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
    comfyui: ComfyUIInstaller,
    ffmpeg: FFmpegInstaller,
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
        let comfyui = ComfyUIInstaller::new(platform.clone());
        let ffmpeg = FFmpegInstaller::new(platform.clone());
        
        Ok(Self {
            platform,
            tier,
            ollama,
            whisper,
            piper,
            comfyui,
            ffmpeg,
        })
    }
    
    /// Run complete intelligent installation workflow
    pub async fn install(&mut self) -> Result<()> {
        info!("🚀 Starting HAI-Net Mesh-First installation workflow...");
        
        // Step 1: Ask if user already has a networked shared folder
        let has_existing = self.prompt_has_existing_shared_drive()?;
        
        let existing_path = if has_existing {
            Some(self.prompt_shared_drive_path()?)
        } else {
            None
        };
        
        // Step 2: Discover devices and (if needed) set up shared drive during the process
        let _devices = self.discover_mesh_devices(existing_path.as_deref()).await?;
        
        // Step 3: The actual deployment is now handled inside setup_and_deploy_mesh
        // which is called by discover_mesh_devices if the user approves the plan.
        
        // Step 4: Handle local dependencies intelligently based on discovery
        // To do this, we re-assess localhost to check the services list
        // (Since discover_mesh_devices consumes the capabilities locally inside its flow)
        let localhost_caps = self.assess_localhost_capabilities_with_services().await.unwrap_or_else(|_| {
            crate::installer::ssh_client::DeviceCapabilities {
                ip: "127.0.0.1".to_string(),
                hostname: "localhost".to_string(),
                cpu_cores: 1,
                ram_gb: 1.0,
                gpu: None,
                disk_gb: 10.0,
                os: "Linux".to_string(),
                arch: "x86_64".to_string(),
                services: vec![],
                score: 0.0,
            }
        });

        let has_ollama = localhost_caps.services.iter().any(|s| s.name == "ollama");
        
        if has_ollama {
            info!("✅ Intelligent skip: Ollama is already running locally. Skipping installation.");
        } else {
            self.install_ollama().await?;
        }
        
        self.download_default_model().await?;
        self.install_whisper().await?;
        self.download_whisper_model().await?;
        self.install_piper().await?;
        self.download_piper_model().await?;

        // Install ComfyUI if we have a GPU (Tier 3/4)
        if self.tier == SystemTier::Tier3 || self.tier == SystemTier::Tier4 {
            self.install_comfyui().await?;
        } else {
            info!("⏭️  Skipping ComfyUI installation (requires a more capable GPU / Tier 3+ device).");
        }
        
        // Install FFmpeg (always useful for media tasks)
        self.install_ffmpeg().await?;
        
        info!("✅ Installation workflow complete!");
        Ok(())
    }

    /// Ask if the user already has a networked and mounted shared folder
    fn prompt_has_existing_shared_drive(&self) -> Result<bool> {
        use std::io::{self, Write};
        
        println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("📁 Shared Drive Setup");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        print!("Do you already have a networked and mounted read/write shared folder? (y/N): ");
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        let response = input.trim().to_lowercase();
        Ok(response == "y" || response == "yes")
    }
    
    /// Ask user for the path to their existing shared drive
    fn prompt_shared_drive_path(&self) -> Result<String> {
        use std::io::{self, Write};
        
        print!("Enter the path to your shared folder (default: /media/hai-drive): ");
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        let mut path = input.trim().to_string();
        if path.is_empty() {
            path = "/media/hai-drive".to_string();
        }
        
        info!("✅ Using existing shared folder at {}", path);
        Ok(path)
    }
    
    /// Create the standard HAI-Net subdirectory tree inside the shared drive
    fn initialize_shared_drive_structure(&self, path: &str) -> Result<()> {
        let base_path = std::path::Path::new(path);
        
        let directories = vec![
            "comfyui/custom_nodes",
            "comfyui/models/checkpoints",
            "comfyui/models/clip",
            "comfyui/models/configs",
            "comfyui/models/controlnet",
            "comfyui/models/diffusion_models",
            "comfyui/models/embeddings",
            "comfyui/models/loras",
            "comfyui/models/upscale_models",
            "comfyui/models/vae",
            "comfyui/workflows",
            "logs",
            "media_cache",
            "ollama",
            "projects",
            "vllm/models",
            "whisper",
            "Zimms",
        ];
        
        for dir in directories {
            let full_path = base_path.join(dir);
            if !full_path.exists() {
                if let Err(e) = std::fs::create_dir_all(&full_path) {
                    tracing::warn!("⚠️  Failed to create directory {}: {}", full_path.display(), e);
                } else {
                    tracing::debug!("Created directory: {}", full_path.display());
                }
            }
        }
        
        Ok(())
    }
    
    
    /// Display device choices and let user pick where to host the shared drive.
    /// Returns (remote_export_path, host_ip, local_mount_path).
    fn prompt_shared_drive_device_choice(&self, devices: &[(String, String, f64, bool)]) -> Result<(String, String, String)> {
        use std::io::{self, Write};
        
        println!("\n📊 Available devices to host the shared drive:");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        
        for (i, (hostname, ip, disk_gb, is_local)) in devices.iter().enumerate() {
            let local_tag = if *is_local { " ← this machine" } else { "" };
            let disk_display = if *disk_gb >= 0.0 {
                format!("{:.1} GB free", disk_gb)
            } else {
                "disk checked after login".to_string()
            };
            println!("  [{}] {} ({}) — {}{}", i + 1, hostname, ip, disk_display, local_tag);
        }
        
        println!();
        print!("Which device should host the shared drive? [1]: ");
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let choice_str = input.trim();
        
        let choice: usize = if choice_str.is_empty() {
            0
        } else {
            choice_str.parse::<usize>().unwrap_or(1).saturating_sub(1)
        };
        
        let (hostname, ip, _, is_local) = devices.get(choice).unwrap_or(&devices[0]);
        
        print!("Enter folder path on {} (default: /media/hai-drive): ", hostname);
        io::stdout().flush()?;
        
        let mut path_input = String::new();
        io::stdin().read_line(&mut path_input)?;
        let mut remote_path = path_input.trim().to_string();
        if remote_path.is_empty() {
            remote_path = "/media/hai-drive".to_string();
        }
        
        // For remote devices, ask where to mount locally (the remote path may not
        // make sense on this machine, e.g. /media/fast/NoSlop is BigBOY-specific).
        let local_mount = if !is_local && remote_path != "/media/hai-drive" {
            print!("Local mount point on this machine (default: /media/hai-drive): ");
            io::stdout().flush()?;
            
            let mut mount_input = String::new();
            io::stdin().read_line(&mut mount_input)?;
            let mount = mount_input.trim().to_string();
            if mount.is_empty() {
                "/media/hai-drive".to_string()
            } else {
                mount
            }
        } else {
            // Local device or default path — use the same path
            remote_path.clone()
        };
        
        Ok((remote_path, ip.clone(), local_mount))
    }
    
    /// Set up the shared drive on the local machine
    fn setup_shared_drive_on_local(&self, path: &str) -> Result<String> {
        info!("📁 Setting up shared drive locally at {}...", path);
        
        let base_path = std::path::Path::new(path);
        
        // Create with sudo if needed
        if !base_path.exists() {
            if let Err(e) = std::fs::create_dir_all(base_path) {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    info!("Requires elevated permissions to create {}. Using sudo...", path);
                    let user = std::env::var("USER").unwrap_or_else(|_| "root".to_string());
                    
                    let _ = std::process::Command::new("sudo")
                        .arg("mkdir").arg("-p").arg(path)
                        .status();
                        
                    let _ = std::process::Command::new("sudo")
                        .arg("chown").arg("-R")
                        .arg(format!("{}:{}", user, user))
                        .arg(path)
                        .status();
                } else {
                    tracing::warn!("⚠️  Failed to create base directory {}: {}", path, e);
                }
            }
        }
        
        self.initialize_shared_drive_structure(path)?;
        
        // Set up NFS server locally
        if let Err(e) = self.setup_nfs_server(path) {
            tracing::warn!("⚠️  Failed to configure NFS server: {}", e);
        }
        
        info!("✅ Shared drive initialized at {}", path);
        Ok(path.to_string())
    }
    
    /// Set up the shared drive on a remote device via SSH.
    /// Uses existing credentials from device assessment — no extra login prompt.
    /// `path` is the export path on the remote device (e.g., /media/fast/NoSlop).
    /// `local_mount_path` is where non-hosting nodes will mount the share (e.g., /media/hai-drive).
    fn setup_shared_drive_on_remote(&self, remote_ip: &str, path: &str, local_mount_path: &str, existing_creds: Option<&(String, String)>) -> Result<String> {
        use crate::installer::ssh_client::{SSHClient, SSHCredentials, SSHClientTrait};
        
        // Get username from existing credentials or fall back to current user
        let username = match existing_creds {
            Some((user, _)) => user.clone(),
            None => std::env::var("USER").unwrap_or_else(|_| "root".to_string()),
        };
        
        let mut password = match existing_creds {
            Some((_, pwd)) => pwd.clone(),
            None => String::new(),
        };
        
        info!("📁 Setting up shared drive on {}:{} as {}...", remote_ip, path, username);
        
        let creds = SSHCredentials { username: username.clone(), password: String::new() };
        let mut client = SSHClient::new(remote_ip.to_string(), creds);
        client.connect()?;
        
        // Prefer SSH key auth (already distributed during assessment)
        let key_path = dirs::home_dir()
            .unwrap_or_else(|| std::path::Path::new("/root").to_path_buf())
            .join(".ssh/hainet-mesh");
        
        if key_path.exists() {
            client.authenticate_pubkey(&key_path, None)?;
        } else if let Some((_, pwd)) = existing_creds {
            // Fall back to password auth if key not available
            let creds = SSHCredentials { username: username.clone(), password: pwd.clone() };
            let mut client_pw = SSHClient::new(remote_ip.to_string(), creds);
            client_pw.connect()?;
            client_pw.authenticate_password()?;
            client = client_pw;
        } else {
            anyhow::bail!("No credentials available for {}. Cannot set up shared drive.", remote_ip);
        }
        
        // Test if sudo requires password or has outdated permissions
        if password.is_empty() {
            if client.execute_command("sudo -n apt-get update -qq 2>/dev/null").is_err() {
                warn!("⚠️  The remote node's sudo permissions are outdated and require an update.");
                if let Ok(pwd) = dialoguer::Password::new()
                    .with_prompt(format!("Enter password for {}@{} to proceed", username, remote_ip))
                    .interact()
                {
                    password = pwd;
                    
                    // Immediately heal the remote sudoers file so future operations and Step 5 succeed
                    let sudoers_content = format!(
                        "{user} ALL=(ALL) NOPASSWD: /usr/bin/systemctl * hainet-*, /usr/bin/systemctl * nfs-*, /usr/bin/systemctl daemon-reload, /usr/bin/apt-get install *, /usr/bin/apt-get update, /usr/sbin/exportfs *, /usr/bin/tee *, /usr/bin/mv /tmp/hainet-upload-* /usr/local/bin/*, /usr/bin/mv /tmp/hainet* /etc/*, /usr/bin/mkdir -p /usr/local/bin*, /usr/bin/mkdir -p /etc/hainet*, /usr/bin/mkdir -p /var/lib/hainet*, /usr/bin/mkdir -p /var/log/hainet*, /usr/bin/mkdir -p /media/*, /usr/bin/chown * hainet*, /usr/bin/chown -R *, /usr/bin/chmod *, /usr/sbin/useradd *, /usr/sbin/userdel *, /usr/sbin/groupdel *, /usr/bin/rm -f /usr/local/bin/hainet-*, /usr/bin/rm -f /etc/systemd/system/hainet-*, /usr/bin/rm -rf /etc/hainet*, /usr/bin/rm -rf /var/lib/hainet*, /usr/bin/rm -rf /var/log/hainet*, /usr/bin/rm -rf /opt/hainet*, /bin/mv /tmp/hainet-upload-* /usr/local/bin/*, /bin/mv /tmp/hainet* /etc/*, /bin/mkdir -p *, /bin/chown * hainet*, /bin/chown -R *, /bin/chmod *, /bin/rm -f /usr/local/bin/hainet-*, /bin/rm -f /etc/systemd/system/hainet-*, /bin/rm -rf /etc/hainet*, /bin/rm -rf /var/lib/hainet*, /bin/rm -rf /var/log/hainet*, /bin/rm -rf /opt/hainet*, /usr/bin/mount *, /bin/mount *",
                        user = username
                    );
                    
                    let update_sudoers_cmd = format!(
                        "echo '{}' | sudo -S bash -c 'echo \"{}\" > /etc/sudoers.d/hainet && chmod 440 /etc/sudoers.d/hainet'",
                        password, sudoers_content
                    );
                    let _ = client.execute_command(&update_sudoers_cmd);
                }
            }
        }
        
        // Helper to run a command as root. 
        // If password is empty, relies on NOPASSWD whitelist (must be exact command).
        // If password provided, uses sudo -S with bash -c for maximum flexibility.
        let run_sudo = |client: &SSHClient, cmd: &str| -> Result<String, anyhow::Error> {
            let wrapped_cmd = if password.is_empty() {
                format!("sudo -n {}", cmd)
            } else {
                format!("echo '{}' | sudo -S bash -c \"{}\"", password, cmd.replace("\"", "\\\""))
            };
            client.execute_command(&wrapped_cmd)
        };
        
        // Check disk space on remote
        if let Ok(disk_output) = client.execute_command("df -BG / 2>/dev/null | awk 'NR==2 {gsub(\"G\",\"\",$4); print $4}'") {
            if let Ok(disk_gb) = disk_output.parse::<f64>() {
                info!("📊 Remote device has {:.1} GB free disk space", disk_gb);
            }
        }
        
        // Create shared drive directory on remote
        info!("📁 Creating shared drive on {}:{}...", remote_ip, path);
        let _ = run_sudo(&client, &format!("mkdir -p {}", path));
        let _ = run_sudo(&client, &format!("chown -R {}:{} {}", username, username, path));
        
        // Create subdirectory structure on remote
        let directories = vec![
            "comfyui/custom_nodes", "comfyui/models/checkpoints", "comfyui/models/clip",
            "comfyui/models/configs", "comfyui/models/controlnet", "comfyui/models/diffusion_models",
            "comfyui/models/embeddings", "comfyui/models/loras", "comfyui/models/upscale_models",
            "comfyui/models/vae", "comfyui/workflows",
            "logs", "media_cache", "ollama", "projects", "vllm/models", "whisper", "Zimms",
        ];
        
        for dir in &directories {
            let _ = client.execute_command(&format!("mkdir -p {}/{}", path, dir)); // Inner dirs owned by user, no sudo needed
        }
        
        // Install NFS server on remote
        info!("🌐 Installing NFS server on {}...", remote_ip);
        let _ = run_sudo(&client, "apt-get update -qq");
        
        // When password is empty, we can't use ENV vars in sudo because SETENV is not in the whitelist.
        let apt_cmd = if password.is_empty() {
            "apt-get install -y nfs-kernel-server"
        } else {
            "DEBIAN_FRONTEND=noninteractive apt-get install -y nfs-kernel-server"
        };
        
        if let Err(e) = run_sudo(&client, apt_cmd) {
            warn!("⚠️  Failed to install nfs-kernel-server on {}: {}", remote_ip, e);
            warn!("   The sudoers entry may need updating. Re-run the installer to refresh permissions.");
        }
        
        // Configure /etc/exports
        let export_entry = format!("{} *(rw,sync,no_subtree_check,no_root_squash)", path);
        let check_cmd = format!("grep -q '{}' /etc/exports 2>/dev/null && echo exists || echo missing", path);
        let check_result = client.execute_command(&check_cmd).unwrap_or_else(|_| "missing".to_string());
        
        if check_result.contains("missing") {
            let export_cmd = if password.is_empty() {
                format!("echo '{}' | sudo -n tee -a /etc/exports", export_entry)
            } else {
                format!("echo '{}' | sudo -S bash -c \"echo '{}' >> /etc/exports\"", password, export_entry)
            };
            let _ = client.execute_command(&export_cmd);
            
            let _ = run_sudo(&client, "exportfs -ra");
            let _ = run_sudo(&client, "systemctl enable nfs-kernel-server");
            let _ = run_sudo(&client, "systemctl restart nfs-kernel-server");
        }
        
        // Verify NFS server is actually running on remote before we try to mount
        std::thread::sleep(std::time::Duration::from_secs(2)); // Give NFS time to start
        let nfs_status = run_sudo(&client, "systemctl is-active nfs-kernel-server")
            .unwrap_or_else(|_| "unknown".to_string());
        
        if nfs_status.trim() == "active" {
            info!("✅ NFS server is running on {}", remote_ip);
        } else {
            warn!("⚠️  NFS server may not be running on {} (status: {}). Trying to start...", remote_ip, nfs_status.trim());
            let _ = run_sudo(&client, "systemctl start nfs-kernel-server");
            std::thread::sleep(std::time::Duration::from_secs(2));
            
            let retry_status = run_sudo(&client, "systemctl is-active nfs-kernel-server")
                .unwrap_or_else(|_| "unknown".to_string());
            if retry_status.trim() != "active" {
                warn!("⚠️  NFS server failed to start on {}. Mount will likely fail.", remote_ip);
                warn!("   Check: sudo systemctl status nfs-kernel-server on {}", remote_ip);
            }
        }
        
        // Verify the export is visible
        let showmount_check = client.execute_command(&format!("showmount -e localhost 2>/dev/null | grep -q '{}' && echo visible || echo hidden", path))
            .unwrap_or_else(|_| "hidden".to_string());
        if showmount_check.trim() != "visible" {
            warn!("⚠️  NFS export for {} is not visible yet on {}. Re-exporting...", path, remote_ip);
            let _ = run_sudo(&client, "exportfs -ra");
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
        
        client.disconnect()?;
        
        // Mount the remote NFS share on localhost at the local mount path
        info!("🌐 Mounting {}:{} → {} on localhost...", remote_ip, path, local_mount_path);
        let _ = std::process::Command::new("sudo")
            .args(&["apt-get", "install", "-y", "nfs-common"])
            .status();
        let _ = std::process::Command::new("sudo")
            .args(&["mkdir", "-p", local_mount_path])
            .status();
            
        // Add to fstab if not already there (source is remote, destination is local)
        let fstab_entry = format!("{}:{} {} nfs defaults 0 0", remote_ip, path, local_mount_path);
        let fstab_check = std::process::Command::new("bash")
            .arg("-c")
            .arg(&format!("grep -q '{}:{}' /etc/fstab", remote_ip, path))
            .status();
        if fstab_check.map(|s| !s.success()).unwrap_or(true) {
            let _ = std::process::Command::new("bash")
                .arg("-c")
                .arg(&format!("echo '{}' | sudo tee -a /etc/fstab", fstab_entry))
                .status();
        }
        
        // Try mounting with retries
        let mut mount_ok = false;
        for attempt in 1..=3 {
            let mount_status = std::process::Command::new("sudo")
                .args(&["mount", "-t", "nfs", &format!("{}:{}", remote_ip, path), local_mount_path])
                .status();
            
            match mount_status {
                Ok(status) if status.success() => {
                    mount_ok = true;
                    break;
                }
                _ => {
                    if attempt < 3 {
                        warn!("⚠️  Mount attempt {}/3 failed. Retrying in 3 seconds...", attempt);
                        std::thread::sleep(std::time::Duration::from_secs(3));
                    }
                }
            }
        }
        
        if mount_ok {
            info!("✅ Shared drive ready at {} (hosted on {}:{})", local_mount_path, remote_ip, path);
        } else {
            warn!("⚠️  Failed to mount {}:{} locally. The NFS share was configured on the remote", remote_ip, path);
            warn!("   but the local mount failed. You can try manually:");
            warn!("   sudo mount -t nfs {}:{} {}", remote_ip, path, local_mount_path);
        }
        
        Ok(local_mount_path.to_string())
    }
    
    /// Configures the NFS server to share the drive across the mesh (localhost)
    fn setup_nfs_server(&self, shared_path: &str) -> Result<()> {
        info!("🌐 Configuring NFS server to share {} on the local network...", shared_path);
        
        // Install NFS server if not present
        let install_status = std::process::Command::new("sudo")
            .arg("apt-get")
            .arg("install")
            .arg("-y")
            .arg("nfs-kernel-server")
            .status()
            .context("Failed to run apt-get for nfs-kernel-server")?;
            
        if !install_status.success() {
            tracing::warn!("Failed to install nfs-kernel-server. Shared drive may not be accessible to remote nodes.");
        }
        
        // Update /etc/exports
        let export_entry = format!("{} *(rw,sync,no_subtree_check,no_root_squash)", shared_path);
        
        let check_exports = std::process::Command::new("grep")
            .arg("-q")
            .arg(&export_entry)
            .arg("/etc/exports")
            .status()?;
            
        if !check_exports.success() {
            let echo_cmd = format!("echo '{}' | sudo tee -a /etc/exports", export_entry);
            std::process::Command::new("bash")
                .arg("-c")
                .arg(&echo_cmd)
                .status()
                .context("Failed to update /etc/exports")?;
                
            std::process::Command::new("sudo")
                .arg("exportfs")
                .arg("-a")
                .status()?;
                
            std::process::Command::new("sudo")
                .arg("systemctl")
                .arg("restart")
                .arg("nfs-kernel-server")
                .status()?;
                
            info!("✅ NFS server configured and restarted.");
        } else {
            info!("✅ NFS export already exists.");
        }
        
        Ok(())
    }
    
    /// Assess localhost capabilities with deep service discovery
    async fn assess_localhost_capabilities_with_services(&self) -> Result<crate::installer::ssh_client::DeviceCapabilities> {
        let mut caps = self.assess_localhost_capabilities().await?;
        
        // Use a dummy SSHClient connected to 127.0.0.1 just to run local commands
        // Actually, since we're local, we can just run the commands via std::process::Command
        let script = r#"
        # Check for Ollama
        OLLAMA_PATTERN="ollama ""serve"
        if pgrep -x ollama >/dev/null || pgrep -f "$OLLAMA_PATTERN" >/dev/null; then
            PORT=$(ss -tulnp 2>/dev/null | grep ollama | awk '{print $5}' | cut -d':' -f2 | head -n 1)
            if [ -z "$PORT" ]; then PORT=11434; fi
            echo "ollama:$PORT"
        fi
        
        # Check for ComfyUI
        COMFY_PATTERN1="main.py.*""comfyui"
        COMFY_PATTERN2="ComfyUI/""main.py"
        if pgrep -f "$COMFY_PATTERN1" >/dev/null || pgrep -f "$COMFY_PATTERN2" >/dev/null; then
            PORT=$(pgrep -f "$COMFY_PATTERN2" -a | grep -oP -- '--port\s+\K\d+' | head -n 1)
            if [ -z "$PORT" ]; then PORT=8188; fi
            echo "comfyui:$PORT"
        fi
        
        # Check for vLLM
        VLLM_PATTERN="vllm.entrypoints.openai.api""_server"
        VLLM_PATTERN2="vllm.entry""points"
        if pgrep -f "$VLLM_PATTERN" >/dev/null; then
            PORT=$(pgrep -f "$VLLM_PATTERN2" -a | grep -oP -- '--port\s+\K\d+' | head -n 1)
            if [ -z "$PORT" ]; then PORT=8000; fi
            echo "vllm:$PORT"
        fi

        # Check for LiteLLM
        LITELLM_PATTERN="lite""llm"
        if pgrep -f "$LITELLM_PATTERN" >/dev/null; then
            PORT=$(pgrep -f "$LITELLM_PATTERN" -a | grep -oP -- '--port\s+\K\d+' | head -n 1)
            if [ -z "$PORT" ]; then PORT=4000; fi
            echo "litellm:$PORT"
        fi

        # Check for SearXNG
        SEARX_PATTERN1="searx""ng"
        SEARX_PATTERN2="sea""rx"
        if pgrep -f "$SEARX_PATTERN1" >/dev/null || pgrep -f "$SEARX_PATTERN2" >/dev/null; then
            PORT=$(ss -tulnp 2>/dev/null | grep -i 'searx' | awk '{print $5}' | cut -d':' -f2 | head -n 1)
            if [ -z "$PORT" ]; then PORT=8080; fi
            echo "searxng:$PORT"
        fi
        "#;
        
        if let Ok(output) = std::process::Command::new("sh").arg("-c").arg(script).output() {
            let out_str = String::from_utf8_lossy(&output.stdout);
            for line in out_str.lines() {
                if let Some((name, port_str)) = line.split_once(':') {
                    if let Ok(port) = port_str.parse::<u16>() {
                        caps.services.push(crate::installer::ssh_client::DiscoveredService {
                            name: name.to_string(),
                            port,
                            details: std::collections::HashMap::new(),
                        });
                    }
                }
            }
        }
        
        // We can skip the curl queries for models here since download_default_model 
        // does its own self.ollama.list_models() check.
        
        Ok(caps)
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
        // First check if the user already has models downloaded
        let existing_models = self.ollama.list_models().await.unwrap_or_default();
        if !existing_models.is_empty() {
            info!("✅ Found existing models: {}", existing_models.join(", "));
            info!("Skipping default model download.");
            return Ok(());
        }

        let model_name = match self.tier {
            SystemTier::Tier1 => {
                info!("📦 Tier 1 system detected - downloading qwen2.5:0.5b");
                "qwen2.5:0.5b"
            }
            SystemTier::Tier2 => {
                info!("📦 Tier 2 system detected - downloading gemma2:2b");
                "gemma2:2b"
            }
            SystemTier::Tier3 | SystemTier::Tier4 => {
                info!("📦 Tier 3/4 system detected - downloading gemma2:9b");
                "gemma2:9b"
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

    /// Install ComfyUI if not present
    async fn install_comfyui(&mut self) -> Result<()> {
        info!("🎨 Checking ComfyUI installation...");
        
        if self.comfyui.is_installed().await? {
            info!("✅ ComfyUI already installed");
        } else {
            info!("📥 ComfyUI not found, installing...");
            if let Err(e) = self.comfyui.install().await {
                warn!("⚠️  ComfyUI installation failed: {}", e);
            } else {
                info!("✅ ComfyUI installed successfully");
            }
        }
        
        Ok(())
    }

    /// Install FFmpeg if not present
    async fn install_ffmpeg(&mut self) -> Result<()> {
        info!("🎬 Checking FFmpeg installation...");
        
        if self.ffmpeg.is_installed().await? {
            info!("✅ FFmpeg already installed");
        } else {
            info!("📥 FFmpeg not found, installing...");
            if let Err(e) = self.ffmpeg.install().await {
                warn!("⚠️  FFmpeg installation failed: {}", e);
            } else {
                info!("✅ FFmpeg installed successfully");
            }
        }
        
        Ok(())
    }
    
    /// Prompt user if they want to set up multi-device mesh
    #[allow(dead_code)]
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
    pub async fn discover_mesh_devices(&self, existing_shared_drive_path: Option<&str>) -> Result<Vec<DeviceCandidate>> {
        info!("🔍 Discovering devices on local network...");
        
        // Step 1: Ensure nmap is installed
        ensure_nmap_installed(&self.platform).await?;
        
        // Step 2: Scan local network
        let scanner = NetworkScanner::new()?;
        let mut devices = scanner.scan_local_network()?;
        
        // Always ensure localhost is in the device list
        let local_ip = local_ip_address::local_ip().map(|ip| ip.to_string()).unwrap_or_else(|_| "127.0.0.1".to_string());
        if !devices.iter().any(|d| d.ip == local_ip || d.ip == "127.0.0.1") {
            let actual_hostname = std::process::Command::new("hostname")
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|| "localhost".to_string());
                
            devices.push(DeviceCandidate {
                ip: local_ip.clone(),
                hostname: Some(actual_hostname),
                mac_address: None,
            });
        }
        
        // Step 3: Display discovered devices
        if devices.is_empty() {
            info!("⚠️  No devices with SSH found on local network");
            return Ok(devices);
        }
        
        info!("✅ Discovered {} devices with SSH enabled:", devices.len());
        for (i, device) in devices.iter().enumerate() {
            let hostname_display = match (&device.hostname, &device.mac_address) {
                (Some(h), _) => h.clone(),
                (None, Some(m)) => format!("unknown - MAC: {}", m),
                (None, None) => "unknown".to_string(),
            };
            info!("  [{}] {} ({})", i + 1, device.ip, hostname_display);
        }
        
        if self.prompt_assess_devices()? {
            let local_ips: Vec<String> = local_ip_address::list_afinet_netifas()
                .unwrap_or_default()
                .into_iter()
                .map(|(_, ip)| ip.to_string())
                .collect();
                
            let (local_devices, remote_devices): (Vec<_>, Vec<_>) = devices.clone().into_iter().partition(|d| {
                d.ip == "127.0.0.1" || d.ip == "localhost" || local_ips.contains(&d.ip)
            });

            let (mut capabilities, credentials_map) = self.assess_device_capabilities(&remote_devices).await?;

            if !local_devices.is_empty() {
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
            }

            self.display_capabilities(&capabilities);
            
            // Step 4.5: Set up shared drive if user doesn't have one yet
            let (shared_drive_path, host_ip_opt, remote_path_opt) = match existing_shared_drive_path {
                Some(path) => (path.to_string(), None, None),
                None => {
                    // Build device list from assessed capabilities (now we have real disk data)
                    let device_disk_info: Vec<(String, String, f64, bool)> = capabilities.iter().map(|cap| {
                        let is_local = cap.ip == "127.0.0.1" || cap.ip == "localhost" || local_ips.contains(&cap.ip);
                        (cap.hostname.clone(), cap.ip.clone(), cap.disk_gb, is_local)
                    }).collect();
                    
                    let (remote_path, host_ip, local_mount_path) = self.prompt_shared_drive_device_choice(&device_disk_info)?;
                    let is_local = device_disk_info.iter().any(|(_, ip, _, local)| ip == &host_ip && *local);
                    
                    if is_local {
                        self.setup_shared_drive_on_local(&remote_path)?;
                        (remote_path, None, None) // On local, path is same, no host IP needed for slaves to know
                    } else {
                        // Use existing credentials from device assessment
                        let creds = credentials_map.get(&host_ip);
                        self.setup_shared_drive_on_remote(&host_ip, &remote_path, &local_mount_path, creds)?;
                        (local_mount_path, Some(host_ip), Some(remote_path))
                    }
                }
            };
            
            // Step 5: Set up SSH keys and deploy (if user wants to proceed)
            if self.prompt_deploy_mesh()? {
                self.setup_and_deploy_mesh(&capabilities, credentials_map, &remote_devices, &shared_drive_path, host_ip_opt.as_deref(), remote_path_opt.as_deref()).await?;
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
    async fn setup_and_deploy_mesh(&self, capabilities: &[DeviceCapabilities], credentials_map: std::collections::HashMap<String, (String, String)>, scanned_devices: &[DeviceCandidate], shared_drive_path: &str, host_ip: Option<&str>, remote_path: Option<&str>) -> Result<()> {
        info!("\n🔐 Setting up SSH keys and deploying to mesh...");
        
        // Step 1: Generate SSH key pair (idempotent — reuses existing key)
        let key_manager = SSHKeyManager::new()?;
        key_manager.generate_key_pair("hainet-mesh")?;

        // Step 2: Distribute SSH key and set up passwordless sudo on devices that used password auth
        // (Skip devices that already authenticated via mesh key — they have it)
        info!("\n🔐 Distributing mesh keys and configuring sudo access...");
        for (ip, (username, password)) in &credentials_map {
            if password.is_empty() {
                info!("✓ {} — already configured (skipping)", ip);
                continue;
            }
            match key_manager.copy_to_remote(ip, username, password) {
                Ok(_) => info!("✓ Mesh key distributed to {}", ip),
                Err(e) => info!("⚠️  Failed to distribute mesh key to {}: {}", ip, e),
            }
            // Set up passwordless sudo for hainet commands while we still have the password
            if let Err(e) = key_manager.setup_sudoers_on_remote(ip, username, password) {
                info!("⚠️  Could not configure sudoers on {}: {}", ip, e);
            }
        }
        
        let username = credentials_map.values().next().map(|(u, _)| u.clone()).unwrap_or_else(|| "root".to_string());
        
        // Step 3: Display key info
        info!("\n📋 HAI-Net Mesh Key:");
        info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        info!("Key location: {}", key_manager.private_key_path().display());
        info!("This key persists for re-installs, updates, and uninstall.");
        info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        
        // Step 4: Assign roles and deploy
        let mut orchestrator = DeploymentOrchestrator::new();
        orchestrator.set_shared_drive_path(shared_drive_path.to_string());
        if let Some(ip) = host_ip {
            orchestrator.set_shared_drive_host_ip(ip.to_string());
        }
        if let Some(path) = remote_path {
            orchestrator.set_shared_drive_remote_path(path.to_string());
        }
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
            orchestrator.deploy_all(&username, &credentials_map, client_factory).await?;
            
            // Step 5: Save mesh manifest with MAC addresses for IP change resilience
            use crate::installer::ssh_keys::{MeshManifest, MeshNode};
            let manifest = MeshManifest {
                updated_at: chrono::Utc::now().to_rfc3339(),
                nodes: orchestrator.assignments().iter().map(|a| {
                    let node_username = credentials_map.get(&a.ip)
                        .map(|(u, _)| u.clone())
                        .unwrap_or_else(|| username.clone());
                    // Look up MAC address from scan results
                    let mac = scanned_devices.iter()
                        .find(|d| d.ip == a.ip)
                        .and_then(|d| d.mac_address.clone());
                    MeshNode {
                        ip: a.ip.clone(),
                        hostname: a.hostname.clone(),
                        username: node_username,
                        role: format!("{}", a.role),
                        mac_address: mac,
                    }
                }).collect(),
            };
            key_manager.save_manifest(&manifest)?;
            
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
    async fn assess_device_capabilities(&self, devices: &[DeviceCandidate]) -> Result<(Vec<DeviceCapabilities>, std::collections::HashMap<String, (String, String)>)> {
        use std::io::{self, Write};
        use std::collections::HashMap;

        let mut capabilities = Vec::new();
        let mut credentials_map = HashMap::new();
        
        // Check if we have a saved mesh manifest and key for re-installs
        let key_manager = SSHKeyManager::new()?;
        let manifest = key_manager.load_manifest();
        let has_mesh_key = key_manager.has_key_pair();
        
        if has_mesh_key && manifest.is_some() {
            info!("🔑 Found existing HAI-Net mesh key — attempting key-based auth (no passwords needed)...");
            
            // Check for IP changes: try to reconnect manifest nodes that aren't in the scan
            if let Some(ref m) = manifest {
                for mnode in &m.nodes {
                    let still_in_scan = devices.iter().any(|d| d.ip == mnode.ip);
                    if !still_in_scan {
                        // This node's IP may have changed — try to find it by MAC or hostname
                        let new_ip = devices.iter().find(|d| {
                            // Match by MAC address (most reliable)
                            if let (Some(ref scan_mac), Some(ref manifest_mac)) = (&d.mac_address, &mnode.mac_address) {
                                if scan_mac.to_lowercase() == manifest_mac.to_lowercase() {
                                    return true;
                                }
                            }
                            // Fall back to hostname match
                            if let Some(ref scan_host) = d.hostname {
                                if scan_host.to_lowercase() == mnode.hostname.to_lowercase() {
                                    return true;
                                }
                            }
                            false
                        });
                        
                        if let Some(found) = new_ip {
                            info!("🔄 IP change detected for {}: {} → {} (matched by {})",
                                mnode.hostname, mnode.ip, found.ip,
                                if found.mac_address.is_some() { "MAC address" } else { "hostname" }
                            );
                        }
                    }
                }
            }
        }
        
        // Then assess remote devices
        for device in devices {
            info!("\n🔍 Assessing device: {}", device.ip);
            
            // Try 1: If we have a mesh key, try key auth silently first
            if has_mesh_key {
                // Find username: check manifest by current IP, then by MAC/hostname for moved devices
                let username = manifest.as_ref()
                    .and_then(|m| {
                        // Direct IP match first
                        m.nodes.iter().find(|n| n.ip == device.ip)
                            .or_else(|| {
                                // MAC address match (device may have new IP)
                                m.nodes.iter().find(|n| {
                                    if let (Some(ref manifest_mac), Some(ref scan_mac)) = (&n.mac_address, &device.mac_address) {
                                        manifest_mac.to_lowercase() == scan_mac.to_lowercase()
                                    } else {
                                        false
                                    }
                                })
                            })
                            .or_else(|| {
                                // Hostname match
                                if let Some(ref scan_host) = device.hostname {
                                    m.nodes.iter().find(|n| n.hostname.to_lowercase() == scan_host.to_lowercase())
                                } else {
                                    None
                                }
                            })
                    })
                    .map(|n| n.username.clone())
                    .unwrap_or_else(|| std::env::var("USER").unwrap_or_else(|_| "root".to_string()));
                
                if key_manager.test_mesh_key_auth(&device.ip, &username) {
                    info!("✓ Mesh key auth succeeded for {}@{}", username, device.ip);
                    
                    let credentials = SSHCredentials {
                        username: username.clone(),
                        password: String::new(),
                    };
                    let mut client = SSHClient::new(device.ip.clone(), credentials);
                    
                    if client.connect().is_ok() {
                        if client.authenticate_pubkey(key_manager.private_key_path(), None).is_ok() {
                            match client.assess_capabilities() {
                                Ok(caps) => {
                                    credentials_map.insert(device.ip.clone(), (username, String::new()));
                                    capabilities.push(caps);
                                    let _ = client.disconnect();
                                    continue; // Skip password prompt
                                }
                                Err(e) => {
                                    info!("⚠️  Key auth worked but capability assessment failed: {}", e);
                                }
                            }
                        }
                        let _ = client.disconnect();
                    }
                }
            }
            
            // Try 2: Fall back to password-based authentication
            loop {
                // Prompt for credentials per device (they might differ)
                print!("Username for {} (default: current user, type 'skip' to ignore): ", device.ip);
                io::stdout().flush()?;
                let mut username = String::new();
                io::stdin().read_line(&mut username)?;
                let username = username.trim();
                
                if username.eq_ignore_ascii_case("skip") {
                    info!("⏭️ Skipping device {}", device.ip);
                    break;
                }
                
                let username = if username.is_empty() {
                    std::env::var("USER").unwrap_or_else(|_| "root".to_string())
                } else {
                    username.to_string()
                };
                
                let password = dialoguer::Password::new()
                    .with_prompt(format!("Password for {}@{}", username, device.ip))
                    .interact()?;
                
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

                                credentials_map.insert(device.ip.clone(), (username, password));
                                
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
        
        Ok((capabilities, credentials_map))
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
            
        // Use our new comprehensive discovery script for local services too
        let mut services = Vec::new();
        let script = r#"
        # Check for Ollama
        OLLAMA_PATTERN="ollama ""serve"
        if pgrep -x ollama >/dev/null || pgrep -f "$OLLAMA_PATTERN" >/dev/null; then
            PORT=$(ss -tulnp 2>/dev/null | grep ollama | awk '{print $5}' | cut -d':' -f2 | head -n 1)
            if [ -z "$PORT" ]; then PORT=11434; fi
            echo "ollama:$PORT"
        fi
        
        # Check for ComfyUI
        COMFY_PATTERN1="main.py.*""comfyui"
        COMFY_PATTERN2="ComfyUI/""main.py"
        if pgrep -f "$COMFY_PATTERN1" >/dev/null || pgrep -f "$COMFY_PATTERN2" >/dev/null; then
            PORT=$(pgrep -f "$COMFY_PATTERN2" -a | grep -oP -- '--port\s+\K\d+' | head -n 1)
            if [ -z "$PORT" ]; then PORT=8188; fi
            echo "comfyui:$PORT"
        fi
        
        # Check for vLLM
        VLLM_PATTERN="vllm.entrypoints.openai.api""_server"
        VLLM_PATTERN2="vllm.entry""points"
        if pgrep -f "$VLLM_PATTERN" >/dev/null; then
            PORT=$(pgrep -f "$VLLM_PATTERN2" -a | grep -oP -- '--port\s+\K\d+' | head -n 1)
            if [ -z "$PORT" ]; then PORT=8000; fi
            echo "vllm:$PORT"
        fi

        # Check for LiteLLM
        LITELLM_PATTERN="lite""llm"
        if pgrep -f "$LITELLM_PATTERN" >/dev/null; then
            PORT=$(pgrep -f "$LITELLM_PATTERN" -a | grep -oP -- '--port\s+\K\d+' | head -n 1)
            if [ -z "$PORT" ]; then PORT=4000; fi
            echo "litellm:$PORT"
        fi

        # Check for SearXNG
        SEARX_PATTERN1="searx""ng"
        SEARX_PATTERN2="sea""rx"
        if pgrep -f "$SEARX_PATTERN1" >/dev/null || pgrep -f "$SEARX_PATTERN2" >/dev/null; then
            PORT=$(ss -tulnp 2>/dev/null | grep -i 'searx' | awk '{print $5}' | cut -d':' -f2 | head -n 1)
            if [ -z "$PORT" ]; then PORT=8080; fi
            echo "searxng:$PORT"
        fi
        "#;
        
        if let Ok(output) = Command::new("sh").arg("-c").arg(script).output() {
            let out_str = String::from_utf8_lossy(&output.stdout);
            for line in out_str.lines() {
                if let Some((name, port_str)) = line.split_once(':') {
                    if let Ok(port) = port_str.parse::<u16>() {
                        services.push(crate::installer::ssh_client::DiscoveredService {
                            name: name.to_string(),
                            port,
                            details: std::collections::HashMap::new(),
                        });
                    }
                }
            }
        }
        
        // Models querying for local services
        for s in &mut services {
            if s.name == "ollama" || s.name == "vllm" {
                let endpoint = if s.name == "ollama" {
                    format!("http://localhost:{}/api/tags", s.port)
                } else {
                    format!("http://localhost:{}/v1/models", s.port)
                };
                let curl_cmd = format!("curl -s --max-time 3 {}", endpoint);
                if let Ok(output) = Command::new("sh").arg("-c").arg(&curl_cmd).output() {
                    let resp = String::from_utf8_lossy(&output.stdout);
                    if s.name == "ollama" {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&resp) {
                            if let Some(models) = json.get("models").and_then(|m| m.as_array()) {
                                let model_names: Vec<String> = models.iter()
                                    .filter_map(|m| m.get("name").and_then(|n| n.as_str()).map(String::from))
                                    .collect();
                                s.details.insert("models".to_string(), model_names.join(","));
                            }
                        }
                    } else if s.name == "vllm" {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&resp) {
                            if let Some(data) = json.get("data").and_then(|m| m.as_array()) {
                                let model_names: Vec<String> = data.iter()
                                    .filter_map(|m| m.get("id").and_then(|n| n.as_str()).map(String::from))
                                    .collect();
                                s.details.insert("models".to_string(), model_names.join(","));
                            }
                        }
                    }
                }
            }
        }
        
        let mut capabilities = DeviceCapabilities {
            ip: local_ip,
            hostname,
            cpu_cores,
            ram_gb,
            gpu,
            disk_gb,
            os,
            arch,
            services,
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
        
        info!("\n📊 Device Capabilities & Discovered Services Summary:");
        info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        
        for caps in capabilities {
            info!("Device: {} ({})", caps.hostname, caps.ip);
            info!("  CPU: {} cores", caps.cpu_cores);
            info!("  RAM: {:.1} GB", caps.ram_gb);
            info!("  GPU: {}", caps.gpu.as_deref().unwrap_or("None"));
            info!("  Disk: {:.1} GB available", caps.disk_gb);
            info!("  OS: {} ({})", caps.os, caps.arch);
            info!("  Score: {:.1}", caps.score);
            
            if caps.services.is_empty() {
                info!("  Services: None detected");
            } else {
                info!("  Services:");
                for service in &caps.services {
                    let mut details_str = String::new();
                    if let Some(models) = service.details.get("models") {
                        details_str = format!(" [Models: {}]", models);
                    }
                    info!("    - {} (Port: {}){}", service.name, service.port, details_str);
                }
            }
            info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        }
        
        // Suggest master node (highest score)
        if let Some(master) = capabilities.iter().max_by(|a, b| a.score.partial_cmp(&b.score).unwrap()) {
            info!("\n🎯 Recommended Master Node: {} ({})", master.hostname, master.ip);
            info!("   Score: {:.1} (Best hardware for coordination)", master.score);
        }
        
        info!("\n📋 Review the deployment plan above. The orchestrator will intelligently");
        info!("   skip installing services that are already running.");
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
