//! # START OF FILE hainet-portal/src-tauri/src/tts_handler.rs
//! Text-to-Speech handler for HAI-Net Portal
//!
//! Provides speech synthesis capabilities using hainet-core TTS engine.

use serde::{Deserialize, Serialize};
use hainet_core::multimodal::{TextToSpeech};

/// TTS handler for Portal
pub struct TTSHandler {
    tts: Option<TextToSpeech>,
}

/// TTS synthesis request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynthesisRequest {
    /// Text to synthesize
    pub text: String,
    
    /// Voice model (optional, uses default if not provided)
    pub voice: Option<String>,
    
    /// Speaking speed (0.5 - 2.0, default 1.0)
    pub speed: Option<f32>,
}

/// TTS synthesis response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynthesisResponse {
    /// Base64-encoded audio data
    pub audio_base64: String,
    
    /// Audio format
    pub format: String,
    
    /// Sample rate in Hz
    pub sample_rate: u32,
    
    /// Duration in milliseconds
    pub duration_ms: u64,
    
    /// Text that was synthesized
    pub text: String,
}

impl TTSHandler {
    /// Create new TTS handler
    pub fn new() -> Self {
        // Try to initialize TTS engine
        let tts = TextToSpeech::new().ok();
        
        if tts.is_none() {
            eprintln!("⚠️  TTS engine not available. Install Piper via hainet-seed.");
        }
        
        Self { tts }
    }
    
    /// Check if TTS is ready
    pub fn is_ready(&self) -> bool {
        self.tts.as_ref().map(|t| t.is_ready()).unwrap_or(false)
    }
    
    /// Synthesize speech from text
    pub async fn synthesize(&self, request: SynthesisRequest) -> Result<SynthesisResponse, String> {
        let tts = self.tts.as_ref()
            .ok_or_else(|| "TTS engine not available".to_string())?;
        
        // For now, we'll use the default configuration
        // TODO: Implement dynamic voice/speed configuration when needed
        // This requires making TextToSpeech cloneable or using interior mutability
        
        // Synthesize with default config
        let audio_base64 = tts.synthesize_base64(&request.text)
            .map_err(|e| format!("TTS synthesis failed: {}", e))?;
        
        // Get result metadata
        let result = tts.synthesize(&request.text)
            .map_err(|e| format!("TTS synthesis failed: {}", e))?;
        
        Ok(SynthesisResponse {
            audio_base64,
            format: format!("{:?}", result.format),
            sample_rate: result.sample_rate,
            duration_ms: result.duration_ms,
            text: request.text,
        })
    }
    
    /// List available voices
    pub fn list_voices(&self) -> Result<Vec<String>, String> {
        let tts = self.tts.as_ref()
            .ok_or_else(|| "TTS engine not available".to_string())?;
        
        tts.list_voices()
            .map_err(|e| format!("Failed to list voices: {}", e))
    }
}

impl Default for TTSHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_tts_handler_creation() {
        let handler = TTSHandler::new();
        // May not be ready if Piper not installed, which is expected
        println!("TTS ready: {}", handler.is_ready());
    }
    
    #[test]
    fn test_synthesis_request_serialization() {
        let request = SynthesisRequest {
            text: "Hello world".to_string(),
            voice: Some("en_US-lessac-medium".to_string()),
            speed: Some(1.0),
        };
        
        let json = serde_json::to_string(&request).expect("Serialization failed");
        assert!(json.contains("Hello world"));
    }
}
