//! HAI-Net Seed Library
//! 
//! AI-guided installer and bootstrap system for setting up new HAI-Net nodes.

// Installer module (Cycle 0.5 - Phase B)
pub mod installer;

// TODO: Implement these modules in later cycles
// pub mod setup;
// pub mod onboarding;
// pub mod bootstrap;
// pub mod identity;

use anyhow::Result;
use tracing::info;
use serde::Deserialize;

// Re-export key installer types
pub use installer::Installer;
pub use installer::platform::{Platform, SystemTier, Architecture};

/// Configuration injected by the NoSlop mobile-assisted hub deployer.
/// Serialized as JSON and passed via `hainet-seed install --config <path>`.
#[derive(Debug, Deserialize)]
pub struct HubConfig {
    /// Cloudflare Tunnel token for remote mobile access.
    /// Required unless the user has a static IP and port forwarding.
    #[serde(default)]
    pub cloudflare_token: Option<String>,

    /// Whether the user's ISP provides a static IP (bypasses tunnel requirement).
    #[serde(default)]
    pub has_static_ip: bool,

    /// Optional shared media folder path.
    #[serde(default)]
    pub shared_folder: Option<String>,

    /// NoSlop identity to clone onto this Hub (Identity Clone model).
    /// When present, the Hub becomes a permanent mirror of the mobile identity,
    /// keeping the same .onion address online 24/7 and allowing the AI Persona
    /// to sign posts on behalf of the user.
    #[serde(default)]
    pub identity: Option<HubIdentity>,
}

/// The user's NoSlop cryptographic identity, transferred during deployment.
/// All fields are Base64-encoded. The Hub stores these in its own keychain
/// and uses them for signing, encryption, and Tor hidden service registration.
#[derive(Debug, Deserialize)]
pub struct HubIdentity {
    /// Ed25519 signing public key (Base64, X.509 wrapped)
    pub public_key: String,
    /// Ed25519 signing private key (Base64, PKCS#8 wrapped)
    pub private_key: String,
    /// X25519 encryption public key (Base64)
    pub enc_public_key: String,
    /// X25519 encryption private key (Base64)
    pub enc_private_key: String,
    /// Tor v3 .onion address derived from the Ed25519 key
    pub onion_address: String,
    /// Human-readable display name ("handle.tripcode")
    pub display_name: String,
}

/// Initialize the seed system
pub async fn init() -> Result<()> {
    info!("🌱 Initializing HAI-Net Seed system...");
    
    // TODO: Initialize core components
    // - System requirements checker
    // - Identity generation
    // - Membership application system
    // - Model downloader
    // - Hub configuration
    
    info!("✅ HAI-Net Seed system initialized");
    Ok(())
}

/// Main seed service entry point
pub struct SeedService {
    installer: Installer,
}

impl SeedService {
    pub async fn new() -> Result<Self> {
        init().await?;
        
        let installer = Installer::new().await?;
        
        Ok(Self {
            installer,
        })
    }
    
    pub async fn install(&mut self) -> Result<()> {
        info!("🚀 Starting HAI-Net installation...");
        
        // Run installation workflow
        self.installer.install().await?;
        
        // TODO: Additional setup steps
        // - Generate identity
        // - Apply for membership
        // - Configure hub
        // - Bootstrap network
        
        Ok(())
    }

    /// Non-interactive installation driven by a JSON config file.
    /// Called by the NoSlop mobile-assisted hub deployer via SSH.
    pub async fn install_from_config(&mut self, config_path: &str) -> Result<()> {
        info!("📋 Loading hub config from: {}", config_path);
        
        let config_str = std::fs::read_to_string(config_path)?;
        let config: HubConfig = serde_json::from_str(&config_str)?;

        info!("📋 Config loaded successfully");
        if config.has_static_ip {
            info!("🌐 Static IP mode: skipping Cloudflare tunnel setup");
        }
        if let Some(ref identity) = config.identity {
            info!("🔑 NoSlop identity clone: {} (onion: {}...)",
                identity.display_name, &identity.onion_address[..identity.onion_address.len().min(20)]);
        }
        if let Some(ref folder) = config.shared_folder {
            info!("📂 Shared media folder: {}", folder);
        }

        // Step 1: Run the standard local installation workflow non-interactively
        info!("🚀 Running local dependency installation...");
        // For now, we skip the interactive parts (shared drive prompts, device scanning)
        // and go straight to installing core dependencies.
        self.installer.install_core_deps_only().await?;

        // Step 2: Set up Cloudflare tunnel if a token was provided
        if let Some(ref token) = config.cloudflare_token {
            if !token.is_empty() {
                info!("🌐 Setting up Cloudflare Tunnel for remote mobile access...");
                self.setup_cloudflared_tunnel(token).await?;
            }
        } else if !config.has_static_ip {
            tracing::warn!("⚠️  No Cloudflare token and no static IP — remote mobile access will not work!");
        }

        // Step 3: Import the NoSlop identity into the Hub's keychain
        if let Some(ref identity) = config.identity {
            info!("🔐 Importing NoSlop identity into Hub keychain...");
            let keychain_dir = dirs::home_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("/root"))
                .join(".hainet")
                .join("identity");
            let _ = std::fs::create_dir_all(&keychain_dir);

            // Write each key component to disk (the Hub's core services read from here)
            std::fs::write(keychain_dir.join("ed25519_pub.b64"), &identity.public_key)?;
            std::fs::write(keychain_dir.join("ed25519_priv.b64"), &identity.private_key)?;
            std::fs::write(keychain_dir.join("x25519_pub.b64"), &identity.enc_public_key)?;
            std::fs::write(keychain_dir.join("x25519_priv.b64"), &identity.enc_private_key)?;
            std::fs::write(keychain_dir.join("onion_address"), &identity.onion_address)?;
            std::fs::write(keychain_dir.join("display_name"), &identity.display_name)?;

            // Restrict permissions to owner-only
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                for entry in std::fs::read_dir(&keychain_dir)? {
                    let entry = entry?;
                    std::fs::set_permissions(entry.path(), std::fs::Permissions::from_mode(0o600))?;
                }
                std::fs::set_permissions(&keychain_dir, std::fs::Permissions::from_mode(0o700))?;
            }
            info!("✅ Identity imported: {} → {}", identity.display_name, keychain_dir.display());
        }

        // Step 4: Write the hub config for hainet-core to pick up on startup
        let hub_config_dir = "/etc/hainet";
        let _ = std::fs::create_dir_all(hub_config_dir);
        std::fs::write(
            format!("{}/hub_config.json", hub_config_dir), 
            &config_str
        )?;
        info!("✅ Hub config written to {}/hub_config.json", hub_config_dir);

        info!("✅ Config-driven installation complete!");
        Ok(())
    }

    /// Install and configure cloudflared as a systemd service.
    async fn setup_cloudflared_tunnel(&self, token: &str) -> Result<()> {
        use std::process::Command;

        // Install cloudflared binary
        info!("📥 Installing cloudflared...");
        let install_script = r#"
            if ! command -v cloudflared &>/dev/null; then
                curl -sL https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-amd64 -o /tmp/cloudflared
                sudo mv /tmp/cloudflared /usr/local/bin/cloudflared
                sudo chmod +x /usr/local/bin/cloudflared
            fi
        "#;
        Command::new("bash").arg("-c").arg(install_script).status()?;

        // Create systemd service
        let service_content = format!(
            r#"[Unit]
Description=Cloudflare Tunnel for HAI-Net Hub
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/cloudflared tunnel run --token {token}
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
"#
        );

        let service_path = "/etc/systemd/system/hainet-tunnel.service";
        Command::new("bash")
            .arg("-c")
            .arg(format!("echo '{}' | sudo tee {}", service_content, service_path))
            .status()?;

        Command::new("sudo").args(["systemctl", "daemon-reload"]).status()?;
        Command::new("sudo").args(["systemctl", "enable", "hainet-tunnel"]).status()?;
        Command::new("sudo").args(["systemctl", "start", "hainet-tunnel"]).status()?;

        info!("✅ Cloudflare Tunnel service installed and started");
        Ok(())
    }
    
    pub async fn check_requirements(&self) -> Result<()> {
        info!("🔍 Checking system requirements...");
        
        info!("Platform: {}", self.installer.platform());
        info!("System Tier: {}", self.installer.tier());
        
        Ok(())
    }
}
