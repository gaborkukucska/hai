//! HAI-Net Seed Main Binary
//! 
//! AI-guided installer and bootstrap system for setting up new HAI-Net nodes.

use tracing::info;
use anyhow::Result;
use clap::{Parser, Subcommand};
use hainet_seed::SeedService;

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
    /// Check system requirements
    Check,
    /// Generate new identity keypair
    GenIdentity,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("hainet_seed=debug,info")
        .init();

    let cli = Cli::parse();

    info!("🌱 HAI-Net Seed starting up...");
    info!("📋 Version: {}", env!("CARGO_PKG_VERSION"));

    match cli.command {
        Commands::Install => {
            info!("🚀 Starting AI-guided installation...");
            
            let mut service = SeedService::new().await?;
            service.install().await?;
            
            info!("✅ HAI-Net installation completed successfully!");
            info!("🎯 Next steps:");
            info!("   1. Verify Ollama is running: ollama list");
            info!("   2. Start HAI-Net persona: cargo run --package hainet-persona");
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
