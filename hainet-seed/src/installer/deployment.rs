//! # START OF FILE hainet-seed/src/installer/deployment.rs
//! Remote deployment orchestrator for multi-device HAI-Net mesh.
//! Handles role assignment, binary deployment, and service initialization.

use anyhow::{Result, bail};
use crate::installer::ssh_client::DeviceCapabilities;
use std::collections::HashMap;

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
    pub async fn deploy_all(&self, username: &str) -> Result<()> {
        println!("\n🚀 Starting deployment to {} devices...", self.assignments.len());
        
        if self.assignments.is_empty() {
            bail!("No device assignments. Call assign_roles() first.");
        }
        
        // Display deployment plan
        self.display_deployment_plan();
        
        // Deploy to each device
        for assignment in &self.assignments {
            self.deploy_to_device(assignment, username).await?;
        }
        
        println!("\n✅ Deployment complete!");
        
        // Initialize mesh coordination
        if let Some(master) = self.master_node() {
            self.initialize_mesh(master, username).await?;
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
    async fn deploy_to_device(&self, assignment: &DeviceAssignment, username: &str) -> Result<()> {
        println!("\n📦 Deploying to {} ({})...", assignment.hostname, assignment.ip);
        
        // TODO: Implement actual deployment steps:
        // 1. Build binaries for target architecture
        //    cargo build --release --target <arch>
        // 2. Transfer binaries via SCP
        //    scp hainet-* user@ip:/opt/hainet/bin/
        // 3. Create systemd services
        //    ssh user@ip "systemctl enable hainet-core"
        // 4. Configure role-specific settings
        //    ssh user@ip "echo 'ROLE=master' > /etc/hainet/config"
        // 5. Start services
        //    ssh user@ip "systemctl start hainet-core"
        
        println!("\n⚠️  Placeholder: Actual deployment steps");
        println!("   Target: {}@{}", username, assignment.ip);
        println!("   Role: {}", assignment.role);
        println!("   Arch: {}", assignment.capabilities.arch);
        
        println!("\n✓ Deployment to {} complete (mock)", assignment.hostname);
        
        Ok(())
    }
    
    /// Initialize mesh coordination
    async fn initialize_mesh(&self, master: &DeviceAssignment, username: &str) -> Result<()> {
        println!("\n🌐 Initializing mesh network...");
        println!("   Master: {} ({})", master.hostname, master.ip);
        
        let slave_count = self.slave_nodes().len();
        println!("   Slaves: {}", slave_count);
        
        // TODO: Implement mesh initialization:
        // 1. Start libp2p on master
        // 2. Get master peer ID
        // 3. Configure slaves to connect to master
        // 4. Initialize distributed storage
        // 5. Start blockchain consensus
        
        println!("\n⚠️  Placeholder: Mesh initialization steps");
        println!("   Master peer: {}@{}", username, master.ip);
        
        println!("\n✅ Mesh network initialized (mock)");
        
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
