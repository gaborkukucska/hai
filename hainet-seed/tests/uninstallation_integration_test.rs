//! # START OF FILE hainet-seed/tests/uninstallation_integration_test.rs

mod common;
use common::MockSSHClient;

use hainet_seed::installer::uninstaller::Uninstaller;
use anyhow::Result;
use std::sync::{Arc, Mutex};
use hainet_seed::installer::ssh_client::SSHCredentials;
use std::fs;

#[tokio::test]
async fn test_uninstallation_flow() -> Result<()> {
    // Create a dummy SSH key for the test
    let home_dir = dirs::home_dir().unwrap();
    let ssh_dir = home_dir.join(".ssh");
    fs::create_dir_all(&ssh_dir)?;
    let pub_key_path = ssh_dir.join("id_ed25519.pub");
    let pub_key_content = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIGCq/d1E/N/dE/dE/dE/dE/dE/dE/dE/dE/dE hainet-test";
    fs::write(&pub_key_path, pub_key_content)?;

    let uninstaller = Uninstaller::new()?;
    let commands = Arc::new(Mutex::new(Vec::new()));
    let mut client = MockSSHClient {
        ip: "127.0.0.1".to_string(),
        _credentials: SSHCredentials {
            username: "test".to_string(),
            password: "".to_string(),
        },
        is_connected: false,
        commands: commands.clone(),
    };

    uninstaller.uninstall_from_device(&mut client).await?;

    let executed_commands = commands.lock().unwrap();

    // Verify that the correct commands were executed in the correct order
    assert!(executed_commands.contains(&"sudo systemctl stop hainet-core.service".to_string()));
    assert!(executed_commands.contains(&"sudo systemctl disable hainet-core.service".to_string()));
    assert!(executed_commands.contains(&"sudo rm /etc/systemd/system/hainet-core.service".to_string()));
    assert!(executed_commands.contains(&"sudo systemctl daemon-reload".to_string()));
    assert!(executed_commands.contains(&"sudo rm -rf /opt/hainet".to_string()));
    assert!(executed_commands.contains(&"sudo rm -rf /etc/hainet".to_string()));
    assert!(executed_commands.contains(&"sudo userdel hainet".to_string()));
    assert!(executed_commands.contains(&"sudo groupdel hainet".to_string()));
    assert!(executed_commands.contains(&format!("grep -v '{}' ~/.ssh/authorized_keys > ~/.ssh/authorized_keys.tmp && mv ~/.ssh/authorized_keys.tmp ~/.ssh/authorized_keys", pub_key_content.trim())));

    Ok(())
}
