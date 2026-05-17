//! HAI-Net Blockchain Main Binary
//! 
//! Entry point for the blockchain validator and governance system.
//! Works in both development mode (cargo run) and as a deployed systemd service.

use tracing::{info, debug};
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // Load configuration
    let config = hainet_core::config::HainetConfig::load();

    // Initialize logging with configured directory
    let _guard = hainet_core::logging::initialize_logging_with_dir(
        "hainet-chain",
        &config.logs.log_level,
        Some(&config.logs.log_dir),
    )?;

    info!("⛓️  HAI-Net Blockchain starting up...");
    info!("📋 Version: {}", env!("CARGO_PKG_VERSION"));
    info!("🏛️ Constitutional governance: ACTIVE");
    info!("📄 Role: {}", config.role_display());

    // TODO: Initialize blockchain components
    // - Tendermint consensus
    // - Block validation
    // - State machine
    // - Governance proposals
    // - Membership registry
    // - Constitutional validation

    info!("✅ HAI-Net Blockchain initialized successfully");

    // Periodic heartbeat
    let heartbeat = tokio::spawn(async {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            debug!("💓 hainet-chain heartbeat — alive");
        }
    });

    // Keep running until shutdown signal
    tokio::signal::ctrl_c().await?;
    info!("🛑 HAI-Net Blockchain shutting down gracefully");
    heartbeat.abort();
    
    Ok(())
}
