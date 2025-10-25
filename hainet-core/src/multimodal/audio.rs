//! # START OF FILE hainet-core/src/multimodal/audio.rs
//! Audio processing utilities for multimodal capabilities
//!
//! Handles audio format detection, conversion, and preprocessing
//! for Speech-to-Text (STT) processing.

use anyhow::{Context, Result};
use std::io::Cursor;

/// Supported audio formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFormat {
    /// WebM container with Opus codec (Chrome/Firefox)
    WebMOpus,
    
    /// WAV format (universal, uncompressed)
    Wav,
    
    /// MP3 format (compressed, older browsers)
    Mp3,
    
    /// Unknown/unsupported format
    Unknown,
}

impl AudioFormat {
    /// Detect audio format from raw bytes using magic numbers
    pub fn detect(data: &[u8]) -> Self {
        if data.len() < 12 {
            return Self::Unknown;
        }
        
        // WAV: "RIFF" + size + "WAVE"
        if &data[0..4] == b"RIFF" && &data[8..12] == b"WAVE" {
            return Self::Wav;
        }
        
        // WebM: 0x1A 0x45 0xDF 0xA3 (EBML header)
        if data[0] == 0x1A && data[1] == 0x45 && data[2] == 0xDF && data[3] == 0xA3 {
            return Self::WebMOpus;
        }
        
        // MP3: 0xFF 0xFB or 0xFF 0xF3 or "ID3"
        if (data[0] == 0xFF && (data[1] == 0xFB || data[1] == 0xF3)) || &data[0..3] == b"ID3" {
            return Self::Mp3;
        }
        
        Self::Unknown
    }
    
    /// Get human-readable format name
    pub fn name(&self) -> &'static str {
        match self {
            Self::WebMOpus => "WebM/Opus",
            Self::Wav => "WAV",
            Self::Mp3 => "MP3",
            Self::Unknown => "Unknown",
        }
    }
}

/// Audio processor for format conversion and preprocessing
pub struct AudioProcessor {
    /// Target sample rate for Whisper (16kHz)
    target_sample_rate: u32,
    
    /// Target channels (mono)
    target_channels: u16,
}

impl AudioProcessor {
    /// Create a new audio processor with default Whisper settings
    pub fn new() -> Self {
        Self {
            target_sample_rate: 16000, // Whisper requirement
            target_channels: 1,          // Mono
        }
    }
    
    /// Create with custom settings
    pub fn with_settings(sample_rate: u32, channels: u16) -> Self {
        Self {
            target_sample_rate: sample_rate,
            target_channels: channels,
        }
    }
    
    /// Decode Base64 audio data
    pub fn decode_base64(&self, base64_data: &str) -> Result<Vec<u8>> {
        use base64::{engine::general_purpose, Engine as _};
        
        general_purpose::STANDARD
            .decode(base64_data)
            .context("Failed to decode Base64 audio data")
    }
    
    /// Process audio: detect format, convert to WAV if needed, resample to 16kHz mono
    pub fn process(&self, audio_data: &[u8]) -> Result<Vec<u8>> {
        let format = AudioFormat::detect(audio_data);
        
        match format {
            AudioFormat::Wav => {
                // Already WAV, just verify/resample if needed
                self.process_wav(audio_data)
            }
            AudioFormat::WebMOpus | AudioFormat::Mp3 => {
                // Decode WebM/Opus or MP3 using symphonia
                self.decode_with_symphonia(audio_data, format)
            }
            AudioFormat::Unknown => {
                anyhow::bail!("Unknown or unsupported audio format")
            }
        }
    }
    
    /// Process WAV audio (verify format, resample if needed)
    fn process_wav(&self, wav_data: &[u8]) -> Result<Vec<u8>> {
        use hound::{WavReader, WavWriter};
        
        // Read WAV file
        let mut reader = WavReader::new(Cursor::new(wav_data))
            .context("Failed to read WAV file")?;
        
        let spec = reader.spec();
        
        // Check if we need to resample/convert
        let needs_conversion = spec.sample_rate != self.target_sample_rate
            || spec.channels != self.target_channels;
        
        if !needs_conversion {
            // Already in target format, return as-is
            return Ok(wav_data.to_vec());
        }
        
        // Convert to target format
        let samples: Vec<i16> = if spec.bits_per_sample == 16 {
            reader.samples::<i16>().collect::<Result<Vec<_>, _>>()?
        } else {
            // Convert from other bit depths to 16-bit
            reader
                .samples::<f32>()
                .map(|s| s.map(|sample| (sample * 32767.0) as i16))
                .collect::<Result<Vec<_>, _>>()?
        };
        
        // Resample and/or convert channels
        let processed_samples = self.resample_and_convert_channels(
            &samples,
            spec.sample_rate,
            spec.channels,
        )?;
        
        // Write to new WAV buffer
        let mut output = Cursor::new(Vec::new());
        let mut writer = WavWriter::new(
            &mut output,
            hound::WavSpec {
                channels: self.target_channels,
                sample_rate: self.target_sample_rate,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )?;
        
        for sample in processed_samples {
            writer.write_sample(sample)?;
        }
        
        writer.finalize()?;
        Ok(output.into_inner())
    }
    
    /// Resample and convert channels (basic implementation)
    fn resample_and_convert_channels(
        &self,
        samples: &[i16],
        source_sample_rate: u32,
        source_channels: u16,
    ) -> Result<Vec<i16>> {
        // Convert to mono if needed (average channels)
        let mono_samples: Vec<i16> = if source_channels == 1 {
            samples.to_vec()
        } else {
            samples
                .chunks(source_channels as usize)
                .map(|chunk| {
                    let sum: i32 = chunk.iter().map(|&s| s as i32).sum();
                    (sum / chunk.len() as i32) as i16
                })
                .collect()
        };
        
        // Resample if needed (simple linear interpolation)
        if source_sample_rate == self.target_sample_rate {
            Ok(mono_samples)
        } else {
            Ok(self.resample_linear(&mono_samples, source_sample_rate))
        }
    }
    
    /// Simple linear resampling (good enough for speech)
    fn resample_linear(&self, samples: &[i16], source_rate: u32) -> Vec<i16> {
        let ratio = source_rate as f64 / self.target_sample_rate as f64;
        let target_len = (samples.len() as f64 / ratio).ceil() as usize;
        
        let mut resampled = Vec::with_capacity(target_len);
        
        for i in 0..target_len {
            let source_idx = i as f64 * ratio;
            let idx0 = source_idx.floor() as usize;
            let idx1 = (idx0 + 1).min(samples.len() - 1);
            let frac = source_idx - idx0 as f64;
            
            let sample0 = samples[idx0] as f64;
            let sample1 = samples[idx1] as f64;
            
            let interpolated = sample0 + (sample1 - sample0) * frac;
            resampled.push(interpolated as i16);
        }
        
        resampled
    }
    
    /// Decode WebM/Opus or MP3 using Symphonia, then convert to WAV
    fn decode_with_symphonia(&self, audio_data: &[u8], _format: AudioFormat) -> Result<Vec<u8>> {
        use symphonia::core::audio::SampleBuffer;
        use symphonia::core::codecs::DecoderOptions;
        use symphonia::core::formats::FormatOptions;
        use symphonia::core::io::MediaSourceStream;
        use symphonia::core::meta::MetadataOptions;
        use symphonia::core::probe::Hint;
        
        // Create media source from byte array (need owned copy for 'static lifetime)
        let audio_data_owned = audio_data.to_vec();
        let mss = MediaSourceStream::new(Box::new(Cursor::new(audio_data_owned)), Default::default());
        
        // Probe format
        let hint = Hint::new();
        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
            .context("Failed to probe audio format")?;
        
        let mut format = probed.format;
        
        // Get default track
        let track = format
            .default_track()
            .context("No default audio track found")?;
        
        // Create decoder
        let mut decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &DecoderOptions::default())
            .context("Failed to create decoder")?;
        
        // Get track parameters
        let track_id = track.id;
        let codec_params = decoder.codec_params();
        let channels = codec_params.channels.map(|c| c.count()).unwrap_or(1) as u16;
        let sample_rate = codec_params.sample_rate.unwrap_or(self.target_sample_rate);
        
        // Decode all packets into samples
        let mut samples = Vec::new();
        
        loop {
            // Get next packet
            let packet = match format.next_packet() {
                Ok(p) => p,
                Err(_) => break, // End of stream
            };
            
            // Skip packets not from the selected track
            if packet.track_id() != track_id {
                continue;
            }
            
            // Decode packet
            match decoder.decode(&packet) {
                Ok(decoded) => {
                    // Convert to i16 samples
                    let mut sample_buf = SampleBuffer::<i16>::new(
                        decoded.capacity() as u64,
                        *decoded.spec(),
                    );
                    sample_buf.copy_interleaved_ref(decoded);
                    samples.extend_from_slice(sample_buf.samples());
                }
                Err(_) => continue, // Skip errored packets
            }
        }
        
        if samples.is_empty() {
            anyhow::bail!("No audio samples decoded from input");
        }
        
        // Resample and convert channels
        let processed_samples = self.resample_and_convert_channels(
            &samples,
            sample_rate,
            channels,
        )?;
        
        // Write to WAV buffer
        let mut output = Cursor::new(Vec::new());
        let mut writer = hound::WavWriter::new(
            &mut output,
            hound::WavSpec {
                channels: self.target_channels,
                sample_rate: self.target_sample_rate,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )?;
        
        for sample in processed_samples {
            writer.write_sample(sample)?;
        }
        
        writer.finalize()?;
        Ok(output.into_inner())
    }
}

impl Default for AudioProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_format_detection_wav() {
        // WAV magic number: "RIFF" + size + "WAVE"
        let wav_data = b"RIFF\x00\x00\x00\x00WAVE";
        assert_eq!(AudioFormat::detect(wav_data), AudioFormat::Wav);
    }
    
    #[test]
    fn test_format_detection_webm() {
        // WebM EBML header
        let webm_data = b"\x1A\x45\xDF\xA3\x00\x00\x00\x00\x00\x00\x00\x00";
        assert_eq!(AudioFormat::detect(webm_data), AudioFormat::WebMOpus);
    }
    
    #[test]
    fn test_format_detection_mp3() {
        // MP3 sync word
        let mp3_data = b"\xFF\xFB\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";
        assert_eq!(AudioFormat::detect(mp3_data), AudioFormat::Mp3);
    }
    
    #[test]
    fn test_format_detection_unknown() {
        let unknown_data = b"UNKNOWN_FORMAT_DATA";
        assert_eq!(AudioFormat::detect(unknown_data), AudioFormat::Unknown);
    }
    
    #[test]
    fn test_format_names() {
        assert_eq!(AudioFormat::Wav.name(), "WAV");
        assert_eq!(AudioFormat::WebMOpus.name(), "WebM/Opus");
        assert_eq!(AudioFormat::Mp3.name(), "MP3");
        assert_eq!(AudioFormat::Unknown.name(), "Unknown");
    }
    
    #[test]
    fn test_audio_processor_creation() {
        let processor = AudioProcessor::new();
        assert_eq!(processor.target_sample_rate, 16000);
        assert_eq!(processor.target_channels, 1);
    }
    
    #[test]
    fn test_base64_decode() {
        let processor = AudioProcessor::new();
        let base64 = "SGVsbG8gV29ybGQh"; // "Hello World!"
        let decoded = processor.decode_base64(base64).unwrap();
        assert_eq!(decoded, b"Hello World!");
    }
}
