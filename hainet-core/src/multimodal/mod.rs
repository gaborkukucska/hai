//! # START OF FILE hainet-core/src/multimodal/mod.rs
//! Multimodal AI capabilities for HAI-Net
//!
//! This module provides core multimodal functionality including:
//! - Speech-to-Text (STT) via Whisper
//! - Text-to-Speech (TTS) via Piper/Coqui (future)
//! - Audio processing and format conversion
//!
//! ## Architecture
//!
//! Multimodal capabilities are designed for multi-device deployment:
//! - Can run on a single device (offline-first)
//! - Can be distributed across mesh network (master-slave architecture)
//! - Master coordinates service placement based on device capabilities
//!
//! ## Privacy & Offline-First
//!
//! All multimodal processing happens locally by default:
//! - Models stored in `~/.hainet/models/`
//! - No external API calls unless explicitly configured
//! - User data never leaves the local hub without consent

pub mod audio;
pub mod stt;
pub mod tts;

pub use audio::{AudioFormat, AudioProcessor};
pub use stt::{SpeechToText, TranscriptionResult, WhisperConfig};
pub use tts::{TextToSpeech, SynthesisConfig, SynthesisResult, AudioOutputFormat};

/// Configuration for multimodal services
#[derive(Debug, Clone)]
pub struct MultimodalConfig {
    /// Enable Speech-to-Text service
    pub stt_enabled: bool,
    
    /// Path to Whisper model files
    pub whisper_model_path: std::path::PathBuf,
    
    /// Default language for transcription ("auto" for automatic detection)
    pub default_language: String,
    
    /// Device role in mesh network
    pub device_role: DeviceRole,
}

/// Device role in multi-device mesh network
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceRole {
    /// Master coordinator (assigns services to slaves)
    Master,
    
    /// Slave worker (receives service assignments)
    Slave,
    
    /// Standalone (single device, runs all services locally)
    Standalone,
}

impl Default for MultimodalConfig {
    fn default() -> Self {
        let home_dir = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
        
        Self {
            stt_enabled: true,
            whisper_model_path: home_dir.join(".hainet/models/whisper-base.en"),
            default_language: "auto".to_string(),
            device_role: DeviceRole::Standalone,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_config() {
        let config = MultimodalConfig::default();
        assert!(config.stt_enabled);
        assert_eq!(config.default_language, "auto");
        assert_eq!(config.device_role, DeviceRole::Standalone);
    }
    
    #[test]
    fn test_device_roles() {
        assert_ne!(DeviceRole::Master, DeviceRole::Slave);
        assert_ne!(DeviceRole::Master, DeviceRole::Standalone);
        assert_ne!(DeviceRole::Slave, DeviceRole::Standalone);
    }
}
