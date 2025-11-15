//! HAI-Net Core Library
//!
//! Core functionality for the HAI-Net daemon including networking, storage,
//! and service coordination.

pub mod logging;
pub mod multimodal;
pub mod networking;
pub mod storage;

use anyhow::Result;
use tracing::info;

pub use multimodal::{
    AudioFormat, AudioProcessor, DeviceRole, MultimodalConfig, SpeechToText, TranscriptionResult,
    WhisperConfig,
};
pub use storage::{ContentAddressedStore, ContentHash, P2PFileSync, StorageManager};

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
