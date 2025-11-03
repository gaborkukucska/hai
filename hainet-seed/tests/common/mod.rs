//! # START OF FILE hainet-seed/tests/common/mod.rs

use anyhow::Result;
use hainet_seed::installer::ssh_client::{SSHClientTrait, DeviceCapabilities, SSHCredentials};
use std::path::Path;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct MockSSHClient {
    pub ip: String,
    pub _credentials: SSHCredentials,
    pub is_connected: bool,
    pub commands: Arc<Mutex<Vec<String>>>,
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
