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
    // Initialize logging
    let _guard = hainet_core::logging::initialize_logging("hainet-seed", "debug")?;

    let cli = Cli::parse();

    info!("🌱 HAI-Net Seed starting up...");
    info!("📋 Version: {}", env!("CARGO_PKG_VERSION"));

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
