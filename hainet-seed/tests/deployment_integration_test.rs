//! # START OF FILE hainet-seed/tests/deployment_integration_test.rs
// Integration tests for the deployment orchestrator.
// These tests will use a mock SSHClient to avoid real network operations.

mod common;
use common::MockSSHClient;

use hainet_seed::installer::deployment::{DeploymentOrchestrator, DeviceRole};
use hainet_seed::installer::ssh_client::{DeviceCapabilities, SSHCredentials};
use std::sync::{Arc, Mutex};
use std::time;

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
    // Create a dummy binary file for the test to find
    let workspace_root = hainet_seed::installer::deployment::find_workspace_root().unwrap();
    let target_dir = workspace_root.join("target/release");
    std::fs::create_dir_all(&target_dir).unwrap();
    let dummy_binaries = ["hainet-core", "hainet-chain", "hainet-bridge", "hainet-portal"];
    for binary in &dummy_binaries {
        let path = target_dir.join(binary);
        if !path.exists() {
            std::fs::File::create(&path).unwrap();
        }
    }

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

    use std::collections::HashMap;
    std::env::set_var("HAINET_SKIP_BUILD", "1");
    let timeout = time::Duration::from_millis(300000);
    let mut credentials_map = HashMap::new();
    credentials_map.insert("192.168.1.11".to_string(), ("remoteuser".to_string(), "password".to_string()));

    let result = tokio::time::timeout(timeout, orchestrator.deploy_all("testuser", &credentials_map, client_factory)).await.unwrap();
    assert!(result.is_ok());
    std::env::remove_var("HAINET_SKIP_BUILD");

    // Verify that the correct commands were executed
    let executed_commands = commands.lock().unwrap();
    assert!(executed_commands.iter().any(|cmd| cmd == "mkdir -p /opt/hainet/bin"));
    assert!(executed_commands.iter().any(|cmd| cmd.starts_with("upload_file to /opt/hainet/bin/hainet-core")));
    assert!(executed_commands.iter().any(|cmd| cmd.contains("sudo mv /tmp/hainet.toml /etc/hainet/hainet.toml")));
    assert!(executed_commands.iter().any(|cmd| cmd.contains("sudo systemctl enable hainet-core.service")));
}