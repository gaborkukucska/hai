use std::process::Stdio;
use std::time::Duration;
use anyhow::{Result, Context, anyhow};
use tokio::process::{Command, Child};
use tokio::time::sleep;
use tracing::{info, warn, error, debug};
use std::path::PathBuf;

pub struct AgentSidecar {
    process: Option<Child>,
    port: u16,
}

impl AgentSidecar {
    pub fn new(port: u16) -> Self {
        Self {
            process: None,
            port,
        }
    }

    /// Spawn the Python sidecar process
    pub async fn spawn(&mut self, hai_root: &str) -> Result<()> {
        let agent_svc_dir = PathBuf::from(hai_root).join("services").join("agent-svc");
        let venv_python = agent_svc_dir.join(".venv").join("bin").join("python");
        
        info!("Spawning AgentService sidecar on port {}", self.port);
        
        if !venv_python.exists() {
            return Err(anyhow!("Python virtual environment not found at {:?}", venv_python));
        }

        let child = Command::new(venv_python)
            .arg("bridge.py")
            .current_dir(&agent_svc_dir)
            .env("AGENT_SVC_PORT", self.port.to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .context("Failed to spawn Python AgentService sidecar")?;
            
        self.process = Some(child);
        
        // Give it a moment to bind to the port
        // In a real production system, we'd loop and try connecting to the gRPC port
        sleep(Duration::from_secs(3)).await;
        
        Ok(())
    }

    /// Stop the sidecar process
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(mut child) = self.process.take() {
            info!("Stopping AgentService sidecar");
            child.kill().await.context("Failed to kill sidecar process")?;
            child.wait().await?;
        }
        Ok(())
    }
}

impl Drop for AgentSidecar {
    fn drop(&mut self) {
        if let Some(child) = &mut self.process {
            // Attempt a synchronous kill as fallback if dropped unexpectedly
            if let Some(id) = child.id() {
                debug!("Killing sidecar process {} on drop", id);
                let _ = std::process::Command::new("kill")
                    .arg("-9")
                    .arg(id.to_string())
                    .output();
            }
        }
    }
}
