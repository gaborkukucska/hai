//! # START OF FILE hainet-seed/tests/deployment_integration_test.rs
// Integration tests for the deployment orchestrator.
// These tests will use a mock SSHClient to avoid real network operations.

use hainet_seed::installer::deployment::{DeploymentOrchestrator, DeviceRole};
use hainet_seed::installer::ssh_client::{DeviceCapabilities, SSHClientTrait, SSHCredentials};
use anyhow::Result;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time;

// Mock SSHClient for testing
#[derive(Clone)]
struct MockSSHClient {
    ip: String,
    _credentials: SSHCredentials,
    is_connected: bool,
    commands: Arc<Mutex<Vec<String>>>,
}

impl SSHClientTrait for MockSSHClient {
    fn connect(&mut self) -> Result<()> {
        self.is_connected = true;
        Ok(())
    }

    fn authenticate_password(&mut self) -> Result<()> {
        Ok(())
    }

    fn authenticate_pubkey(&mut self, _private_key_path: &Path, _passphrase: Option<&str>) -> Result<()> {
        Ok(())
    }

    fn disconnect(&mut self) -> Result<()> {
        self.is_connected = false;
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.is_connected
    }

    fn assess_capabilities(&self) -> Result<DeviceCapabilities> {
        Ok(DeviceCapabilities {
            ip: self.ip.clone(),
            hostname: "mock-device".to_string(),
            cpu_cores: 4,
            ram_gb: 8.0,
            gpu: None,
            disk_gb: 100.0,
            os: "Linux".to_string(),
            arch: "x86_64".to_string(),
            score: 100.0,
        })
    }

    fn execute_command(&self, command: &str) -> Result<String> {
        self.commands.lock().unwrap().push(command.to_string());
        Ok("mock output".to_string())
    }

    fn upload_file(&self, _local_path: &Path, remote_path: &str) -> Result<()> {
        self.commands.lock().unwrap().push(format!("upload_file to {}", remote_path));
        Ok(())
    }

    fn create_remote_directory(&self, path: &str) -> Result<()> {
        self.commands.lock().unwrap().push(format!("mkdir -p {}", path));
        Ok(())
    }

    fn set_permissions(&self, path: &str, mode: u32) -> Result<()> {
        self.commands.lock().unwrap().push(format!("chmod {:o} {}", mode, path));
        Ok(())
    }
}

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
fn test_orchestrator_creation() {
    let orchestrator = DeploymentOrchestrator::new();
    assert_eq!(orchestrator.assignments().len(), 0);
}

#[test]
fn test_role_assignment() {
    let mut orchestrator = DeploymentOrchestrator::new();
    let capabilities = create_mock_capabilities(3);
    orchestrator.assign_roles(capabilities).unwrap();
    assert_eq!(orchestrator.assignments().len(), 3);

    let master_count = orchestrator.assignments().iter().filter(|a| a.role == DeviceRole::Master).count();
    let slave_count = orchestrator.assignments().iter().filter(|a| a.role == DeviceRole::Slave).count();

    assert_eq!(master_count, 1);
    assert_eq!(slave_count, 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_deployment_all() {
    let mut orchestrator = DeploymentOrchestrator::new();
    let capabilities = create_mock_capabilities(2);
    orchestrator.assign_roles(capabilities).unwrap();

    let commands = Arc::new(Mutex::new(Vec::new()));
    let client_factory = |ip: String, credentials: SSHCredentials| {
        MockSSHClient {
            ip,
            _credentials: credentials,
            is_connected: false,
            commands: commands.clone(),
        }
    };

    let timeout = time::Duration::from_millis(300000);
    let result = tokio::time::timeout(timeout, orchestrator.deploy_all("testuser", client_factory)).await;
    assert!(result.is_ok());

    // Verify that the correct commands were executed
    let executed_commands = commands.lock().unwrap();
    assert!(executed_commands.iter().any(|cmd| cmd.contains("mkdir -p /opt/hainet/bin")));
    assert!(executed_commands.iter().any(|cmd| cmd.contains("sudo mv /tmp/hainet.toml /etc/hainet/hainet.toml")));
    assert!(executed_commands.iter().any(|cmd| cmd.contains("sudo systemctl enable hainet-core.service")));
    assert!(executed_commands.iter().any(|cmd| cmd.contains("sudo systemctl start hainet-core.service")));
}