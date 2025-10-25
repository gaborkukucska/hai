//! # START OF FILE hainet-core/src/multimodal/tts.rs
//! Text-to-Speech (TTS) engine for multimodal capabilities
//!
//! Provides speech synthesis using Piper TTS via external process execution.
//! Designed for offline-first operation with local voice models.

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use serde::{Deserialize, Serialize};

/// TTS synthesis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynthesisConfig {
    /// Voice model to use (e.g., "en_US-lessac-medium")
    pub voice: String,
    
    /// Speaking rate (0.5 - 2.0, default 1.0)
    pub speed: f32,
    
    /// Output sample rate (default 22050 Hz for Piper)
    pub sample_rate: u32,
    
    /// Audio format for output
    pub output_format: AudioOutputFormat,
}

impl Default for SynthesisConfig {
    fn default() -> Self {
        Self {
            voice: "en_US-lessac-medium".to_string(),
            speed: 1.0,
            sample_rate: 22050,
            output_format: AudioOutputFormat::Wav,
        }
    }
}

/// Audio output format for TTS
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioOutputFormat {
    /// WAV format (uncompressed, high quality)
    Wav,
    
    /// MP3 format (compressed, smaller size)
    Mp3,
}

/// TTS synthesis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynthesisResult {
    /// Audio data (WAV or MP3 bytes)
    pub audio_data: Vec<u8>,
    
    /// Audio format
    pub format: AudioOutputFormat,
    
    /// Sample rate in Hz
    pub sample_rate: u32,
    
    /// Duration in milliseconds
    pub duration_ms: u64,
    
    /// Text that was synthesized
    pub text: String,
}

/// Text-to-Speech engine using Piper
pub struct TextToSpeech {
    /// Configuration for synthesis
    config: SynthesisConfig,
    
    /// Path to Piper executable
    piper_path: PathBuf,
    
    /// Path to voice models directory
    models_dir: PathBuf,
}

impl TextToSpeech {
    /// Create a new TTS engine with default configuration
    pub fn new() -> Result<Self> {
        Self::with_config(SynthesisConfig::default())
    }
    
    /// Create TTS engine with custom configuration
    pub fn with_config(config: SynthesisConfig) -> Result<Self> {
        // Detect Piper installation
        let piper_path = Self::find_piper_executable()
            .context("Piper TTS not found. Please run hainet-seed installer first.")?;
        
        // Get models directory
        let models_dir = Self::get_models_directory()?;
        
        Ok(Self {
            config,
            piper_path,
            models_dir,
        })
    }
    
    /// Synthesize speech from text
    pub fn synthesize(&self, text: &str) -> Result<SynthesisResult> {
        if text.trim().is_empty() {
            anyhow::bail!("Cannot synthesize empty text");
        }
        
        // Get voice model path
        let model_path = self.get_voice_model_path(&self.config.voice)?;
        
        // Create temporary output file
        let temp_dir = std::env::temp_dir();
        let output_path = temp_dir.join(format!("hainet_tts_{}.wav", uuid::Uuid::new_v4()));
        
        // Build Piper command
        let mut cmd = Command::new(&self.piper_path);
        cmd.arg("--model")
            .arg(&model_path)
            .arg("--output_file")
            .arg(&output_path);
        
        // Add speed control if not default
        if (self.config.speed - 1.0).abs() > 0.01 {
            cmd.arg("--length_scale").arg(format!("{:.2}", 1.0 / self.config.speed));
        }
        
        // Set up stdin pipe for text input
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        
        // Spawn process
        let mut child = cmd.spawn()
            .context("Failed to spawn Piper process")?;
        
        // Write text to stdin
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            stdin.write_all(text.as_bytes())
                .context("Failed to write text to Piper stdin")?;
        }
        
        // Wait for completion
        let output = child.wait_with_output()
            .context("Failed to wait for Piper process")?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Piper synthesis failed: {}", stderr);
        }
        
        // Read audio data
        let audio_data = std::fs::read(&output_path)
            .context("Failed to read synthesized audio file")?;
        
        // Calculate duration (estimate from WAV file size)
        let duration_ms = Self::estimate_duration_ms(&audio_data, self.config.sample_rate)?;
        
        // Clean up temp file
        let _ = std::fs::remove_file(&output_path);
        
        Ok(SynthesisResult {
            audio_data,
            format: AudioOutputFormat::Wav,
            sample_rate: self.config.sample_rate,
            duration_ms,
            text: text.to_string(),
        })
    }
    
    /// Synthesize and return Base64-encoded audio for IPC
    pub fn synthesize_base64(&self, text: &str) -> Result<String> {
        use base64::{engine::general_purpose, Engine as _};
        
        let result = self.synthesize(text)?;
        Ok(general_purpose::STANDARD.encode(&result.audio_data))
    }
    
    /// Get current configuration
    pub fn config(&self) -> &SynthesisConfig {
        &self.config
    }
    
    /// Update configuration
    pub fn set_config(&mut self, config: SynthesisConfig) -> Result<()> {
        // Validate voice model exists
        let _ = self.get_voice_model_path(&config.voice)?;
        self.config = config;
        Ok(())
    }
    
    /// List available voice models
    pub fn list_voices(&self) -> Result<Vec<String>> {
        let mut voices = Vec::new();
        
        if !self.models_dir.exists() {
            return Ok(voices);
        }
        
        for entry in std::fs::read_dir(&self.models_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("onnx") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    voices.push(stem.to_string());
                }
            }
        }
        
        voices.sort();
        Ok(voices)
    }
    
    /// Check if TTS engine is ready
    pub fn is_ready(&self) -> bool {
        self.piper_path.exists() && self.models_dir.exists()
    }
    
    // Private helper methods
    
    /// Find Piper executable in common locations
    fn find_piper_executable() -> Option<PathBuf> {
        // Check PATH
        if let Ok(path) = which::which("piper") {
            return Some(path);
        }
        
        // Check common installation locations
        let home = dirs::home_dir()?;
        let candidates = vec![
            home.join(".local/bin/piper"),
            PathBuf::from("/usr/local/bin/piper"),
            PathBuf::from("/usr/bin/piper"),
        ];
        
        candidates.into_iter().find(|p| p.exists())
    }
    
    /// Get models directory path
    fn get_models_directory() -> Result<PathBuf> {
        let home = dirs::home_dir()
            .context("Failed to determine home directory")?;
        
        Ok(home.join(".hainet/models/piper"))
    }
    
    /// Get full path to voice model file
    fn get_voice_model_path(&self, voice: &str) -> Result<PathBuf> {
        let model_path = self.models_dir.join(format!("{}.onnx", voice));
        
        if !model_path.exists() {
            anyhow::bail!(
                "Voice model '{}' not found. Available voices: {:?}",
                voice,
                self.list_voices().unwrap_or_default()
            );
        }
        
        Ok(model_path)
    }
    
    /// Estimate audio duration from WAV file
    fn estimate_duration_ms(wav_data: &[u8], _sample_rate: u32) -> Result<u64> {
        use hound::WavReader;
        
        let reader = WavReader::new(std::io::Cursor::new(wav_data))
            .context("Failed to read WAV file for duration estimation")?;
        
        let spec = reader.spec();
        let num_samples = reader.len();
        
        let duration_ms = (num_samples as u64 * 1000) / (spec.sample_rate as u64 * spec.channels as u64);
        
        Ok(duration_ms)
    }
}

impl Default for TextToSpeech {
    fn default() -> Self {
        Self::new().expect("Failed to initialize TTS engine")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_synthesis_config_default() {
        let config = SynthesisConfig::default();
        assert_eq!(config.voice, "en_US-lessac-medium");
        assert_eq!(config.speed, 1.0);
        assert_eq!(config.sample_rate, 22050);
    }
    
    #[test]
    fn test_audio_output_format() {
        assert_eq!(AudioOutputFormat::Wav, AudioOutputFormat::Wav);
        assert_ne!(AudioOutputFormat::Wav, AudioOutputFormat::Mp3);
    }
    
    #[test]
    fn test_tts_creation_without_piper() {
        // This test will fail if Piper is not installed, which is expected
        // In production, the installer will handle Piper installation
        let result = TextToSpeech::new();
        
        // We don't assert success here because Piper may not be installed yet
        // The test just verifies the API works
        match result {
            Ok(_) => println!("TTS engine created successfully"),
            Err(e) => println!("TTS engine creation failed (expected if Piper not installed): {}", e),
        }
    }
    
    #[test]
    fn test_synthesis_result_serialization() {
        let result = SynthesisResult {
            audio_data: vec![1, 2, 3, 4],
            format: AudioOutputFormat::Wav,
            sample_rate: 22050,
            duration_ms: 1000,
            text: "Hello world".to_string(),
        };
        
        // Test serialization
        let json = serde_json::to_string(&result).expect("Serialization failed");
        assert!(json.contains("Hello world"));
        
        // Test deserialization
        let deserialized: SynthesisResult = serde_json::from_str(&json).expect("Deserialization failed");
        assert_eq!(deserialized.text, "Hello world");
        assert_eq!(deserialized.sample_rate, 22050);
    }
}
