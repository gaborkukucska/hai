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

    /// Transcribe audio data using hainet-core STT engine
    /// 
    /// Flow:
    /// 1. Portal sends audio (base64-encoded) to this handler
    /// 2. Handler decodes and processes audio via hainet-core
    /// 3. whisper.cpp performs transcription
    /// 4. Result returned directly to Portal
    pub async fn transcribe(&self, audio: AudioData) -> Result<TranscriptionResult> {
        use hainet_core::multimodal::audio::{AudioProcessor, AudioFormat};
        use hainet_core::multimodal::stt::SpeechToText;
        
        let start_time = std::time::Instant::now();
        let config = self.config.read().await.clone();

        // Decode base64 audio
        let processor = AudioProcessor::new();
        let audio_bytes = processor.decode_base64(&audio.data)
            .context("Failed to decode base64 audio")?;
        
        // Detect audio format
        let format = AudioFormat::detect(&audio_bytes);
        
        tracing::info!("Detected audio format: {:?}", format);
        
        // Process audio (convert to WAV if needed, resample to 16kHz mono)
        let processed_audio = processor.process(&audio_bytes)
            .context("Failed to process audio")?;
        
        // Note: VAD is currently disabled as we'd need to parse the WAV data
        // to extract samples. whisper.cpp will handle silence detection internally.
        if config.vad_enabled {
            tracing::debug!("VAD enabled but deferred to whisper.cpp");
        }
        
        // Create STT engine
        let stt = SpeechToText::new()
            .context("Failed to initialize STT engine")?;
        
        // Check if whisper.cpp is available
        if !stt.is_ready() {
            return Err(anyhow::anyhow!(
                "whisper.cpp not found. Please run 'hainet-seed install' to set up STT."
            ));
        }
        
        // Perform transcription
        let result = if config.language == "auto" {
            stt.transcribe_auto_detect(&processed_audio).await?
        } else {
            stt.transcribe(&processed_audio).await?
        };
        
        let processing_time = start_time.elapsed().as_millis() as u64;
        
        tracing::info!(
            "Transcription complete in {}ms: \"{}\" (confidence: {:.2})",
            processing_time,
            result.text,
            result.confidence
        );
        
        // Convert hainet-core result to portal result
        Ok(TranscriptionResult {
            text: result.text,
            confidence: result.confidence,
            language: result.language,
            processing_time_ms: processing_time,
        })
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
