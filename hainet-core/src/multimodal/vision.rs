//! hainet-core/src/multimodal/vision.rs
//! Vision capabilities for HAI-Net (Webcam Input)

use std::sync::{Arc, Mutex};
use image::{RgbaImage};
use nokhwa::{
    pixel_format::RgbAFormat,
    query,
    utils::{ApiBackend, CameraIndex, RequestedFormat, RequestedFormatType},
    CallbackCamera,
};
use serde::{Deserialize, Serialize};

/// Represents the core vision system for webcam management and frame capture.
pub struct VisionSystem {
    config: VisionConfig,
    camera: Arc<Mutex<Option<CallbackCamera>>>,
    last_frame: Arc<Mutex<Option<RgbaImage>>>,
    status: Arc<Mutex<WebcamStatus>>,
}

/// Configuration for the Vision System.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionConfig {
    pub device_index: usize,
    pub resolution_width: u32,
    pub resolution_height: u32,
    pub frame_rate: u32,
    pub privacy_mode: PrivacyMode,
}

/// Represents the current status of the webcam.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebcamStatus {
    Idle,
    Capturing,
    Error(VisionError),
}

/// Defines privacy controls for webcam capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrivacyMode {
    Off,
    Blur,
}

/// Represents the result of analyzing a single frame.
#[derive(Debug, Clone, Serialize)]
pub struct FrameAnalysisResult {
    pub objects_detected: Vec<String>,
    pub ocr_text: String,
    pub gesture: String,
    pub emotional_valence: f32,
}

/// Custom error types for the vision system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisionError {
    DeviceNotFound,
    CaptureFailed,
    AlreadyCapturing,
    NotCapturing,
    PermissionDenied,
}

impl VisionSystem {
    pub fn new(config: VisionConfig) -> Self {
        Self {
            config,
            camera: Arc::new(Mutex::new(None)),
            last_frame: Arc::new(Mutex::new(None)),
            status: Arc::new(Mutex::new(WebcamStatus::Idle)),
        }
    }

    pub fn list_devices() -> Result<Vec<String>, nokhwa::NokhwaError> {
        let cameras = query(ApiBackend::Auto)?;
        Ok(cameras.iter().map(|cam| cam.human_name()).collect())
    }

    pub fn start_capture(&self) -> Result<(), VisionError> {
        let mut status = self.status.lock().unwrap();
        if *status == WebcamStatus::Capturing {
            return Err(VisionError::AlreadyCapturing);
        }

        let mut camera_lock = self.camera.lock().unwrap();
        let format = RequestedFormat::new::<RgbAFormat>(RequestedFormatType::AbsoluteHighestFrameRate);

        let last_frame = self.last_frame.clone();
        let callback = move |buffer: nokhwa::Buffer| {
            if let Ok(frame) = buffer.decode_image::<RgbAFormat>() {
                let mut last_frame_lock = last_frame.lock().unwrap();
                *last_frame_lock = Some(frame);
            }
        };

        let camera_result = CallbackCamera::new(CameraIndex::Index(self.config.device_index as u32), format, callback);

        match camera_result {
            Ok(mut cam) => {
                cam.open_stream().map_err(|_| VisionError::CaptureFailed)?;
                *camera_lock = Some(cam);
                *status = WebcamStatus::Capturing;
                Ok(())
            }
            Err(_) => {
                *status = WebcamStatus::Error(VisionError::DeviceNotFound);
                Err(VisionError::DeviceNotFound)
            }
        }
    }

    pub fn stop_capture(&self) {
        let mut camera_lock = self.camera.lock().unwrap();
        if let Some(mut cam) = camera_lock.take() {
            cam.stop_stream().unwrap_or_default();
        }
        let mut status = self.status.lock().unwrap();
        *status = WebcamStatus::Idle;
    }

    pub fn capture_frame(&self) -> Result<RgbaImage, VisionError> {
        let last_frame_lock = self.last_frame.lock().unwrap();
        if let Some(frame) = last_frame_lock.as_ref() {
            let mut img = frame.clone();
            if self.config.privacy_mode == PrivacyMode::Blur {
                image::imageops::blur(&mut img, 10.0);
            }
            Ok(img)
        } else {
            Err(VisionError::NotCapturing)
        }
    }

    pub fn analyze_frame_mock(&self, _frame: &RgbaImage) -> FrameAnalysisResult {
        FrameAnalysisResult {
            objects_detected: vec!["cup".to_string(), "keyboard".to_string()],
            ocr_text: "HAI-Net".to_string(),
            gesture: "thumbs_up".to_string(),
            emotional_valence: 0.8,
        }
    }
}

impl Default for VisionConfig {
    fn default() -> Self {
        Self {
            device_index: 0,
            resolution_width: 1280,
            resolution_height: 720,
            frame_rate: 30,
            privacy_mode: PrivacyMode::Off,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vision_system_new() {
        let config = VisionConfig::default();
        let vs = VisionSystem::new(config);
        assert_eq!(*vs.status.lock().unwrap(), WebcamStatus::Idle);
    }

    #[test]
    fn test_default_vision_config() {
        let config = VisionConfig::default();
        assert_eq!(config.device_index, 0);
        assert_eq!(config.resolution_width, 1280);
        assert_eq!(config.resolution_height, 720);
        assert_eq!(config.frame_rate, 30);
        assert_eq!(config.privacy_mode, PrivacyMode::Off);
    }

    #[test]
    fn test_analyze_frame_mock() {
        let config = VisionConfig::default();
        let vs = VisionSystem::new(config);
        let dummy_frame = RgbaImage::new(10, 10);
        let analysis = vs.analyze_frame_mock(&dummy_frame);

        assert_eq!(analysis.objects_detected, vec!["cup".to_string(), "keyboard".to_string()]);
        assert_eq!(analysis.ocr_text, "HAI-Net".to_string());
        assert_eq!(analysis.gesture, "thumbs_up".to_string());
        assert_eq!(analysis.emotional_valence, 0.8);
    }
}
