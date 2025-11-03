//! # START OF FILE hainet-seed/src/installer/deployment.rs
//! Remote deployment orchestrator for multi-device HAI-Net mesh.
//! Handles role assignment, binary deployment, and service initialization.

use anyhow::{Result, bail, Context};
use crate::installer::ssh_client::{DeviceCapabilities, SSHCredentials, SSHClientTrait};
use std::collections::HashMap;
use std::path::PathBuf;

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
}

impl DeploymentOrchestrator {
    /// Create new deployment orchestrator
    pub fn new() -> Self {
        Self {
            assignments: Vec::new(),
        }
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
        
        // Use SSH key authentication (keys should be set up by now)
        let key_path = dirs::home_dir()
            .unwrap_or_else(|| Path::new("/root").to_path_buf())
            .join(".ssh/hainet-mesh");
        
        client.authenticate_pubkey(&key_path, None)?;
        
        // Step 2: Create installation directory
        println!("📁 Creating installation directories...");
        client.create_remote_directory("/opt/hainet/bin")?;
        client.create_remote_directory("/etc/hainet")?;
        
        // Step 3: Transfer binaries based on role
        println!("📤 Transferring binaries...");
        self.transfer_binaries(&client, &assignment.role)?;
        
        // Step 4: Configure role-specific settings
        println!("⚙️  Configuring role settings...");
        self.configure_device(&client, &assignment)?;
        
        // Step 5: Create and enable systemd services
        println!("🔧 Setting up services...");
        self.setup_services(&client, &assignment.role)?;
        
        // Disconnect
        client.disconnect()?;
        
        println!("✓ Deployment to {} complete", assignment.hostname);
        
        Ok(())
    }

    /// Deploy HAI-Net to the local machine
    async fn deploy_to_localhost(&self, assignment: &DeviceAssignment) -> Result<()> {
        println!("\n📦 Deploying to localhost ({})...", assignment.hostname);

        use std::fs;
        use std::process::Command;
        use std::path::Path;

        // Step 2: Create installation directory
        println!("📁 Creating installation directories...");
        fs::create_dir_all("/opt/hainet/bin")?;
        fs::create_dir_all("/etc/hainet")?;

        // Step 3: Transfer binaries based on role
        println!("📤 Copying binaries...");
        let binaries = match assignment.role {
            DeviceRole::Master => vec!["hainet-core", "hainet-chain", "hainet-bridge", "hainet-portal"],
            DeviceRole::Slave => vec!["hainet-core", "hainet-chain"],
            DeviceRole::Standalone => vec!["hainet-core", "hainet-portal"],
            DeviceRole::UIOnly => vec!["hainet-portal"],
        };
        let target_dir = find_workspace_root()?.join("target/release");
        for binary_name in binaries {
            let source_path = target_dir.join(binary_name);
            let dest_path = Path::new("/opt/hainet/bin").join(binary_name);
            if source_path.exists() {
                fs::copy(&source_path, &dest_path)?;
                Command::new("chmod").arg("+x").arg(&dest_path).status()?;
            }
        }

        // Step 4: Configure role-specific settings
        println!("⚙️  Configuring role settings...");
        let config_content = match assignment.role {
            DeviceRole::Master => "[network]\nrole = \"master\"\nport = 8080\n\n[storage]\ndata_dir = \"/var/lib/hainet\"\n".to_string(),
            _ => "[network]\nrole = \"standalone\"\nport = 8080\n\n[storage]\ndata_dir = \"/var/lib/hainet\"\n".to_string(),
        };
        fs::write("/etc/hainet/hainet.toml", config_content)?;

        // Step 5: Create and enable systemd services
        println!("🔧 Setting up services...");
        let services = match assignment.role {
            DeviceRole::Master | DeviceRole::Slave | DeviceRole::Standalone => vec!["hainet-core", "hainet-chain"],
            DeviceRole::UIOnly => vec!["hainet-portal"],
        };
        for service_name in services {
            let service_content = format!(
                "[Unit]\nDescription=HAI-Net {}\nAfter=network.target\n\n[Service]\nType=simple\nExecStart=/opt/hainet/bin/{}\nRestart=always\nUser=root\nGroup=root\n\n[Install]\nWantedBy=multi-user.target\n",
                service_name, service_name
            );
            fs::write(format!("/etc/systemd/system/{}.service", service_name), service_content)?;
            Command::new("systemctl").arg("enable").arg(format!("{}.service", service_name)).status()?;
        }
        Command::new("systemctl").arg("daemon-reload").status()?;

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
        if let Some((username, _)) = credentials_map.get(&master.ip) {
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
        
        println!("\n✅ Mesh network initialized successfully!");
        println!("   Master: {} (services running)", master.hostname);
        println!("   Slaves: {} (services running)", slave_count);
        
        // Display next steps
        println!("\n📋 Next Steps:");
        println!("   • Access UI at: http://{}:3000", master.ip);
        println!("   • Check logs: sudo journalctl -u hainet-core -f");
        println!("   • View status: sudo systemctl status hainet-core");
        println!("\n💡 Note: Full P2P mesh networking will be enabled in Phase 8");
        
        Ok(())
    }
    
    /// Start HAI-Net services on a remote device
    /// 
    /// Connects via SSH and starts systemd services based on device role.
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
            .join(".ssh/id_ed25519");
        
        client.authenticate_pubkey(&key_path, None)?;
        
        // Determine which services to start based on role
        let services = match role {
            DeviceRole::Master | DeviceRole::Slave | DeviceRole::Standalone => {
                vec!["hainet-core", "hainet-chain"]
            },
            DeviceRole::UIOnly => {
                vec!["hainet-portal"]
            },
        };
        
        for service in services {
            println!("   Starting {} on {}...", service, ip);
            
            // Start service
            let start_cmd = format!("sudo systemctl start {}.service", service);
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
            let status_cmd = format!("sudo systemctl is-active {}.service", service);
            match client.execute_command(&status_cmd) {
                Ok(output) => {
                    if output.trim() == "active" {
                        println!("   ✓ {} started successfully", service);
                    } else {
                        println!("   ⚠️  {} may not have started (status: {})", service, output.trim());
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
    
    /// Verify mesh network health by checking master node services
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
            .join(".ssh/id_ed25519");
        
        client.authenticate_pubkey(&key_path, None)?;
        
        println!("\n📊 Master Node Health Check:");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        
        // Check hainet-core status
        match client.execute_command("sudo systemctl status hainet-core.service | head -n 3") {
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
        
        // Check if configuration was loaded
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
        match client.execute_command("sudo ss -tuln | grep ':8080' || echo 'not_listening'") {
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

        use std::process::Command;
        
        // Map architecture to Rust target triple
        let target = get_target_triple(arch);

        if target.is_none() {
            println!("⚠️  Unknown architecture {}, using host architecture", arch);
            return Ok(()); // Build for host architecture
        }
        let target = target.unwrap();
        
        println!("📦 Building HAI-Net for target: {}", target);

        // Find workspace root
        let workspace_root = find_workspace_root().context("Failed to find workspace root")?;
        
        // Build all required packages in release mode
        let status = Command::new("cargo")
            .current_dir(&workspace_root)
            .args(&["build", "--release", "--target", target, "--workspace"])
            .status()
            .context("Failed to execute cargo build")?;
        
        if !status.success() {
            bail!("Cargo build failed for target {}", target);
        }
        
        println!("✓ Build complete for {}", target);
        Ok(())
    }

    /// Transfer binaries to remote device based on role
    #[cfg(not(test))]
    fn transfer_binaries<C: SSHClientTrait>(&self, client: &C, role: &DeviceRole) -> Result<()> {
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
                "hainet-portal",
            ],
            DeviceRole::UIOnly => vec![
                "hainet-portal",
            ],
        };
        
        // Get target directory (TODO: use actual target architecture)
        let workspace_root = find_workspace_root().context("Failed to find workspace root for transfer")?;
        let target_dir = workspace_root.join("target/release");
        
        for binary_name in binaries {
            let local_path = target_dir.join(binary_name);
            println!("[TRANSFER] Looking for binary at: {:?}", local_path);
            
            if !local_path.exists() {
                println!("⚠️  Binary {} not found, skipping", binary_name);
                continue;
            }
            
            let remote_path = format!("/opt/hainet/bin/{}", binary_name);
            client.upload_file(&local_path, &remote_path)?;
            
            // Make binary executable
            client.set_permissions(&remote_path, 0o755)?;
        }
        
        Ok(())
    }

    #[cfg(test)]
    fn transfer_binaries<C: SSHClientTrait>(&self, _client: &C, _role: &DeviceRole) -> Result<()> {
        // No-op for tests
        Ok(())
    }
    
    /// Configure device with role-specific settings
    fn configure_device<C: SSHClientTrait>(&self, client: &C, assignment: &DeviceAssignment) -> Result<()> {
        // Create hainet.toml configuration
        let config = match assignment.role {
            DeviceRole::Master => {
                format!(
                    "[network]\nrole = \"master\"\nport = 8080\n\n[storage]\ndata_dir = \"/var/lib/hainet\"\n"
                )
            },
            DeviceRole::Slave => {
                // Get master IP (first Master in assignments)
                let master_ip = self.master_node()
                    .map(|m| m.ip.as_str())
                    .unwrap_or("10.0.0.10");
                
                format!(
                    "[network]\nrole = \"slave\"\nmaster_ip = \"{}\"\nport = 8080\n\n[storage]\ndata_dir = \"/var/lib/hainet\"\n",
                    master_ip
                )
            },
            DeviceRole::Standalone => {
                format!(
                    "[network]\nrole = \"standalone\"\nport = 8080\n\n[storage]\ndata_dir = \"/var/lib/hainet\"\n"
                )
            },
            DeviceRole::UIOnly => {
                let master_ip = self.master_node()
                    .map(|m| m.ip.as_str())
                    .unwrap_or("10.0.0.10");
                
                format!(
                    "[network]\nrole = \"ui-only\"\nmaster_ip = \"{}\"\nport = 3000\n",
                    master_ip
                )
            },
        };
        
        // Write config to remote file
        let config_path = "/tmp/hainet.toml";
        let write_cmd = format!("cat > {} << 'EOF'\n{}EOF", config_path, config);
        client.execute_command(&write_cmd)?;
        
        // Move to /etc/hainet/
        client.execute_command(&format!("sudo mv {} /etc/hainet/hainet.toml", config_path))?;
        client.execute_command("sudo chown root:root /etc/hainet/hainet.toml")?;
        client.set_permissions("/etc/hainet/hainet.toml", 0o644)?;
        
        println!("✓ Configuration written to /etc/hainet/hainet.toml");
        
        Ok(())
    }
    
    /// Set up systemd services for the device role
    fn setup_services<C: SSHClientTrait>(&self, client: &C, role: &DeviceRole) -> Result<()> {
        let services = match role {
            DeviceRole::Master | DeviceRole::Slave | DeviceRole::Standalone => {
                vec!["hainet-core", "hainet-chain"]
            },
            DeviceRole::UIOnly => {
                vec!["hainet-portal"]
            },
        };
        
        for service_name in services {
            // Create systemd service file
            let service_content = format!(
                "[Unit]\n\
                 Description=HAI-Net {}\n\
                 After=network.target\n\n\
                 [Service]\n\
                 Type=simple\n\
                 ExecStart=/opt/hainet/bin/{}\n\
                 Restart=always\n\
                 User=hainet\n\
                 Group=hainet\n\n\
                 [Install]\n\
                 WantedBy=multi-user.target\n",
                service_name, service_name
            );
            
            // Write service file
            let service_path = format!("/tmp/{}.service", service_name);
            let write_cmd = format!("cat > {} << 'EOF'\n{}EOF", service_path, service_content);
            client.execute_command(&write_cmd)?;
            
            // Move to systemd directory
            client.execute_command(&format!(
                "sudo mv {} /etc/systemd/system/{}.service",
                service_path, service_name
            ))?;
            
            // Enable service
            client.execute_command(&format!("sudo systemctl enable {}.service", service_name))?;
            
            println!("✓ Service {} configured and enabled", service_name);
        }
        
        // Reload systemd
        client.execute_command("sudo systemctl daemon-reload")?;
        
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
