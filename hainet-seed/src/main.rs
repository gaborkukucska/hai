//! HAI-Net Seed Main Binary
//! 
//! AI-guided installer and bootstrap system for setting up new HAI-Net nodes.

use tracing::info;
use anyhow::Result;
use clap::{Parser, Subcommand};
use hainet_seed::SeedService;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "hainet-seed")]
#[command(about = "HAI-Net Seed - AI-guided installer and bootstrap system")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Install HAI-Net with AI-guided setup
    Install,
    /// Uninstall HAI-Net from deployed devices
    Uninstall,
    /// Check system requirements
    Check,
    /// Generate new identity keypair
    GenIdentity,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Create logs directory
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("hainet-seed");
    let logs_dir = data_dir.join("logs");
    std::fs::create_dir_all(&logs_dir)?;
    
    // Create log file with timestamp
    let log_file = logs_dir.join(format!(
        "hainet-seed-{}.log",
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
        .with(EnvFilter::new("hainet_seed=debug,info"))
        .init();

    let cli = Cli::parse();

    info!("🌱 HAI-Net Seed starting up...");
    info!("📋 Version: {}", env!("CARGO_PKG_VERSION"));
    info!("📝 Logs being written to: {}", log_file.display());

    match cli.command {
        Commands::Install => {
            info!("🚀 Starting AI-guided installation...");
            
            let mut service = SeedService::new().await?;
            service.install().await?;
            
            info!("✅ HAI-Net installation completed successfully!");
        }
        Commands::Uninstall => {
            info!("🗑️ Starting uninstallation process...");
            let uninstaller = hainet_seed::installer::uninstaller::Uninstaller::new()?;
            uninstaller.uninstall().await?;
        }
        Commands::Check => {
            info!("🔍 Checking system requirements...");
            
            let service = SeedService::new().await?;
            service.check_requirements().await?;
            
            info!("✅ System check complete");
        }
        Commands::GenIdentity => {
            info!("🔑 Generating new identity...");
            // TODO: Generate DID and keypair
            println!("Identity generation will be implemented here!");
        }
    }

    info!("✅ HAI-Net Seed completed successfully");
    
    Ok(())
}
