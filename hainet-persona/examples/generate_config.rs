//! Generate default HAI-Net configuration file
//! 
//! Usage: cargo run --package hainet-persona --example generate_config

use hainet_persona::HaiNetConfig;

fn main() -> anyhow::Result<()> {
    // Load default configuration
    let config = HaiNetConfig::default();
    
    // Save to ~/.hainet/config.toml
    let config_path = HaiNetConfig::default_config_path();
    
    println!("Generating default HAI-Net configuration...");
    println!("Config path: {:?}", config_path);
    
    config.save(&config_path)?;
    
    println!("\n✅ Successfully created config file at: {:?}", config_path);
    println!("\nDefault settings:");
    println!("  Admin Model:    {}", config.default_models.admin_model);
    println!("  PM Model:       {}", config.default_models.pm_model);
    println!("  Worker Model:   {}", config.default_models.worker_model);
    println!("  Guardian Model: {}", config.default_models.guardian_model);
    println!("\nYou can now edit this file to customize your HAI-Net configuration.");
    println!("The config will be automatically loaded on next startup.");
    
    Ok(())
}
