//! # START OF FILE hainet-portal/src-tauri/src/stt_handler.rs
//! Speech-to-Text handler using HAI-Net's provider discovery system
//! Handles audio transcription with VAD (Voice Activity Detection) support
//! Integrates with multi-device hub for distributed processing

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use anyhow::{Result, Context};

/// STT provider type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum STTProvider {
    /// Local provider discovered via AI provider discovery
    Local,
    /// Remote hub device (for distributed processing)
    Remote { device_id: String },
    /// External API (fallback when offline not available)
    External { api_name: String },
}

/// Configuration for STT processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct STTConfig {
    /// Preferred provider (None = auto-select)
    pub preferred_provider: Option<STTProvider>,
    /// Language code (e.g., "en", "auto" for auto-detect)
    pub language: String,
    /// Enable voice activity detection
    pub vad_enabled: bool,
    /// VAD threshold (0.0-1.0, higher = more strict)
    pub vad_threshold: f32,
    /// Offline-only mode (reject external APIs)
    pub offline_only: bool,
}

impl Default for STTConfig {
    fn default() -> Self {
        Self {
            preferred_provider: None, // Auto-select best available
            language: "auto".to_string(),
            vad_enabled: true,
            vad_threshold: 0.5,
            offline_only: true, // Privacy-first: default to offline
        }
    }
}

/// STT transcription result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionResult {
    /// Transcribed text
    pub text: String,
    /// Confidence score (0.0-1.0)
    pub confidence: f32,
    /// Language detected
    pub language: String,
    /// Processing time in milliseconds
    pub processing_time_ms: u64,
}

/// Audio format for processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioData {
    /// Base64-encoded audio data
    pub data: String,
    /// Sample rate (e.g., 16000, 44100)
    pub sample_rate: u32,
    /// Number of channels (1 = mono, 2 = stereo)
    pub channels: u16,
    /// Audio format (wav, ogg, mp3, etc.)
    pub format: String,
}

/// STT Handler - Bridge to Admin AI for distributed STT processing
/// 
/// This handler does NOT directly call AI providers. Instead, it:
/// 1. Sends audio to Admin AI via IPC
/// 2. Admin AI uses provider discovery to find best STT provider
/// 3. Admin AI may delegate to PM → Worker → MCP tools
/// 4. Returns transcription result to Portal UI
pub struct STTHandler {
    config: Arc<tokio::sync::RwLock<STTConfig>>,
}

impl STTHandler {
    /// Create new STT handler with default configuration
    pub fn new() -> Self {
        Self {
            config: Arc::new(tokio::sync::RwLock::new(STTConfig::default())),
        }
    }

    /// Create new STT handler with custom configuration
    pub fn with_config(config: STTConfig) -> Self {
        Self {
            config: Arc::new(tokio::sync::RwLock::new(config)),
        }
    }

    /// Update STT configuration
    pub async fn update_config(&self, config: STTConfig) {
        let mut current_config = self.config.write().await;
        *current_config = config;
    }

    /// Get current configuration
    pub async fn get_config(&self) -> STTConfig {
        self.config.read().await.clone()
    }

    /// Transcribe audio data by delegating to Admin AI
    /// 
    /// Flow:
    /// 1. Portal sends audio to this handler
    /// 2. Handler forwards to Admin AI (via admin_bridge)
    /// 3. Admin AI creates STT project (or uses existing STT worker)
    /// 4. PM/Worker discovers best STT provider (local/remote/external)
    /// 5. Result flows back: Worker → PM → Admin → Portal
    pub async fn transcribe(&self, audio: AudioData) -> Result<TranscriptionResult> {
        let _start_time = std::time::Instant::now();
        let config = self.config.read().await.clone();

        // Apply VAD if enabled
        if config.vad_enabled {
            // TODO: Implement VAD check here
            // For now, pass through all audio
        }

        // TODO: Forward audio to Admin AI via AdminBridge
        // This will be implemented when we integrate with admin_bridge.rs
        // The Admin AI will handle:
        // - Provider discovery (via hainet-persona/src/ai_providers/)
        // - Multi-device coordination (via hub master/slave)
        // - Fallback logic (local → remote → external)
        // - Constitutional compliance (via Guardian)

        // For now, return a placeholder
        Err(anyhow::anyhow!(
            "STT integration with Admin AI not yet implemented. \
            This requires Admin AI to discover STT providers and coordinate transcription."
        ))
    }

    /// Calculate audio energy level (for VAD)
    pub fn calculate_audio_level(audio_samples: &[f32]) -> f32 {
        if audio_samples.is_empty() {
            return 0.0;
        }

        // Calculate RMS (Root Mean Square) energy
        let sum_squares: f32 = audio_samples.iter()
            .map(|&sample| sample * sample)
            .sum();

        (sum_squares / audio_samples.len() as f32).sqrt()
    }

    /// Simple VAD check based on audio energy
    pub fn is_speech_detected(audio_level: f32, threshold: f32) -> bool {
        audio_level > threshold
    }
}

impl Default for STTHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = STTConfig::default();
        assert_eq!(config.preferred_provider, None); // Auto-select
        assert_eq!(config.language, "auto");
        assert!(config.vad_enabled);
        assert_eq!(config.vad_threshold, 0.5);
        assert!(config.offline_only); // Privacy-first default
    }

    #[test]
    fn test_audio_level_calculation() {
        let samples = vec![0.1, 0.2, 0.3, 0.4, 0.5];
        let level = STTHandler::calculate_audio_level(&samples);
        assert!(level > 0.0);
        assert!(level < 1.0);
    }

    #[test]
    fn test_vad_detection() {
        assert!(STTHandler::is_speech_detected(0.6, 0.5));
        assert!(!STTHandler::is_speech_detected(0.4, 0.5));
    }

    #[test]
    fn test_empty_audio_level() {
        let samples: Vec<f32> = vec![];
        let level = STTHandler::calculate_audio_level(&samples);
        assert_eq!(level, 0.0);
    }
}
