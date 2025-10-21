//! Identity Management for HAI-Net
//!
//! Implements Decentralized Identifiers (DIDs) and cryptographic binding
//! between humans and their AI personas.

pub mod did;
pub mod keypair;
pub mod link;

pub use did::DID;
pub use keypair::Keypair;
pub use link::{PersonaLink, LinkRecord};

use anyhow::Result;

/// Initialize the identity system
pub async fn init() -> Result<()> {
    tracing::info!("🔑 Initializing HAI-Net Identity system...");
    
    // Identity system is stateless - no initialization needed
    
    tracing::info!("✅ HAI-Net Identity system initialized");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_identity_init() {
        let result = init().await;
        assert!(result.is_ok());
    }
}
