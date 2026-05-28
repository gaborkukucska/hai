//! # START OF FILE hainet-seed/src/installer/deployment.rs
//! Remote deployment orchestrator for multi-device HAI-Net mesh.
//! Handles role assignment, binary deployment, and service initialization.

use anyhow::{Result, bail, Context};
use crate::installer::ssh_client::{DeviceCapabilities, SSHCredentials, SSHClientTrait};
use std::collections::HashMap;
use std::path::PathBuf;
#[allow(unused_imports)]
use tracing::{info, debug, warn};

/// Device role in the HAI-Net mesh
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DeviceRole {
    /// Master node (coordination, blockchain, primary storage)
    Master,
    /// Slave node (compute, storage replication)
    Slave,
    /// Standalone node (not part of mesh)
    Standalone,
    /// UI-Only node (mobile devices, remote portal only)
    UIOnly,
}

impl std::fmt::Display for DeviceRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeviceRole::Master => write!(f, "Master"),
            DeviceRole::Slave => write!(f, "Slave"),
            DeviceRole::Standalone => write!(f, "Standalone"),
            DeviceRole::UIOnly => write!(f, "UI-Only"),
        }
    }
}

impl DeviceRole {
    /// Check if this role requires full HAI-Net stack
    pub fn requires_full_stack(&self) -> bool {
        matches!(self, DeviceRole::Master | DeviceRole::Slave | DeviceRole::Standalone)
    }
    
    /// Check if this role is mobile/remote UI only
    pub fn is_ui_only(&self) -> bool {
        matches!(self, DeviceRole::UIOnly)
    }
}

/// Device assignment with role
#[derive(Debug, Clone)]
pub struct DeviceAssignment {
    pub ip: String,
    pub hostname: String,
    pub role: DeviceRole,
    pub capabilities: DeviceCapabilities,
}

/// Remote deployment orchestrator
pub struct DeploymentOrchestrator {
    assignments: Vec<DeviceAssignment>,
    /// The local mount point used by all nodes (e.g., /media/hai-drive)
    shared_drive_path: String,
    /// The IP of the device hosting the NFS share
    shared_drive_host_ip: Option<String>,
    /// The export path on the hosting device (e.g., /media/fast/NoSlop on BigBOY)
    shared_drive_remote_path: Option<String>,
}

/// Returns the systemd service names that should be deployed for a given role.
pub fn services_for_role(role: &DeviceRole) -> Vec<&'static str> {
    match role {
        DeviceRole::Master => vec!["hainet-core", "hainet-chain", "hainet-bridge", "hainet-portal"],
        DeviceRole::Slave => vec!["hainet-core", "hainet-chain"],
        DeviceRole::Standalone => vec!["hainet-core", "hainet-chain", "hainet-bridge", "hainet-portal"],
        DeviceRole::UIOnly => vec!["hainet-portal"],
    }
}

impl DeploymentOrchestrator {
    /// Create new deployment orchestrator
    pub fn new() -> Self {
        Self {
            assignments: Vec::new(),
            shared_drive_path: "/media/hai-drive".to_string(),
            shared_drive_host_ip: None,
            shared_drive_remote_path: None,
        }
    }
    
    /// Set the local mount path (used by all nodes to access the shared drive)
    pub fn set_shared_drive_path(&mut self, path: String) {
        self.shared_drive_path = path;
    }
    
    /// Set the IP of the device hosting the NFS share
    pub fn set_shared_drive_host_ip(&mut self, ip: String) {
        self.shared_drive_host_ip = Some(ip);
    }
    
    /// Set the export path on the hosting device (may differ from local mount path)
    pub fn set_shared_drive_remote_path(&mut self, path: String) {
        self.shared_drive_remote_path = Some(path);
    }
    
    /// Assign roles to devices based on capabilities
    /// 
    /// Role assignment strategy:
    /// 1. Devices with < 2GB RAM → UI-Only (mobile devices)
    /// 2. Highest scoring device (≥ 2GB RAM) → Master
    /// 3. Remaining devices (≥ 2GB RAM) → Slaves
    /// 4. Single device → Standalone (unless mobile, then UI-Only)
    /// 
    /// # Errors
    /// Returns an error if no devices are available
    pub fn assign_roles(&mut self, capabilities: Vec<DeviceCapabilities>) -> Result<()> {
        if capabilities.is_empty() {
            bail!("No devices available for role assignment");
        }
        
        // Separate mobile devices (< 2GB RAM) from compute-capable devices
        let (mobile_devices, compute_devices): (Vec<_>, Vec<_>) = capabilities
            .into_iter()
            .partition(|d| d.ram_gb < 2.0);
        
        // Handle single device case
        if compute_devices.is_empty() && mobile_devices.len() == 1 {
            println!("⚠️  Only 1 mobile device available - assigning UI-Only role");
            let device = &mobile_devices[0];
            self.assignments.push(DeviceAssignment {
                ip: device.ip.clone(),
                hostname: device.hostname.clone(),
                role: DeviceRole::UIOnly,
                capabilities: device.clone(),
            });
            return Ok(());
        }
        
        if compute_devices.len() == 1 && mobile_devices.is_empty() {
            println!("⚠️  Only 1 device available - assigning Standalone role");
            let device = &compute_devices[0];
            self.assignments.push(DeviceAssignment {
                ip: device.ip.clone(),
                hostname: device.hostname.clone(),
                role: DeviceRole::Standalone,
                capabilities: device.clone(),
            });
            return Ok(());
        }
        
        // Sort compute devices by capability score (descending)
        let mut sorted_devices = compute_devices.clone();
        sorted_devices.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        
        println!("\n📋 Role Assignment:");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        
        // Assign mobile devices to UI-Only role
        if !mobile_devices.is_empty() {
            for device in &mobile_devices {
                println!("📱 UI-Only: {} ({}) - {:.1}GB RAM (mobile device)", 
                         device.hostname, device.ip, device.ram_gb);
                
                self.assignments.push(DeviceAssignment {
                    ip: device.ip.clone(),
                    hostname: device.hostname.clone(),
                    role: DeviceRole::UIOnly,
                    capabilities: device.clone(),
                });
            }
            println!(); // Blank line for readability
        }
        
        // Assign master to highest scoring compute device
        if !sorted_devices.is_empty() {
            let master = &sorted_devices[0];
            println!("🎯 Master: {} ({}) - Score: {:.1}", 
                     master.hostname, master.ip, master.score);
            
            self.assignments.push(DeviceAssignment {
                ip: master.ip.clone(),
                hostname: master.hostname.clone(),
                role: DeviceRole::Master,
                capabilities: master.clone(),
            });
            
            // Assign slaves to remaining compute devices
            for device in sorted_devices.iter().skip(1) {
                println!("   Slave: {} ({}) - Score: {:.1}", 
                         device.hostname, device.ip, device.score);
                
                self.assignments.push(DeviceAssignment {
                    ip: device.ip.clone(),
                    hostname: device.hostname.clone(),
                    role: DeviceRole::Slave,
                    capabilities: device.clone(),
                });
            }
        }
        
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        
        Ok(())
    }
    
    /// Get device assignments
    pub fn assignments(&self) -> &[DeviceAssignment] {
        &self.assignments
    }
    
    /// Get master node assignment (if any)
    pub fn master_node(&self) -> Option<&DeviceAssignment> {
        self.assignments.iter().find(|a| a.role == DeviceRole::Master)
    }
    
    /// Get slave node assignments
    pub fn slave_nodes(&self) -> Vec<&DeviceAssignment> {
        self.assignments.iter()
            .filter(|a| a.role == DeviceRole::Slave)
            .collect()
    }
    
    /// Deploy HAI-Net to assigned devices
    /// 
    /// Deployment steps:
    /// 1. Deploy binaries to remote devices
    /// 2. Configure roles (master/slave)
    /// 3. Initialize storage mesh
    /// 4. Start services
    /// 
    /// # Note
    /// This is a placeholder. Actual deployment requires:
    /// - Binary compilation for target architectures
    /// - SCP/rsync for file transfer
    /// - Remote systemd service creation
    /// - Distributed storage initialization
    pub async fn deploy_all<'a, F, C>(&'a self, _username: &str, credentials_map: &std::collections::HashMap<String, (String, String)>, mut client_factory: F) -> Result<()>
    where
        F: FnMut(String, SSHCredentials) -> C,
        C: SSHClientTrait + 'a,
    {
        println!("\n🚀 Starting deployment to {} devices...", self.assignments.len());
        
        if self.assignments.is_empty() {
            bail!("No device assignments. Call assign_roles() first.");
        }
        
        // Display deployment plan
        self.display_deployment_plan();
        
        // Build binaries once for each required architecture
        let required_arches = self.assignments.iter()
            .map(|a| a.capabilities.arch.clone())
            .collect::<std::collections::HashSet<_>>();

        for arch in required_arches {
            self.build_binaries(&arch)?;
        }

        let local_ip = local_ip_address::local_ip().ok();

        // Deploy to each device
        for assignment in &self.assignments {
            let is_local = local_ip.as_ref().map_or(false, |ip| assignment.ip == ip.to_string());
            if is_local {
                self.deploy_to_localhost(assignment).await?;
            } else {
                if let Some((username, _)) = credentials_map.get(&assignment.ip) {
                    self.deploy_to_device(assignment, username, &mut client_factory).await?;
                }
            }
        }
        
        println!("\n✅ Deployment complete!");
        
        // Initialize mesh coordination
        if let Some(master) = self.master_node() {
            self.initialize_mesh(master, credentials_map, client_factory).await?;
        }
        
        Ok(())
    }
    
    /// Display deployment plan
    fn display_deployment_plan(&self) {
        println!("\n📋 Deployment Plan:");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        
        for assignment in &self.assignments {
            let role_emoji = match assignment.role {
                DeviceRole::Master => "👑",
                DeviceRole::Slave => "⚙️",
                DeviceRole::Standalone => "🔹",
                DeviceRole::UIOnly => "📱",
            };
            
            println!("{} {} - {} ({})", 
                     role_emoji, 
                     assignment.role, 
                     assignment.hostname, 
                     assignment.ip);
            
            let services = match assignment.role {
                DeviceRole::Master => vec![
                    "hainet-core (Master mode)",
                    "hainet-chain (Blockchain)",
                    "hainet-bridge (Gateway)",
                    "hainet-portal (UI)",
                ],
                DeviceRole::Slave => vec![
                    "hainet-core (Slave mode)",
                    "hainet-chain (Validator)",
                ],
                DeviceRole::Standalone => vec![
                    "hainet-core (Standalone)",
                    "hainet-portal (UI)",
                ],
                DeviceRole::UIOnly => vec![
                    "hainet-portal (UI only - connects to home hub)",
                ],
            };
            
            for service in services {
                println!("  • {}", service);
            }
        }
        
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    }
    
    /// Deploy HAI-Net to a single device
    async fn deploy_to_device<'a, F, C>(&'a self, assignment: &DeviceAssignment, username: &str, client_factory: &mut F) -> Result<()>
    where
        F: FnMut(String, SSHCredentials) -> C,
        C: SSHClientTrait + 'a,
    {
        println!("\n📦 Deploying to {} ({})...", assignment.hostname, assignment.ip);
        
        use std::path::Path;
        
        // Step 1: Connect via SSH
        let credentials = SSHCredentials {
            username: username.to_string(),
            password: String::new(), // SSH key auth assumed
        };
        
        let mut client = client_factory(assignment.ip.clone(), credentials);
        client.connect()?;
        
        // Use hainet-mesh key authentication (distributed during install)
        let key_path = dirs::home_dir()
            .unwrap_or_else(|| Path::new("/root").to_path_buf())
            .join(".ssh/hainet-mesh");
        
        client.authenticate_pubkey(&key_path, None)?;
        
        // Step 1.5: Stop any existing services for clean re-deployment
        println!("🔄 Stopping existing services (if any)...");
        let _ = client.execute_command("sudo -n systemctl stop hainet-core hainet-chain hainet-bridge hainet-portal 2>/dev/null || true");
        
        // Step 2: Create system user and directories
        println!("🔧 Creating hainet system user...");
        self.create_system_user(&client)?;
        
        println!("📁 Creating system directories...");
        self.create_system_directories(&client)?;
        
        // Step 2.5: Mount shared network drive from master
        if assignment.role != DeviceRole::Master {
            println!("🌐 Mounting shared network drive from master...");
            if let Some(master) = self.master_node() {
                self.mount_shared_drive_on_remote(&client, &master.ip)?;
            } else {
                warn!("⚠️  No master node defined, skipping NFS mount for {}", assignment.hostname);
            }
        }
        
        // Step 3: Transfer binaries based on role
        println!("📤 Transferring binaries...");
        self.transfer_binaries(&client, &assignment.role, &assignment.capabilities.arch)?;
        
        // Step 3.5: Transfer AI persona prompts
        let base = if self.shared_drive_host_ip.as_deref() == Some(&assignment.ip) {
            self.shared_drive_remote_path.as_deref().unwrap_or(&self.shared_drive_path)
        } else {
            &self.shared_drive_path
        };
        println!("📤 Transferring AI persona prompts...");
        self.transfer_prompts(&client, base)?;
        
        println!("📤 Transferring system configs...");
        self.transfer_configs(&client)?;
        
        // Step 4: Configure role-specific settings
        println!("⚙️  Configuring role settings...");
        self.configure_device(&client, &assignment)?;
        
        // Step 5: Create and enable systemd services
        println!("🔧 Setting up system services...");
        self.setup_services(&client, &assignment.role)?;
        
        // Disconnect
        client.disconnect()?;
        
        println!("✓ Deployment to {} complete", assignment.hostname);
        
        Ok(())
    }

    /// Deploy HAI-Net to the local machine (idempotent - safe for re-deployments)
    async fn deploy_to_localhost(&self, assignment: &DeviceAssignment) -> Result<()> {
        println!("\n📦 Deploying to localhost ({})...", assignment.hostname);

        use std::process::Command;

        // Step 0: Stop any existing services for clean re-deployment
        println!("🔄 Checking for existing deployment...");
        for svc in &["hainet-core", "hainet-chain", "hainet-bridge", "hainet-portal"] {
            let svc_name = format!("{}.service", svc);
            // Only try to stop if the service unit file exists
            let exists = Command::new("systemctl")
                .args(["list-unit-files", &svc_name])
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).contains(&svc_name))
                .unwrap_or(false);
            if exists {
                let _ = Command::new("sudo")
                    .args(&["systemctl", "stop", &svc_name])
                    .status();
            }
        }

        // Step 1: Create system user and align to UID/GID 995
        println!("🔧 Creating/aligning hainet system user...");
        let _ = Command::new("sudo").args(&["groupadd", "-g", "995", "hainet"]).status();
        let _ = Command::new("sudo").args(&["groupadd", "hainet"]).status();
        let _ = Command::new("sudo")
            .args(&["useradd", "-r", "-u", "995", "-g", "hainet", "-s", "/bin/false", "-d", "/var/lib/hainet", "-m", "hainet"])
            .status();
        let _ = Command::new("sudo").args(&["usermod", "-u", "995", "hainet"]).status();
        let _ = Command::new("sudo").args(&["groupmod", "-g", "995", "hainet"]).status();
        println!("✓ System user 'hainet' created (aligned to UID 995)");

        // Step 2: Create system directories
        println!("📁 Creating system directories...");
        let dirs = vec!["/usr/local/bin", "/etc/hainet", "/var/lib/hainet", "/var/log/hainet"];
        for dir in dirs {
            Command::new("sudo")
                .args(&["mkdir", "-p", dir])
                .status()?;
        }
        
        // Set ownership
        Command::new("sudo")
            .args(&["chown", "-R", "hainet:hainet", "/etc/hainet", "/var/lib/hainet", "/var/log/hainet"])
            .status()?;

        // Step 3: Copy binaries to system directories
        println!("📤 Copying binaries...");
        let binaries = match assignment.role {
            DeviceRole::Master => vec!["hainet-core", "hainet-chain", "hainet-bridge"],
            DeviceRole::Slave => vec!["hainet-core", "hainet-chain"],
            DeviceRole::Standalone => vec!["hainet-core", "hainet-chain", "hainet-bridge"],
            DeviceRole::UIOnly => vec!["hainet-portal"],
        };
        // Use target-triple-aware path to match where build_binaries() puts output
        let workspace_root = find_workspace_root()?;
        let host_arch = std::env::consts::ARCH;
        let target_triple = get_target_triple(host_arch);
        let target_dir = match target_triple {
            Some(triple) => {
                let triple_path = workspace_root.join("target").join(triple).join("release");
                let plain_path = workspace_root.join("target/release");
                // Prefer the triple path (build_binaries uses --target), fall back to plain
                if triple_path.exists() { triple_path } else { plain_path }
            }
            None => workspace_root.join("target/release"),
        };
        info!("📂 Looking for compiled binaries in: {}", target_dir.display());
        for binary_name in binaries {
            let source_path = target_dir.join(binary_name);
            if source_path.exists() {
                info!("✓ Found binary: {}", source_path.display());
                Command::new("sudo")
                    .args(&["cp", source_path.to_str().unwrap(), "/usr/local/bin/"])
                    .status()?;
                Command::new("sudo")
                    .args(&["chmod", "+x", &format!("/usr/local/bin/{}", binary_name)])
                    .status()?;
                println!("✓ Installed {} to /usr/local/bin/", binary_name);
            } else {
                warn!("⚠️  Binary not found at {}. Service will fail to start!", source_path.display());
                println!("⚠️  Binary {} not found at {} — skipping", binary_name, source_path.display());
            }
        }

        // Step 3.5: Copy AI persona prompts
        println!("📤 Copying AI persona prompts...");
        let base = if self.shared_drive_host_ip.as_deref() == Some(&assignment.ip) {
            self.shared_drive_remote_path.as_deref().unwrap_or(&self.shared_drive_path)
        } else {
            &self.shared_drive_path
        };
        self.copy_prompts_local(base)?;

        // Step 3.6: Copy system configs
        println!("📤 Copying system configs...");
        self.copy_configs_local()?;

        // Step 4: Configure role-specific settings
        println!("⚙️  Configuring role settings...");
        // Determine correct log directory (same logic as remote configure_device)
        let is_nfs_host = self.shared_drive_host_ip.as_deref().map_or(false, |hip| {
            // For localhost, check if the NFS host IP matches any local IP
            let local_ips: Vec<String> = local_ip_address::list_afinet_netifas()
                .unwrap_or_default()
                .into_iter()
                .map(|(_, ip)| ip.to_string())
                .collect();
            local_ips.contains(&hip.to_string())
        });
        let log_dir = if is_nfs_host {
            let base = self.shared_drive_remote_path.as_deref().unwrap_or(&self.shared_drive_path);
            format!("{}/logs", base)
        } else {
            format!("{}/logs", self.shared_drive_path)
        };
        let data_dir = if is_nfs_host {
            let base = self.shared_drive_remote_path.as_deref().unwrap_or(&self.shared_drive_path);
            format!("{}/data", base)
        } else {
            format!("{}/data", self.shared_drive_path)
        };
        let config_content = match assignment.role {
            DeviceRole::Master => format!("[network]\nrole = \"master\"\nport = 8080\n\n[storage]\ndata_dir = \"{}\"\n\n[logs]\nlog_dir = \"{}\"\nlog_level = \"info\"\n", data_dir, log_dir),
            DeviceRole::Slave => {
                let master_ip = self.master_node().map(|m| m.ip.as_str()).unwrap_or("10.0.0.10");
                format!("[network]\nrole = \"slave\"\nmaster_ip = \"{}\"\nport = 8080\n\n[storage]\ndata_dir = \"{}\"\n\n[logs]\nlog_dir = \"{}\"\nlog_level = \"info\"\n", master_ip, data_dir, log_dir)
            },
            _ => format!("[network]\nrole = \"standalone\"\nport = 8080\n\n[storage]\ndata_dir = \"{}\"\n\n[logs]\nlog_dir = \"{}\"\nlog_level = \"info\"\n", data_dir, log_dir),
        };

        let config_path = "/tmp/hainet.toml";
        std::fs::write(config_path, config_content)?;
        Command::new("sudo")
            .args(&["mv", config_path, "/etc/hainet/hainet.toml"])
            .status()?;
        Command::new("sudo")
            .args(&["chown", "hainet:hainet", "/etc/hainet/hainet.toml"])
            .status()?;

        // Ensure log and data directories exist and are writable by hainet
        let _ = Command::new("sudo")
            .args(&["mkdir", "-p", &log_dir])
            .status();
        let _ = Command::new("sudo")
            .args(&["chown", "-R", "hainet:hainet", &log_dir])
            .status();
        let _ = Command::new("sudo")
            .args(&["mkdir", "-p", &data_dir])
            .status();
        let _ = Command::new("sudo")
            .args(&["chown", "-R", "hainet:hainet", &data_dir])
            .status();


        // Step 5: Create and enable systemd system services
        println!("🔧 Setting up system services...");
        let services: Vec<&str> = services_for_role(&assignment.role);
        for service_name in &services {
            let service_content = format!(
                "[Unit]
Description=HAI-Net {}
After=network.target

[Service]
Type=simple
User=hainet
Group=hainet
ExecStart=/usr/local/bin/{}
Restart=always
RestartSec=10
Environment=RUST_LOG=info
WorkingDirectory=/var/lib/hainet
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
",
                service_name, service_name
            );
            let service_path = format!("/tmp/{}.service", service_name);
            std::fs::write(&service_path, service_content)?;
            
            Command::new("sudo")
                .args(&["mv", &service_path, &format!("/etc/systemd/system/{}.service", service_name)])
                .status()?;
        }
        
        Command::new("sudo")
            .args(&["systemctl", "daemon-reload"])
            .status()?;
        
        for service_name in &services {
            Command::new("sudo")
                .args(&["systemctl", "restart", &format!("{}.service", service_name)])
                .status()?;
        }

        // Step 6: Start services immediately
        println!("🚀 Starting services...");
        for service_name in &services {
            let status = Command::new("sudo")
                .args(&["systemctl", "start", &format!("{}.service", service_name)])
                .status()?;
            
            if status.success() {
                println!("✓ {} started successfully", service_name);
            } else {
                println!("⚠️  Failed to start {}", service_name);
            }
        }

        println!("✓ Deployment to localhost complete");

        Ok(())
    }
    
    /// Initialize mesh coordination
    /// 
    /// Starts services on all deployed devices and verifies mesh health.
    /// 
    /// # Implementation Notes
    /// This is Phase 7B implementation - service orchestration.
    /// Full P2P mesh networking (libp2p) will be implemented in Phase 8.
    async fn initialize_mesh<'a, F, C>(&'a self, master: &DeviceAssignment, credentials_map: &std::collections::HashMap<String, (String, String)>, mut client_factory: F) -> Result<()>
    where
        F: FnMut(String, SSHCredentials) -> C,
        C: SSHClientTrait + 'a,
    {
        println!("\n🌐 Initializing mesh network...");
        println!("   Master: {} ({})", master.hostname, master.ip);
        
        let slave_count = self.slave_nodes().len();
        println!("   Slaves: {}", slave_count);
        
        // Step 1: Start services on master node
        println!("\n🚀 Starting services on master node...");
        let local_ip = local_ip_address::local_ip().ok();
        let master_is_local = local_ip.as_ref().map_or(false, |ip| master.ip == ip.to_string());
        
        if master_is_local {
            // Master is localhost, services already started during deploy_to_localhost
            println!("   ✓ Master services already running on localhost");
        } else if let Some((username, _)) = credentials_map.get(&master.ip) {
            self.start_services_on_device(&master.ip, username, &master.role, &mut client_factory).await?;
        }

        // Step 2: Start services on slave nodes
        if slave_count > 0 {
            println!("\n🚀 Starting services on {} slave node(s)...", slave_count);
            for slave in self.slave_nodes() {
                if let Some((username, _)) = credentials_map.get(&slave.ip) {
                    self.start_services_on_device(&slave.ip, username, &slave.role, &mut client_factory).await?;
                }
            }
        }
        
        // Step 3: Verify mesh health
        println!("\n🔍 Verifying mesh health...");
        if let Some((username, _)) = credentials_map.get(&master.ip) {
            self.verify_mesh_health(master, username, &mut client_factory).await?;
        }
        for slave in self.slave_nodes() {
            if let Some((username, _)) = credentials_map.get(&slave.ip) {
                self.verify_mesh_health(slave, username, &mut client_factory).await?;
            }
        }
        
        println!("\n✅ Mesh network initialized successfully!");
        println!("   Master: {} (services running)", master.hostname);
        println!("   Slaves: {} (services running)", slave_count);
        
        // Display next steps
        println!("\n📋 Next Steps:");
        println!("   • Check logs: sudo journalctl -u hainet-core -f");
        println!("   • View status: sudo systemctl status hainet-core");
        println!("\n💡 Web UI is now available!");
        println!("   Access the portal at: http://127.0.0.1:8080");
        println!("   (In dev mode, run: 'cd hainet-portal && npm run dev' on port 5173)");
        
        Ok(())
    }
    
    /// Start HAI-Net services on a remote device
    /// 
    /// Connects via SSH and starts systemd system services based on device role.
    async fn start_services_on_device<'a, F, C>(
        &'a self,
        ip: &str,
        username: &str,
        role: &DeviceRole,
        client_factory: &mut F,
    ) -> Result<()>
    where
        F: FnMut(String, SSHCredentials) -> C,
        C: SSHClientTrait + 'a,
    {
        use std::path::Path;
        
        let credentials = SSHCredentials {
            username: username.to_string(),
            password: String::new(),
        };
        
        let mut client = client_factory(ip.to_string(), credentials);
        client.connect()?;
        
        // Use SSH key authentication
        let key_path = dirs::home_dir()
            .unwrap_or_else(|| Path::new("/root").to_path_buf())
            .join(".ssh/hainet-mesh");
        
        client.authenticate_pubkey(&key_path, None)?;
        
        // Determine which services to start based on role
        let services: Vec<&str> = services_for_role(role);
        
        for service in services {
            println!("   Starting {} on {}...", service, ip);
            
            // Restart the service so new binaries are picked up during redeployment
            let start_cmd = format!("sudo -n systemctl restart {}.service 2>/dev/null", service);
            match client.execute_command(&start_cmd) {
                Ok(_) => {},
                Err(e) => {
                    println!("   ⚠️  Warning: Failed to start {}: {}", service, e);
                    continue;
                }
            }
            
            // Small delay to allow service to start
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            
            // Check if service started successfully
            let status_cmd = format!("sudo -n systemctl is-active {}.service 2>/dev/null || true", service);
            match client.execute_command(&status_cmd) {
                Ok(output) => {
                    let status = output.trim();
                    if status == "active" || status == "activating" {
                        println!("   ✓ {} started successfully", service);
                    } else {
                        println!("   ⚠️  {} may not have started (status: {})", service, status);
                    }
                },
                Err(e) => {
                    println!("   ⚠️  Could not check {} status: {}", service, e);
                }
            }
        }
        
        client.disconnect()?;
        Ok(())
    }
    
    /// Verify mesh network health by checking master node system services
    async fn verify_mesh_health<'a, F, C>(&'a self, master: &DeviceAssignment, username: &str, client_factory: &mut F) -> Result<()>
    where
        F: FnMut(String, SSHCredentials) -> C,
        C: SSHClientTrait + 'a,
    {
        use std::path::Path;
        
        let credentials = SSHCredentials {
            username: username.to_string(),
            password: String::new(),
        };
        
        let mut client = client_factory(master.ip.clone(), credentials);
        client.connect()?;
        
        let key_path = dirs::home_dir()
            .unwrap_or_else(|| Path::new("/root").to_path_buf())
            .join(".ssh/hainet-mesh");
        
        client.authenticate_pubkey(&key_path, None)?;
        
        let role_str = match master.role {
            DeviceRole::Master => "Master",
            DeviceRole::Slave => "Slave",
            DeviceRole::Standalone => "Standalone",
            DeviceRole::UIOnly => "UI Node",
        };
        
        println!("\n📊 {} Node Health Check ({}):", role_str, master.ip);
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        
        // Check hainet-core system service status
        match client.execute_command("sudo -n systemctl status hainet-core.service 2>/dev/null | head -n 3") {
            Ok(core_status) => {
                println!("🔧 hainet-core:");
                for line in core_status.lines().take(3) {
                    println!("   {}", line);
                }
            },
            Err(e) => {
                println!("⚠️  Could not check hainet-core status: {}", e);
            }
        }
        
        // Check if system configuration was loaded
        match client.execute_command("test -f /etc/hainet/hainet.toml && echo 'exists' || echo 'missing'") {
            Ok(config_check) => {
                if config_check.trim() == "exists" {
                    println!("✓ Configuration file present at /etc/hainet/hainet.toml");
                } else {
                    println!("⚠️  Configuration file missing");
                }
            },
            Err(e) => {
                println!("⚠️  Could not check configuration: {}", e);
            }
        }
        
        // Check listening ports (if hainet-core binds to 8080)
        match client.execute_command("ss -tuln | grep ':8080' || echo 'not_listening'") {
            Ok(port_check) => {
                if !port_check.contains("not_listening") {
                    println!("✓ Network port 8080 listening");
                } else {
                    println!("⚠️  Network port 8080 not yet bound (service may still be starting)");
                }
            },
            Err(_) => {
                // Ignore error, port check is informational
            }
        }
        
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        
        client.disconnect()?;
        Ok(())
    }
    
    /// Build binaries for target architecture
    fn build_binaries(&self, arch: &str) -> Result<()> {
        if std::env::var("HAINET_SKIP_BUILD").is_ok() {
            println!("✅ Skipping build because HAINET_SKIP_BUILD is set.");
            return Ok(());
        }

        // Install required dependencies before building
        self.install_build_dependencies()?;

        use std::process::Command;
        
        // Map architecture to Rust target triple
        let target = get_target_triple(arch);

        if target.is_none() {
            println!("⚠️  Unknown architecture {}, using host architecture", arch);
            return Ok(()); // Build for host architecture
        }
        let target = target.unwrap();
        
        // Determine which packages need to be built based on role assignments
        let mut packages: Vec<&str> = Vec::new();
        for assignment in &self.assignments {
            match assignment.role {
                DeviceRole::Master => {
                    for pkg in &["hainet-core", "hainet-chain", "hainet-bridge", "hainet-portal"] {
                        if !packages.contains(pkg) { packages.push(pkg); }
                    }
                }
                DeviceRole::Slave => {
                    for pkg in &["hainet-core", "hainet-chain"] {
                        if !packages.contains(pkg) { packages.push(pkg); }
                    }
                }
                DeviceRole::Standalone => {
                    for pkg in &["hainet-core", "hainet-portal"] {
                        if !packages.contains(pkg) { packages.push(pkg); }
                    }
                }
                DeviceRole::UIOnly => {
                    if !packages.contains(&"hainet-portal") { packages.push("hainet-portal"); }
                }
            }
        }

        // Find workspace root
        let workspace_root = find_workspace_root().context("Failed to find workspace root")?;
        
        // Build Vite frontend if portal is included
        if packages.contains(&"hainet-portal") {
            println!("📦 Building Vite frontend for hainet-portal...");
            let portal_dir = workspace_root.join("hainet-portal");
            let npm_install = Command::new("npm")
                .current_dir(&portal_dir)
                .args(&["install"])
                .status()
                .context("Failed to execute npm install for hainet-portal")?;
                
            if !npm_install.success() {
                anyhow::bail!("npm install failed for hainet-portal");
            }
            
            let npm_build = Command::new("npm")
                .current_dir(&portal_dir)
                .args(&["run", "build"])
                .status()
                .context("Failed to execute npm run build for hainet-portal")?;
                
            if !npm_build.success() {
                anyhow::bail!("npm run build failed for hainet-portal");
            }
            println!("✓ Vite frontend built successfully");
        }
        
        // Build each required package individually in release mode
        for package in &packages {
            println!("📦 Building {} for target: {}", package, target);
            let status = Command::new("cargo")
                .current_dir(&workspace_root)
                .args(&["build", "--release", "--target", target, "--package", package])
                .status()
                .context(format!("Failed to execute cargo build for {}", package))?;
            
            if !status.success() {
                anyhow::bail!("Build failed for {}. Aborting deployment to prevent using stale binaries.", package);
            } else {
                println!("✓ {} built successfully", package);
            }
        }
        
        Ok(())
    }

    /// Automatically install UI build dependencies based on the host OS
    fn install_build_dependencies(&self) -> Result<()> {
        use std::process::Command;

        println!("🔍 Checking and installing system dependencies for compiling HAI-Net (UI components)...");

        // Try apt-get (Debian/Ubuntu/Lubuntu/Mint)
        if Command::new("which").arg("apt-get").output().map_or(false, |o| o.status.success()) {
            println!("📦 Detected Debian/Ubuntu-based system. Installing dependencies via apt-get...");
            let mut update_cmd = Command::new("sudo");
            update_cmd.args(&["-n", "apt-get", "update"]);
            if let Ok(status) = update_cmd.status() {
                if status.success() {
                    let base_deps = ["build-essential", "cmake", "gcc", "g++", "make", "pkg-config", "libssl-dev", "protobuf-compiler"];
                    let mut install_cmd = Command::new("sudo");
                    install_cmd.args(&["-n", "apt-get", "install", "-y"]).args(&base_deps);
                    
                    if install_cmd.status().map(|s| s.success()).unwrap_or(false) {
                        println!("✓ Dependencies installed successfully.");
                        return Ok(());
                    }
                }
            }
            println!("⚠️  Could not automatically install dependencies. If build fails, run: sudo apt-get install -y build-essential cmake gcc g++ make pkg-config libssl-dev protobuf-compiler");
            return Ok(());
        }
        
        // Try dnf (Fedora/RHEL)
        if Command::new("which").arg("dnf").output().map_or(false, |o| o.status.success()) {
            println!("📦 Detected Fedora/RHEL-based system. Installing dependencies via dnf...");
            let deps = ["cmake", "gcc", "gcc-c++", "make", "libsoup-devel", "webkit2gtk3-devel", "gtk3-devel", "openssl-devel", "pkgconf-pkg-config"];
            let mut install_cmd = Command::new("sudo");
            install_cmd.args(&["-n", "dnf", "install", "-y"]).args(&deps);
            
            if install_cmd.status().map(|s| s.success()).unwrap_or(false) {
                println!("✓ Dependencies installed successfully.");
                return Ok(());
            }
            println!("⚠️  Could not automatically install dependencies via dnf. Proceeding anyway.");
            return Ok(());
        }
        
        // Try pacman (Arch/Manjaro)
        if Command::new("which").arg("pacman").output().map_or(false, |o| o.status.success()) {
            println!("📦 Detected Arch-based system. Installing dependencies via pacman...");
            let deps = ["cmake", "gcc", "make", "libsoup", "webkit2gtk", "gtk3", "openssl", "pkgconf"];
            let mut install_cmd = Command::new("sudo");
            install_cmd.args(&["-n", "pacman", "-S", "--noconfirm", "--needed"]).args(&deps);
            
            if install_cmd.status().map(|s| s.success()).unwrap_or(false) {
                println!("✓ Dependencies installed successfully.");
                return Ok(());
            }
            println!("⚠️  Could not automatically install dependencies via pacman. Proceeding anyway.");
            return Ok(());
        }

        println!("⚠️  Could not detect package manager. Please ensure libsoup, webkit2gtk, and gtk3 development headers are installed.");
        Ok(())
    }

    /// Transfer binaries to remote device based on role
    #[cfg(not(test))]
    fn transfer_binaries<C: SSHClientTrait>(&self, client: &C, role: &DeviceRole, arch: &str) -> Result<()> {
        // Determine which binaries to transfer based on role
        let binaries = match role {
            DeviceRole::Master => vec![
                "hainet-core",
                "hainet-chain",
                "hainet-bridge",
                "hainet-portal",
            ],
            DeviceRole::Slave => vec![
                "hainet-core",
                "hainet-chain",
            ],
            DeviceRole::Standalone => vec![
                "hainet-core",
                "hainet-chain",
                "hainet-bridge",
                "hainet-portal",
            ],
            DeviceRole::UIOnly => vec![
                "hainet-portal",
            ],
        };
        
        // Get target directory
        let workspace_root = find_workspace_root().context("Failed to find workspace root for transfer")?;
        let target_triple = get_target_triple(arch).unwrap_or("release");
        let target_dir = workspace_root.join("target").join(target_triple).join("release");
        
        for binary_name in binaries {
            let local_path = target_dir.join(binary_name);
            println!("[TRANSFER] Looking for binary at: {:?}", local_path);
            
            if !local_path.exists() {
                println!("⚠️  Binary {} not found, skipping", binary_name);
                continue;
            }
            
            // Upload to user-writable temp location
            let temp_path = format!("/tmp/hainet-upload-{}", binary_name);
            
            println!("   Uploading binary (this may take a moment)...");
            client.upload_file(&local_path, &temp_path)?;
            
            // Try to move to /usr/local/bin with sudo -n, fall back to ~/bin
            let move_cmd = format!(
                "sudo -n mv {} /usr/local/bin/{} 2>/dev/null && sudo -n chmod +x /usr/local/bin/{} 2>/dev/null && sudo -n chown root:root /usr/local/bin/{} 2>/dev/null || (mkdir -p ~/bin && mv {} ~/bin/{} && chmod +x ~/bin/{})",
                temp_path, binary_name, binary_name, binary_name, temp_path, binary_name, binary_name
            );
            let _ = client.execute_command(&move_cmd);
            
            println!("✓ Installed {}", binary_name);
        }
        
        Ok(())
    }

    #[cfg(test)]
    fn transfer_binaries<C: SSHClientTrait>(&self, _client: &C, _role: &DeviceRole, _arch: &str) -> Result<()> {
        // No-op for tests
        Ok(())
    }
    
    /// Transfer the AI Persona prompts directory to the remote device
    #[cfg(not(test))]
    fn transfer_prompts<C: SSHClientTrait>(&self, client: &C, dest_base_path: &str) -> Result<()> {
        let workspace_root = find_workspace_root().context("Failed to find workspace root for transfer")?;
        let prompts_dir = workspace_root.join("hainet-persona").join("prompts");
        let te_prompts_file = workspace_root.join("_workspace").join("TrippleEffect").join("prompts.yaml");
        
        if !prompts_dir.exists() {
            println!("⚠️  Prompts directory not found at {:?}, skipping", prompts_dir);
            return Ok(());
        }
        
        // Tar the prompts directory locally
        let tar_path = "/tmp/hainet-prompts.tar.gz";
        let _ = std::process::Command::new("tar")
            .args(&["-czf", tar_path, "-C", prompts_dir.parent().unwrap().to_str().unwrap(), "prompts"])
            .status()?;
            
        let temp_remote_path = "/tmp/hainet-prompts.tar.gz";
        println!("   Uploading prompts to shared drive...");
        client.upload_file(std::path::Path::new(tar_path), temp_remote_path)?;
        
        // Extract on remote and set ownership
        let dest_prompts_path = format!("{}/prompts", dest_base_path);
        let extract_cmd = format!(
            "sudo -n mkdir -p {} && sudo -n tar -xzf {} -C {} && sudo -n chown -R hainet:hainet {} && rm {}",
            dest_base_path, temp_remote_path, dest_base_path, dest_prompts_path, temp_remote_path
        );
        let _ = client.execute_command(&extract_cmd);
        
        // Also upload TrippleEffect prompts.yaml
        if te_prompts_file.exists() {
            let temp_te_path = "/tmp/te_prompts.yaml";
            client.upload_file(&te_prompts_file, temp_te_path)?;
            let _ = client.execute_command(&format!(
                "cat {} | sudo -n tee {}/prompts.yaml > /dev/null && sudo -n chown hainet:hainet {}/prompts.yaml && rm {}",
                temp_te_path, dest_prompts_path, dest_prompts_path, temp_te_path
            ));
        } else {
            println!("⚠️  TrippleEffect prompts.yaml not found at {:?}, skipping", te_prompts_file);
        }
        
        // Cleanup local tar
        let _ = std::fs::remove_file(tar_path);
        
        println!("✓ Installed AI persona prompts");
        Ok(())
    }

    /// Transfer system configs to the remote device
    #[cfg(not(test))]
    fn transfer_configs<C: SSHClientTrait>(&self, client: &C) -> Result<()> {
        let workspace_root = find_workspace_root().context("Failed to find workspace root for config transfer")?;
        let persona_dir = workspace_root.join("hainet-persona");
        
        // Ensure destination dir exists
        client.execute_command("sudo -n mkdir -p /var/lib/hainet/.hainet")?;
        
        for config_file in &["mcp-servers.toml", "ollama-endpoints.toml"] {
            let src = persona_dir.join(config_file);
            if src.exists() {
                let remote_tmp = format!("/tmp/{}", config_file);
                client.upload_file(&src, &remote_tmp)?;
                client.execute_command(&format!(
                    "cat {} | sudo -n tee /var/lib/hainet/.hainet/{} > /dev/null && rm {}", remote_tmp, config_file, remote_tmp
                ))?;
            } else {
                println!("⚠️  Config file not found at {:?}, skipping", src);
            }
        }
        
        let _ = client.execute_command("sudo -n chown -R hainet:hainet /var/lib/hainet/.hainet 2>/dev/null || true");
        
        println!("✓ Installed system configs");
        Ok(())
    }

    #[cfg(test)]
    fn transfer_prompts<C: SSHClientTrait>(&self, _client: &C, _dest_base_path: &str) -> Result<()> {
        Ok(())
    }

    #[cfg(test)]
    fn transfer_configs<C: SSHClientTrait>(&self, _client: &C) -> Result<()> {
        Ok(())
    }

    /// Copy the AI Persona prompts directory to the local device
    fn copy_prompts_local(&self, dest_base_path: &str) -> Result<()> {
        let workspace_root = find_workspace_root().context("Failed to find workspace root for transfer")?;
        let prompts_dir = workspace_root.join("hainet-persona").join("prompts");
        let te_prompts_file = workspace_root.join("_workspace").join("TrippleEffect").join("prompts.yaml");
        
        if !prompts_dir.exists() {
            println!("⚠️  Prompts directory not found at {:?}, skipping", prompts_dir);
            return Ok(());
        }
        
        std::process::Command::new("sudo")
            .args(&["mkdir", "-p", dest_base_path])
            .status()?;
            
        std::process::Command::new("sudo")
            .args(&["cp", "-r", prompts_dir.to_str().unwrap(), dest_base_path])
            .status()?;
            
        let dest_prompts_path = format!("{}/prompts", dest_base_path);
        
        // Also copy TrippleEffect prompts.yaml
        if te_prompts_file.exists() {
            std::process::Command::new("sudo")
                .args(&["cp", te_prompts_file.to_str().unwrap(), &format!("{}/prompts.yaml", dest_prompts_path)])
                .status()?;
        } else {
            println!("⚠️  TrippleEffect prompts.yaml not found at {:?}, skipping", te_prompts_file);
        }
        
        std::process::Command::new("sudo")
            .args(&["chown", "-R", "hainet:hainet", &dest_prompts_path])
            .status()?;
            
        println!("✓ Installed AI persona prompts to {}", dest_prompts_path);
        Ok(())
    }
    
    /// Copy system configs to the local device
    fn copy_configs_local(&self) -> Result<()> {
        let workspace_root = find_workspace_root().context("Failed to find workspace root for config transfer")?;
        let persona_dir = workspace_root.join("hainet-persona");
        
        std::process::Command::new("sudo")
            .args(&["mkdir", "-p", "/var/lib/hainet/.hainet"])
            .status()?;
            
        for config_file in &["mcp-servers.toml", "ollama-endpoints.toml"] {
            let src = persona_dir.join(config_file);
            if src.exists() {
                std::process::Command::new("sudo")
                    .args(&["cp", src.to_str().unwrap(), "/var/lib/hainet/.hainet/"])
                    .status()?;
            } else {
                println!("⚠️  Config file not found at {:?}, skipping", src);
            }
        }
        
        std::process::Command::new("sudo")
            .args(&["chown", "-R", "hainet:hainet", "/var/lib/hainet/.hainet"])
            .status()?;
            
        println!("✓ Installed system configs to /var/lib/hainet/.hainet/");
        Ok(())
    }
    
    /// Configure device with role-specific settings
    fn configure_device<C: SSHClientTrait>(&self, client: &C, assignment: &DeviceAssignment) -> Result<()> {
        // Determine the correct log directory.
        // If this device IS the NFS host, use the actual export path (e.g. /media/fast/hai-drive/logs)
        // rather than the mount-point path (which would create a local-only directory).
        let is_nfs_host = self.shared_drive_host_ip.as_deref() == Some(&assignment.ip);
        let base = if is_nfs_host {
            // Use the real export path on the NFS host machine
            self.shared_drive_remote_path.as_deref().unwrap_or(&self.shared_drive_path)
        } else {
            &self.shared_drive_path
        };
        let log_dir = format!("{}/logs", base);
        let data_dir = format!("{}/data", base);

        // Create hainet.toml configuration with system directories
        let config = match assignment.role {
            DeviceRole::Master => {
                format!("[network]\nrole = \"master\"\nport = 8080\n\n[storage]\ndata_dir = \"{}\"\n\n[logs]\nlog_dir = \"{}\"\nlog_level = \"info\"\n", data_dir, log_dir)
            },
            DeviceRole::Slave => {
                // Get master IP (first Master in assignments)
                let master_ip = self.master_node()
                    .map(|m| m.ip.as_str())
                    .unwrap_or("10.0.0.10");
                
                format!(
                    "[network]\nrole = \"slave\"\nmaster_ip = \"{}\"\nport = 8080\n\n[storage]\ndata_dir = \"{}\"\n\n[logs]\nlog_dir = \"{}\"\nlog_level = \"info\"\n",
                    master_ip, data_dir, log_dir
                )
            },
            DeviceRole::Standalone => {
                format!("[network]\nrole = \"standalone\"\nport = 8080\n\n[storage]\ndata_dir = \"{}\"\n\n[logs]\nlog_dir = \"{}\"\nlog_level = \"info\"\n", data_dir, log_dir)
            },
            DeviceRole::UIOnly => {
                let master_ip = self.master_node()
                    .map(|m| m.ip.as_str())
                    .unwrap_or("10.0.0.10");
                
                format!(
                    "[network]\nrole = \"ui-only\"\nmaster_ip = \"{}\"\nport = 3000\n\n[logs]\nlog_dir = \"{}\"\nlog_level = \"info\"\n",
                    master_ip, log_dir
                )
            },
        };
        
        // Write config to temp file, then move to system directory
        let temp_path = "/tmp/hainet.toml";
        let command = format!("cat > {} << 'EOF'\n{}EOF", temp_path, config);
        client.execute_command(&command)?;
        
        let _ = client.execute_command(&format!("sudo -n mv {} /etc/hainet/hainet.toml 2>/dev/null || cp {} ~/hainet/config/hainet.toml 2>/dev/null || true", temp_path, temp_path));
        let _ = client.execute_command("sudo -n chown hainet:hainet /etc/hainet/hainet.toml 2>/dev/null || true");
        
        // Ensure log directory exists and is writable by hainet
        let _ = client.execute_command(&format!("sudo -n mkdir -p {} 2>/dev/null || mkdir -p {}", log_dir, log_dir));
        let _ = client.execute_command(&format!("sudo -n chown -R hainet:hainet {} 2>/dev/null || true", log_dir));
        
        // Ensure data directory exists and is writable by hainet
        let _ = client.execute_command(&format!("sudo -n mkdir -p {} 2>/dev/null || mkdir -p {}", data_dir, data_dir));
        let _ = client.execute_command(&format!("sudo -n chown -R hainet:hainet {} 2>/dev/null || true", data_dir));
        
        println!("✓ Configuration written to /etc/hainet/hainet.toml");
        
        Ok(())
    }
    
    /// Mount the NFS shared drive on a remote node
    ///
    /// Uses the shared_drive_host_ip and shared_drive_remote_path to construct
    /// the correct NFS source, and shared_drive_path as the local mount point.
    /// Example: mount -t nfs 192.168.0.22:/media/fast/NoSlop /media/hai-drive
    fn mount_shared_drive_on_remote<C: SSHClientTrait>(&self, client: &C, master_ip: &str) -> Result<()> {
        let local_mount = &self.shared_drive_path;
        // The NFS host is the device that exports the share (may differ from master)
        let nfs_host = self.shared_drive_host_ip.as_deref().unwrap_or(master_ip);
        // The remote export path may differ from the local mount point
        let remote_export = self.shared_drive_remote_path.as_deref().unwrap_or(local_mount);
        
        info!("🌐 Mounting NFS share {}:{} → {}", nfs_host, remote_export, local_mount);
        
        // Install nfs-common only if mount.nfs is missing
        if client.execute_command("which mount.nfs >/dev/null 2>&1").is_err() {
            if let Err(e) = client.execute_command("sudo -n apt-get update -qq && sudo -n DEBIAN_FRONTEND=noninteractive apt-get install -y nfs-common") {
                warn!("⚠️  Failed to install nfs-common on remote (requires passwordless sudo): {}", e);
            }
        }
        
        // Create local mount point
        client.execute_command(&format!("sudo -n mkdir -p {}", local_mount))?;
        
        // Check if the *exact* correct share is mounted
        let expected_source = format!("{}:{}", nfs_host, remote_export);
        let check_correct_mount = client.execute_command(&format!("mount | grep -q '^{} on {} '", expected_source, local_mount));
        
        if check_correct_mount.is_err() {
            // The correct share is NOT mounted. 
            // Is *anything* mounted there? If so, it's a mis-mount and we must unmount it.
            let check_any_mount = client.execute_command(&format!("mount | grep -q ' on {} '", local_mount));
            if check_any_mount.is_ok() {
                warn!("⚠️  Incorrect filesystem mounted at {}. Forcefully unmounting...", local_mount);
                let _ = client.execute_command(&format!("sudo -n umount -f {} 2>/dev/null || true", local_mount));
            }
        
            // Check if already in fstab (use the remote export path as source)
            let fstab_entry = format!("{} {} nfs _netdev,x-systemd.automount,noauto,retry=10000,timeo=14 0 0", expected_source, local_mount);
            let check_fstab = client.execute_command(&format!("grep -q '^{}' /etc/fstab", expected_source));
            
            if check_fstab.is_err() {
                let add_fstab = format!("echo '{}' | sudo -n tee -a /etc/fstab", fstab_entry);
                client.execute_command(&add_fstab)?;
            }
            
            // Mount using the NFS source and local destination
            let mount_cmd = format!("sudo -n mount -t nfs {} {}", expected_source, local_mount);
            if let Err(e) = client.execute_command(&mount_cmd) {
                warn!("⚠️  Failed to mount {} → {}: {}", expected_source, local_mount, e);
                warn!("   You can try manually: sudo mount -t nfs {} {}", expected_source, local_mount);
            } else {
                println!("✓ Shared drive mounted: {} → {}", expected_source, local_mount);
            }
        } else {
            println!("✓ Shared drive already mounted at {} from {}.", local_mount, expected_source);
        }
        
        Ok(())
    }

    /// Set up systemd system services for the device role
    fn setup_services<C: SSHClientTrait>(&self, client: &C, role: &DeviceRole) -> Result<()> {
        let services: Vec<&str> = services_for_role(role);
        
        for service_name in services {
            // Create systemd system service file
            let service_content = format!(
                "[Unit]\n\
                 Description=HAI-Net {}\n\
                 After=network.target\n\n\
                 [Service]\n\
                 Type=simple\n\
                 User=hainet\n\
                 Group=hainet\n\
                 ExecStart=/usr/local/bin/{}\n\
                 Restart=always\n\
                 RestartSec=10\n\
                 Environment=RUST_LOG=info\n\
                 WorkingDirectory=/var/lib/hainet\n\
                 StandardOutput=journal\n\
                 StandardError=journal\n\n\
                 [Install]\n\
                 WantedBy=multi-user.target\n",
                service_name, service_name
            );
            
            // Write service file to temp, then move to system directory
            let temp_path = format!("/tmp/{}.service", service_name);
            let command = format!("cat > {} << 'EOF'\n{}EOF", temp_path, service_content);
            client.execute_command(&command)?;
            
            let _ = client.execute_command(&format!("sudo -n mv {} /etc/systemd/system/{}.service 2>/dev/null || true", temp_path, service_name));
            
            // Enable system service
            let _ = client.execute_command(&format!("sudo -n systemctl enable {}.service 2>/dev/null || true", service_name));
            
            println!("✓ System service {} configured and enabled", service_name);
        }
        
        // Reload systemd daemon
        let _ = client.execute_command("sudo -n systemctl daemon-reload 2>/dev/null || true");
        
        Ok(())
    }
    
    /// Generate deployment summary
    pub fn summary(&self) -> DeploymentSummary {
        let role_counts = self.assignments.iter().fold(
            HashMap::new(),
            |mut acc, assignment| {
                *acc.entry(assignment.role.clone()).or_insert(0) += 1;
                acc
            }
        );
        
        DeploymentSummary {
            total_devices: self.assignments.len(),
            master_count: *role_counts.get(&DeviceRole::Master).unwrap_or(&0),
            slave_count: *role_counts.get(&DeviceRole::Slave).unwrap_or(&0),
            standalone_count: *role_counts.get(&DeviceRole::Standalone).unwrap_or(&0),
        }
    }
}

impl Default for DeploymentOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

/// Deployment summary statistics
#[derive(Debug, Clone)]
pub struct DeploymentSummary {
    pub total_devices: usize,
    pub master_count: usize,
    pub slave_count: usize,
    pub standalone_count: usize,
}

/// Find workspace root from current directory
pub fn find_workspace_root() -> Result<PathBuf> {
    let output = std::process::Command::new("cargo")
        .arg("locate-project")
        .arg("--workspace")
        .arg("--message-format=plain")
        .output()
        .context("Failed to run `cargo locate-project`")?;

    if !output.status.success() {
        bail!("`cargo locate-project` failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let root_path = PathBuf::from(stdout.trim()).parent().unwrap().to_path_buf();
    Ok(root_path)
}

/// Map architecture to Rust target triple
fn get_target_triple(arch: &str) -> Option<&'static str> {
    match arch {
        "x86_64" => Some("x86_64-unknown-linux-gnu"),
        "aarch64" => Some("aarch64-unknown-linux-gnu"),
        "armv7l" => Some("armv7-unknown-linux-gnueabihf"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn create_mock_capabilities(count: usize) -> Vec<DeviceCapabilities> {
        (0..count).map(|i| {
            let mut caps = DeviceCapabilities {
                ip: format!("192.168.1.{}", 10 + i),
                hostname: format!("device-{}", i),
                cpu_cores: 4 + i,
                ram_gb: 8.0 + (i as f64 * 4.0),
                gpu: if i == 0 { Some("NVIDIA RTX3060".to_string()) } else { None },
                disk_gb: 100.0 + (i as f64 * 50.0),
                os: "Linux".to_string(),
                arch: "x86_64".to_string(),
                services: vec![],
                score: 0.0,
            };
            caps.calculate_score();
            caps
        }).collect()
    }
    
    #[test]
    fn test_role_assignment_single_device() {
        let mut orchestrator = DeploymentOrchestrator::new();
        let capabilities = create_mock_capabilities(1);
        
        let result = orchestrator.assign_roles(capabilities);
        assert!(result.is_ok());
        
        assert_eq!(orchestrator.assignments().len(), 1);
        assert_eq!(orchestrator.assignments()[0].role, DeviceRole::Standalone);
    }
    
    #[test]
    fn test_role_assignment_two_devices() {
        let mut orchestrator = DeploymentOrchestrator::new();
        let capabilities = create_mock_capabilities(2);
        
        let result = orchestrator.assign_roles(capabilities);
        assert!(result.is_ok());
        
        assert_eq!(orchestrator.assignments().len(), 2);
        
        // First device (highest score) should be master
        assert_eq!(orchestrator.assignments()[0].role, DeviceRole::Master);
        // Second device should be slave
        assert_eq!(orchestrator.assignments()[1].role, DeviceRole::Slave);
    }
    
    #[test]
    fn test_role_assignment_multiple_devices() {
        let mut orchestrator = DeploymentOrchestrator::new();
        let capabilities = create_mock_capabilities(5);
        
        let result = orchestrator.assign_roles(capabilities);
        assert!(result.is_ok());
        
        assert_eq!(orchestrator.assignments().len(), 5);
        
        // Check master node
        let master = orchestrator.master_node();
        assert!(master.is_some());
        assert_eq!(master.unwrap().role, DeviceRole::Master);
        
        // Check slave nodes
        let slaves = orchestrator.slave_nodes();
        assert_eq!(slaves.len(), 4);
    }
    
    #[test]
    fn test_deployment_summary() {
        let mut orchestrator = DeploymentOrchestrator::new();
        let capabilities = create_mock_capabilities(3);
        
        orchestrator.assign_roles(capabilities).unwrap();
        
        let summary = orchestrator.summary();
        assert_eq!(summary.total_devices, 3);
        assert_eq!(summary.master_count, 1);
        assert_eq!(summary.slave_count, 2);
        assert_eq!(summary.standalone_count, 0);
    }
    
    #[test]
    fn test_master_has_highest_score() {
        let mut orchestrator = DeploymentOrchestrator::new();
        let capabilities = create_mock_capabilities(4);
        
        orchestrator.assign_roles(capabilities.clone()).unwrap();
        
        let master = orchestrator.master_node().unwrap();
        let max_score = capabilities.iter().map(|c| c.score).fold(0.0, f64::max);
        
        assert!((master.capabilities.score - max_score).abs() < 0.01);
    }
    
    #[test]
    fn test_mobile_device_detection() {
        let mut orchestrator = DeploymentOrchestrator::new();
        
        // Create mobile device (< 2GB RAM)
        let mut mobile = DeviceCapabilities {
            ip: "192.168.1.50".to_string(),
            hostname: "phone".to_string(),
            cpu_cores: 4,
            ram_gb: 1.5, // Mobile device
            gpu: None,
            disk_gb: 64.0,
            os: "Linux".to_string(),
            arch: "aarch64".to_string(),
            services: vec![],
            score: 0.0,
        };
        mobile.calculate_score();
        
        // Create desktop device
        let mut desktop = DeviceCapabilities {
            ip: "192.168.1.10".to_string(),
            hostname: "desktop".to_string(),
            cpu_cores: 8,
            ram_gb: 16.0,
            gpu: Some("NVIDIA RTX3060".to_string()),
            disk_gb: 500.0,
            os: "Linux".to_string(),
            arch: "x86_64".to_string(),
            services: vec![],
            score: 0.0,
        };
        desktop.calculate_score();
        
        orchestrator.assign_roles(vec![mobile, desktop]).unwrap();
        
        // Mobile device should be UIOnly
        let ui_only_device = orchestrator.assignments().iter()
            .find(|a| a.hostname == "phone")
            .unwrap();
        assert_eq!(ui_only_device.role, DeviceRole::UIOnly);
        
        // Desktop should be Master (only compute device)
        assert_eq!(orchestrator.master_node().unwrap().hostname, "desktop");
    }
    
    #[test]
    fn test_single_mobile_device() {
        let mut orchestrator = DeploymentOrchestrator::new();
        
        let mut mobile = DeviceCapabilities {
            ip: "192.168.1.50".to_string(),
            hostname: "phone".to_string(),
            cpu_cores: 4,
            ram_gb: 1.5,
            gpu: None,
            disk_gb: 64.0,
            os: "Linux".to_string(),
            arch: "aarch64".to_string(),
            services: vec![],
            score: 0.0,
        };
        mobile.calculate_score();
        
        orchestrator.assign_roles(vec![mobile]).unwrap();
        
        assert_eq!(orchestrator.assignments().len(), 1);
        assert_eq!(orchestrator.assignments()[0].role, DeviceRole::UIOnly);
    }
    
    #[test]
    fn test_mixed_devices_with_mobile() {
        let mut orchestrator = DeploymentOrchestrator::new();
        
        // Create 2 mobile devices
        let mut phone1 = DeviceCapabilities {
            ip: "192.168.1.50".to_string(),
            hostname: "phone1".to_string(),
            cpu_cores: 4,
            ram_gb: 1.5,
            gpu: None,
            disk_gb: 64.0,
            os: "Linux".to_string(),
            arch: "aarch64".to_string(),
            services: vec![],
            score: 0.0,
        };
        phone1.calculate_score();
        
        let mut phone2 = DeviceCapabilities {
            ip: "192.168.1.51".to_string(),
            hostname: "phone2".to_string(),
            cpu_cores: 4,
            ram_gb: 1.8,
            gpu: None,
            disk_gb: 32.0,
            os: "Linux".to_string(),
            arch: "aarch64".to_string(),
            services: vec![],
            score: 0.0,
        };
        phone2.calculate_score();
        
        // Create 3 desktop devices
        let desktops = create_mock_capabilities(3);
        
        let mut all_devices = vec![phone1, phone2];
        all_devices.extend(desktops);
        
        orchestrator.assign_roles(all_devices).unwrap();
        
        // Should have 2 UIOnly, 1 Master, 2 Slaves
        assert_eq!(orchestrator.assignments().len(), 5);
        
        let ui_only_count = orchestrator.assignments().iter()
            .filter(|a| a.role == DeviceRole::UIOnly)
            .count();
        assert_eq!(ui_only_count, 2);
        
        assert!(orchestrator.master_node().is_some());
        assert_eq!(orchestrator.slave_nodes().len(), 2);
    }
}

// Helper methods for DeploymentOrchestrator (outside of tests module)
impl DeploymentOrchestrator {
    /// Create hainet system user on remote device and align UID/GID to 995
    fn create_system_user<C: SSHClientTrait>(&self, client: &C) -> Result<()> {
        // Try creating group with GID 995
        let _ = client.execute_command("sudo -n groupadd -g 995 hainet 2>/dev/null || sudo -n groupadd hainet 2>/dev/null || true");
        // Try creating user with UID 995 and group GID 995
        let _ = client.execute_command("sudo -n useradd -r -u 995 -g hainet -s /bin/false -d /var/lib/hainet -m hainet 2>/dev/null || true");
        
        // If user already exists but has a different UID, align it to 995
        let _ = client.execute_command("sudo -n usermod -u 995 hainet 2>/dev/null || true");
        let _ = client.execute_command("sudo -n groupmod -g 995 hainet 2>/dev/null || true");
        
        // Ensure standard system directories are owned by aligned user
        let _ = client.execute_command("sudo -n chown -R hainet:hainet /var/lib/hainet /var/log/hainet /etc/hainet 2>/dev/null || true");

        println!("✓ System user 'hainet' created (aligned to UID 995)");
        Ok(())
    }
    
    /// Create system directories on remote device
    fn create_system_directories<C: SSHClientTrait>(&self, client: &C) -> Result<()> {
        // Combine all mkdir operations into a single command to minimize sudo calls
        // Use sudo -n (non-interactive) and fall back gracefully
        let mkdir_cmd = "sudo -n mkdir -p /usr/local/bin /etc/hainet /var/lib/hainet /var/lib/hainet/data /var/log/hainet 2>/dev/null || mkdir -p ~/hainet ~/hainet/data ~/hainet/logs ~/hainet/config";
        let _ = client.execute_command(mkdir_cmd);
        
        // Try setting ownership (non-fatal if it fails)
        let _ = client.execute_command("sudo -n chown -R hainet:hainet /etc/hainet /var/lib/hainet /var/log/hainet 2>/dev/null || true");
        
        println!("✓ System directories created");
        Ok(())
    }
}
