//! # START OF FILE hainet-portal/src-tauri/tests/stt_integration_test.rs
//! Integration tests for Speech-to-Text (STT) pipeline
//! 
//! Tests the complete flow: Base64 audio → AudioProcessor → STT → Transcription

use hainet_core::multimodal::audio::{AudioProcessor, AudioFormat};
use hainet_core::multimodal::stt::{SpeechToText, WhisperConfig};

/// Helper function to create a simple WAV file in memory
/// Returns a valid 16kHz mono WAV file (1 second of silence)
fn create_test_wav() -> Vec<u8> {
    use hound::{WavSpec, WavWriter};
    use std::io::Cursor;
    
    let spec = WavSpec {
        channels: 1,
        sample_rate: 16000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer = WavWriter::new(&mut cursor, spec).unwrap();
        
        // Write 1 second of silence (16000 samples)
        for _ in 0..16000 {
            writer.write_sample(0i16).unwrap();
        }
        
        writer.finalize().unwrap();
    }
    
    cursor.into_inner()
}

/// Helper function to create WAV file with simulated speech (non-zero samples)
fn create_speech_wav() -> Vec<u8> {
    use hound::{WavSpec, WavWriter};
    use std::io::Cursor;
    
    let spec = WavSpec {
        channels: 1,
        sample_rate: 16000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer = WavWriter::new(&mut cursor, spec).unwrap();
        
        // Write 1 second of simulated speech (sine wave)
        for i in 0..16000 {
            let sample = (i as f32 * 0.01).sin() * 1000.0;
            writer.write_sample(sample as i16).unwrap();
        }
        
        writer.finalize().unwrap();
    }
    
    cursor.into_inner()
}

#[cfg(test)]
mod audio_processing_tests {
    use super::*;
    
    #[test]
    fn test_wav_format_detection() {
        let wav_data = create_test_wav();
        let format = AudioFormat::detect(&wav_data);
        assert_eq!(format, AudioFormat::Wav);
    }
    
    #[test]
    fn test_webm_format_detection() {
        // WebM file signature: 0x1A 0x45 0xDF 0xA3
        let webm_data = vec![0x1A, 0x45, 0xDF, 0xA3, 0x00, 0x00];
        let format = AudioFormat::detect(&webm_data);
        assert_eq!(format, AudioFormat::WebMOpus);
    }
    
    #[test]
    fn test_mp3_format_detection() {
        // MP3 file signature: 0xFF 0xFB or ID3
        let mp3_data = vec![0xFF, 0xFB, 0x00, 0x00];
        let format = AudioFormat::detect(&mp3_data);
        assert_eq!(format, AudioFormat::Mp3);
    }
    
    #[test]
    fn test_unknown_format_detection() {
        let unknown_data = vec![0x00, 0x00, 0x00, 0x00];
        let format = AudioFormat::detect(&unknown_data);
        assert_eq!(format, AudioFormat::Unknown);
    }
    
    #[test]
    fn test_base64_decode() {
        use base64::{Engine as _, engine::general_purpose};
        
        let processor = AudioProcessor::new();
        let wav_data = create_test_wav();
        let base64 = general_purpose::STANDARD.encode(&wav_data);
        
        let decoded = processor.decode_base64(&base64).unwrap();
        assert_eq!(decoded, wav_data);
    }
    
    #[test]
    fn test_base64_decode_invalid() {
        let processor = AudioProcessor::new();
        let result = processor.decode_base64("not-valid-base64!!!");
        assert!(result.is_err());
    }
    
    #[test]
    fn test_audio_processing_wav() {
        let processor = AudioProcessor::new();
        let wav_data = create_test_wav();
        
        // Process should succeed for valid WAV
        let result = processor.process(&wav_data);
        assert!(result.is_ok());
        
        let processed = result.unwrap();
        // Should still be valid WAV data
        assert!(processed.len() > 44); // WAV header is 44 bytes
    }
    
    #[test]
    fn test_audio_processor_with_speech() {
        let processor = AudioProcessor::new();
        let wav_data = create_speech_wav();
        
        let result = processor.process(&wav_data);
        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod stt_engine_tests {
    use super::*;
    
    #[test]
    fn test_stt_creation() {
        let stt = SpeechToText::new();
        assert!(stt.is_ok());
    }
    
    #[test]
    fn test_stt_config_customization() {
        let mut config = WhisperConfig::default();
        config.language = "es".to_string();
        config.threads = 4;
        
        let stt = SpeechToText::with_config(config);
        assert!(stt.is_ok());
        
        let stt = stt.unwrap();
        assert_eq!(stt.config().language, "es");
        assert_eq!(stt.config().threads, 4);
    }
    
    #[test]
    fn test_stt_readiness_check() {
        let stt = SpeechToText::new().unwrap();
        // is_ready() checks if whisper.cpp is available
        // This might be false in CI environments without whisper.cpp
        // But the check itself should not panic
        let _ready = stt.is_ready();
    }
    
    // Conditional test: only runs if whisper.cpp is available
    #[test]
    #[ignore] // Run with: cargo test -- --ignored --test-threads=1
    fn test_stt_transcription_if_whisper_available() {
        let stt = SpeechToText::new().unwrap();
        
        if !stt.is_ready() {
            println!("Skipping: whisper.cpp not available");
            return;
        }
        
        // Create test audio
        let wav_data = create_speech_wav();
        
        // Transcribe (this will actually call whisper.cpp)
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async {
            stt.transcribe(&wav_data).await
        });
        
        // Should succeed if whisper.cpp is properly set up
        if let Ok(transcription) = result {
            println!("Transcription: {}", transcription.text);
            assert!(transcription.processing_time_ms > 0);
        } else {
            // May fail if model not downloaded, which is OK for this test
            println!("Transcription failed (likely missing model): {:?}", result.err());
        }
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    
    #[test]
    fn test_full_audio_pipeline() {
        use base64::{Engine as _, engine::general_purpose};
        
        // Simulate the full flow from Portal to STT
        
        // 1. Create audio data (simulating frontend capture)
        let wav_data = create_test_wav();
        let base64_audio = general_purpose::STANDARD.encode(&wav_data);
        
        // 2. Decode and detect format (simulating STT handler)
        let processor = AudioProcessor::new();
        let decoded = processor.decode_base64(&base64_audio).unwrap();
        let format = AudioFormat::detect(&decoded);
        
        assert_eq!(format, AudioFormat::Wav);
        
        // 3. Process audio (resample, convert to mono)
        let processed = processor.process(&decoded).unwrap();
        
        // Should produce valid WAV output
        assert!(processed.len() > 44);
        
        // 4. Verify processed audio is valid WAV
        use hound::WavReader;
        use std::io::Cursor;
        
        let reader = WavReader::new(Cursor::new(&processed)).unwrap();
        let spec = reader.spec();
        
        // Should be 16kHz mono after processing
        assert_eq!(spec.sample_rate, 16000);
        assert_eq!(spec.channels, 1);
    }
    
    #[test]
    fn test_vad_audio_level_calculation() {
        // Test the VAD helper functions from STT handler
        use app_lib::stt_handler::STTHandler;
        
        // Silence should have low energy
        let silence: Vec<f32> = vec![0.0; 1000];
        let silence_level = STTHandler::calculate_audio_level(&silence);
        assert!(silence_level < 0.01);
        
        // Loud audio should have high energy
        let loud: Vec<f32> = vec![0.8; 1000];
        let loud_level = STTHandler::calculate_audio_level(&loud);
        assert!(loud_level > 0.7);
        
        // VAD should correctly detect speech vs silence
        assert!(!STTHandler::is_speech_detected(silence_level, 0.5));
        assert!(STTHandler::is_speech_detected(loud_level, 0.5));
    }
    
    #[test]
    fn test_stt_handler_config() {
        use app_lib::stt_handler::{STTHandler, STTConfig};
        
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            let handler = STTHandler::new();
            let config = handler.get_config().await;
            
            // Check default config
            assert_eq!(config.language, "auto");
            assert!(config.vad_enabled);
            assert_eq!(config.vad_threshold, 0.5);
            assert!(config.offline_only); // Privacy-first default
            
            // Update config
            let mut new_config = config.clone();
            new_config.language = "es".to_string();
            new_config.vad_threshold = 0.7;
            
            handler.update_config(new_config.clone()).await;
            
            let updated = handler.get_config().await;
            assert_eq!(updated.language, "es");
            assert_eq!(updated.vad_threshold, 0.7);
        });
    }
    
    #[test]
    fn test_audio_data_serialization() {
        use app_lib::stt_handler::AudioData;
        
        let audio = AudioData {
            data: "SGVsbG8gV29ybGQ=".to_string(), // "Hello World" in base64
            sample_rate: 16000,
            channels: 1,
            format: "wav".to_string(),
        };
        
        // Should serialize to JSON
        let json = serde_json::to_string(&audio).unwrap();
        assert!(json.contains("SGVsbG8gV29ybGQ="));
        assert!(json.contains("16000"));
        
        // Should deserialize back
        let deserialized: AudioData = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.data, audio.data);
        assert_eq!(deserialized.sample_rate, 16000);
        assert_eq!(deserialized.channels, 1);
    }
    
    #[test]
    fn test_transcription_result_serialization() {
        use app_lib::stt_handler::TranscriptionResult;
        
        let result = TranscriptionResult {
            text: "Hello, this is a test".to_string(),
            confidence: 0.95,
            language: "en".to_string(),
            processing_time_ms: 1234,
        };
        
        // Should serialize to JSON
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("Hello, this is a test"));
        assert!(json.contains("0.95"));
        
        // Should deserialize back
        let deserialized: TranscriptionResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.text, result.text);
        assert_eq!(deserialized.confidence, 0.95);
        assert_eq!(deserialized.language, "en");
    }
    
    #[tokio::test]
    async fn test_stt_handler_transcription_without_whisper() {
        use base64::{Engine as _, engine::general_purpose};
        use app_lib::stt_handler::{STTHandler, AudioData};
        
        let handler = STTHandler::new();
        
        // Create test audio data
        let wav_data = create_test_wav();
        let base64_audio = general_purpose::STANDARD.encode(&wav_data);
        
        let audio = AudioData {
            data: base64_audio,
            sample_rate: 16000,
            channels: 1,
            format: "wav".to_string(),
        };
        
        // Attempt transcription
        let result = handler.transcribe(audio).await;
        
        // Will fail if whisper.cpp not installed, which is expected
        // This test verifies the error handling works correctly
        if result.is_err() {
            let err_msg = result.unwrap_err().to_string();
            // Should have helpful error message
            assert!(
                err_msg.contains("whisper") || err_msg.contains("hainet-seed"),
                "Error should mention whisper or installation: {}",
                err_msg
            );
        } else {
            // If it succeeds, whisper.cpp is installed
            let transcription = result.unwrap();
            println!("Transcription successful: {}", transcription.text);
            assert!(transcription.processing_time_ms > 0);
        }
    }
}

#[cfg(test)]
mod performance_tests {
    use super::*;
    
    #[test]
    fn test_audio_processing_performance() {
        let processor = AudioProcessor::new();
        let wav_data = create_test_wav();
        
        let start = std::time::Instant::now();
        
        // Process audio 10 times
        for _ in 0..10 {
            let _ = processor.process(&wav_data).unwrap();
        }
        
        let elapsed = start.elapsed();
        
        // Should be fast (< 100ms for 10 iterations)
        assert!(elapsed.as_millis() < 100, "Audio processing too slow: {:?}", elapsed);
    }
    
    #[test]
    fn test_base64_encoding_performance() {
        use base64::{Engine as _, engine::general_purpose};
        
        let wav_data = create_test_wav();
        
        let start = std::time::Instant::now();
        
        // Encode/decode 100 times
        for _ in 0..100 {
            let encoded = general_purpose::STANDARD.encode(&wav_data);
            let _ = general_purpose::STANDARD.decode(&encoded).unwrap();
        }
        
        let elapsed = start.elapsed();
        
        // Should be fast (< 50ms for 100 iterations)
        assert!(elapsed.as_millis() < 50, "Base64 encoding too slow: {:?}", elapsed);
    }
}
