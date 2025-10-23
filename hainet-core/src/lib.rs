//! HAI-Net Core Library
//! 
//! Core functionality for the HAI-Net daemon including networking, storage,
//! and service coordination.

pub mod storage;
pub mod multimodal;

use tracing::info;
use anyhow::Result;

pub use storage::{StorageManager, ContentAddressedStore, ContentHash, P2PFileSync};
pub use multimodal::{
    AudioFormat, AudioProcessor, SpeechToText, TranscriptionResult, 
    WhisperConfig, MultimodalConfig, DeviceRole
};

/// Initialize the HAI-Net core system
pub async fn init() -> Result<()> {
    info!("🌐 Initializing HAI-Net Core");
    
    // TODO: Initialize core components
    // - Configuration loading
    // - Network discovery
    // - Storage system
    // - Service registry
    
    Ok(())
}
