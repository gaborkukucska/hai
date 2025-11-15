//! HAI-Net Portal Main Binary
//! 
//! Chat interface for natural language interaction with your AI persona.

use tracing::info;
use anyhow::Result;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    // Create logs directory
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("hainet-portal");
    let logs_dir = data_dir.join("logs");
    std::fs::create_dir_all(&logs_dir)?;
    
    // Create log file with timestamp
    let log_file = logs_dir.join(format!(
        "hainet-portal-{}.log",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    ));
    
    // Initialize tracing with file appender
    use tracing_subscriber::prelude::*;
    use tracing_subscriber::{fmt, EnvFilter};
    
    let file_appender = tracing_appender::rolling::never(&logs_dir, log_file.file_name().unwrap());
    let (file_writer, _guard) = tracing_appender::non_blocking(file_appender);
    
    tracing_subscriber::registry()
        .with(fmt::layer().with_writer(std::io::stdout))
        .with(fmt::layer().with_writer(file_writer).with_ansi(false))
        .with(EnvFilter::new("hainet_portal=debug,info"))
        .init();

    info!("🖥️  HAI-Net Portal starting up...");
    info!("📋 Version: {}", env!("CARGO_PKG_VERSION"));
    info!("📝 Logs being written to: {}", log_file.display());
    info!("💬 Chat interface initializing...");

    // TODO: Initialize portal components
    // - Tauri app
    // - WebSocket connection to persona
    // - Chat interface
    // - Settings UI
    // - Agent state visualization
    // - Real-time updates

    info!("✅ HAI-Net Portal initialized successfully");
    
    // Keep running until shutdown signal
    tokio::signal::ctrl_c().await?;
    info!("🛑 HAI-Net Portal shutting down gracefully");
    
    Ok(())
}
