//! # START OF FILE hainet-core/src/multimodal/stt.rs
//! Speech-to-Text (STT) engine using Whisper
//!
//! This module provides offline-first speech recognition capabilities
//! using the Whisper model via external whisper.cpp integration.
//!
//! ## Architecture
//!
//! - **Offline-first**: Models stored locally in `~/.hainet/models/`
//! - **Multi-device ready**: Can run on master or slave devices
//! - **Privacy-preserving**: Audio never leaves the local hub
//!
//! ## Implementation Approach
//!
//! Currently uses external whisper.cpp process for transcription.
//! Future: Direct Rust integration via whisper-rs or stabilized Candle.
//!
//! ## Setup Requirements
//!
//! 1. Install whisper.cpp: https://github.com/ggerganov/whisper.cpp
//! 2. Download models to `~/.hainet/models/`
//! 3. Set WHISPER_CPP_PATH environment variable (or it will search PATH)

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use tokio::fs;

/// Whisper model configuration
#[derive(Debug, Clone)]
pub struct WhisperConfig {
    /// Path to whisper.cpp binary
    pub whisper_binary: PathBuf,
    
    /// Path to model weights
    pub model_path: PathBuf,
    
    /// Language code ("auto" for automatic detection, "en", "es", etc.)
    pub language: String,
    
    /// Enable timestamp generation
    pub timestamps: bool,
    
    /// Number of threads for inference (0 = auto)
    pub threads: usize,
    
    /// Enable translation to English (only for non-English audio)
    pub translate: bool,
}

impl Default for WhisperConfig {
    fn default() -> Self {
        let home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        
        // Try to find whisper.cpp binary
        let whisper_binary = std::env::var("WHISPER_CPP_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("whisper"));
        
        Self {
            whisper_binary,
            model_path: home_dir.join(".hainet/models/ggml-base.en.bin"),
            language: "en".to_string(),
            timestamps: false,
            threads: 0,
            translate: false,
        }
    }
}

/// Result of speech-to-text transcription
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionResult {
    /// Transcribed text
    pub text: String,
    
    /// Confidence score (0.0 - 1.0)
    pub confidence: f32,
    
    /// Detected language code (e.g., "en", "es")
    pub language: String,
    
    /// Processing time in milliseconds
    pub processing_time_ms: u64,
    
    /// Optional timestamp segments
    pub segments: Option<Vec<TranscriptionSegment>>,
}

/// Timestamped segment of transcription
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionSegment {
    /// Start time in seconds
    pub start: f32,
    
    /// End time in seconds
    pub end: f32,
    
    /// Segment text
    pub text: String,
    
    /// Segment confidence
    pub confidence: f32,
}

/// Speech-to-Text engine
pub struct SpeechToText {
    config: WhisperConfig,
}

impl SpeechToText {
    /// Create a new STT engine with default configuration
    pub fn new() -> Result<Self> {
        Self::with_config(WhisperConfig::default())
    }
    
    /// Create with custom configuration
    pub fn with_config(config: WhisperConfig) -> Result<Self> {
        // Verify whisper.cpp is available
        if !config.whisper_binary.exists() && !Self::is_in_path(&config.whisper_binary) {
            tracing::warn!(
                "whisper.cpp binary not found at {:?}. \\\n                Transcription will fail until whisper.cpp is installed. \\\n                Install from: https://github.com/ggerganov/whisper.cpp",
                config.whisper_binary
            );
        }
        
        if !config.model_path.exists() {
            tracing::warn!(
                "Whisper model not found at {:?}. \\\n                Download models from: https://huggingface.co/ggerganov/whisper.cpp",
                config.model_path
            );
        }
        
        Ok(Self { config })
    }
    
    /// Check if binary exists in PATH
    fn is_in_path(binary: &PathBuf) -> bool {
        Command::new(binary)
            .arg("--version")
            .output()
            .is_ok()
    }
    
    /// Transcribe audio data (WAV format, 16kHz mono)
    pub async fn transcribe(&self, audio_wav: &[u8]) -> Result<TranscriptionResult> {
        let start_time = std::time::Instant::now();
        
        // Write audio to temporary file
        let temp_dir = std::env::temp_dir();
        let temp_audio = temp_dir.join(format!("hainet_stt_{}.wav", uuid::Uuid::new_v4()));
        
        fs::write(&temp_audio, audio_wav)
            .await
            .context("Failed to write temporary audio file")?;
        
        // Run whisper.cpp
        let output = self.run_whisper(&temp_audio).await?;
        
        // Clean up
        let _ = fs::remove_file(&temp_audio).await;
        
        let processing_time_ms = start_time.elapsed().as_millis() as u64;
        
        // Parse output
        let text = self.parse_whisper_output(&output)?;
        
        Ok(TranscriptionResult {
            text,
            confidence: 0.95, // whisper.cpp doesn't provide confidence scores easily
            language: self.config.language.clone(),
            processing_time_ms,
            segments: None, // TODO: Parse segments from whisper.cpp output
        })
    }
    
    /// Run whisper.cpp process
    async fn run_whisper(&self, audio_path: &PathBuf) -> Result<String> {
        let mut cmd = Command::new(&self.config.whisper_binary);
        
        cmd.arg("-m").arg(&self.config.model_path);
        cmd.arg("-f").arg(audio_path);
        
        if self.config.language != "auto" {
            cmd.arg("-l").arg(&self.config.language);
        }
        
        if self.config.threads > 0 {
            cmd.arg("-t").arg(self.config.threads.to_string());
        }
        
        if self.config.translate {
            cmd.arg("--translate");
        }
        
        // Output format
        cmd.arg("-otxt"); // Plain text output
        cmd.arg("-np");   // No print to stdout (we'll read the file)
        
        tracing::debug!("Running whisper.cpp: {:?}", cmd);
        
        let output = cmd.output()
            .context("Failed to run whisper.cpp")?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("whisper.cpp failed: {}", stderr);
        }
        
        // Read the output file
        let output_path = audio_path.with_extension("txt");
        let text = fs::read_to_string(&output_path)
            .await
            .context("Failed to read whisper.cpp output")?;
        
        // Clean up output file
        let _ = fs::remove_file(&output_path).await;
        
        Ok(text)
    }
    
    /// Parse whisper.cpp output
    fn parse_whisper_output(&self, output: &str) -> Result<String> {
        // whisper.cpp output is already plain text
        // Just trim and return
        let text = output.trim().to_string();
        
        if text.is_empty() {
            tracing::warn!("Whisper produced empty transcription");
        }
        
        Ok(text)
    }
    
    /// Transcribe with language detection
    pub async fn transcribe_auto_detect(&self, audio_wav: &[u8]) -> Result<TranscriptionResult> {
        let mut config = self.config.clone();
        config.language = "auto".to_string();
        
        let stt = Self::with_config(config)?;
        stt.transcribe(audio_wav).await
    }
    
    /// Get configuration
    pub fn config(&self) -> &WhisperConfig {
        &self.config
    }
    
    /// Update configuration
    pub fn set_config(&mut self, config: WhisperConfig) {
        self.config = config;
    }
    
    /// Check if STT is ready (whisper.cpp available)
    pub fn is_ready(&self) -> bool {
        self.config.whisper_binary.exists() || Self::is_in_path(&self.config.whisper_binary)
    }
}

impl Default for SpeechToText {
    fn default() -> Self {
        Self::new().expect("Failed to create default STT engine")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_whisper_config_default() {
        let config = WhisperConfig::default();
        assert_eq!(config.language, "en");
        assert_eq!(config.threads, 0);
        assert!(!config.timestamps);
        assert!(!config.translate);
    }
    
    #[test]
    fn test_stt_creation() {
        let stt = SpeechToText::new();
        assert!(stt.is_ok());
    }
    
    #[test]
    fn test_transcription_result_serialization() {
        let result = TranscriptionResult {
            text: "Hello world".to_string(),
            confidence: 0.95,
            language: "en".to_string(),
            processing_time_ms: 123,
            segments: None,
        };
        
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: TranscriptionResult = serde_json::from_str(&json).unwrap();
        
        assert_eq!(deserialized.text, "Hello world");
        assert_eq!(deserialized.confidence, 0.95);
    }
    
    #[test]
    fn test_segment_serialization() {
        let segment = TranscriptionSegment {
            start: 0.0,
            end: 2.5,
            text: "Hello".to_string(),
            confidence: 0.98,
        };
        
        let json = serde_json::to_string(&segment).unwrap();
        let deserialized: TranscriptionSegment = serde_json::from_str(&json).unwrap();
        
        assert_eq!(deserialized.text, "Hello");
        assert!((deserialized.start - 0.0).abs() < 0.001);
    }
}
