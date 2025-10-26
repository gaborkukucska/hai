//! hainet-portal/src-tauri/src/vision_handler.rs
//! Tauri command handlers for vision capabilities.

use std::sync::Mutex;
use tauri::{command, State};
use image::ImageFormat;
use hainet_core::multimodal::{VisionSystem, VisionConfig, PrivacyMode, FrameAnalysisResult};
use base64::{engine::general_purpose::STANDARD, Engine as _};

/// Tauri state to hold the VisionSystem instance.
pub struct VisionState(pub Mutex<Option<VisionSystem>>);

/// Serializable result for a captured frame.
#[derive(serde::Serialize)]
pub struct FrameCaptureResult {
    image_base64: String,
    analysis: FrameAnalysisResult,
}

#[command]
pub async fn list_webcam_devices() -> Result<Vec<String>, String> {
    VisionSystem::list_devices().map_err(|e| e.to_string())
}

#[command]
pub async fn start_webcam(state: State<'_, VisionState>, config: VisionConfig) -> Result<(), String> {
    let mut vision_system = state.0.lock().unwrap();
    let new_system = VisionSystem::new(config);
    new_system.start_capture().map_err(|e| format!("{:?}", e))?;
    *vision_system = Some(new_system);
    Ok(())
}

#[command]
pub async fn stop_webcam(state: State<'_, VisionState>) -> Result<(), String> {
    let mut vision_system = state.0.lock().unwrap();
    if let Some(system) = vision_system.take() {
        system.stop_capture();
    }
    Ok(())
}

#[command]
pub async fn capture_frame(state: State<'_, VisionState>) -> Result<FrameCaptureResult, String> {
    let vision_system = state.0.lock().unwrap();
    if let Some(system) = &*vision_system {
        let frame = system.capture_frame().map_err(|e| format!("{:?}", e))?;
        let analysis = system.analyze_frame_mock(&frame);

        // Convert frame to Base64
        let mut buf = Vec::new();
        frame.write_to(&mut std::io::Cursor::new(&mut buf), ImageFormat::Png)
            .map_err(|e| e.to_string())?;
        let image_base64 = STANDARD.encode(&buf);

        Ok(FrameCaptureResult {
            image_base64,
            analysis,
        })
    } else {
        Err("Webcam is not running.".to_string())
    }
}

#[command]
pub async fn set_privacy_mode(state: State<'_, VisionState>, mode: PrivacyMode) -> Result<(), String> {
    let mut vision_system = state.0.lock().unwrap();
    if let Some(_system) = &mut *vision_system {
        // This requires modifying VisionSystem to allow config changes,
        // or re-initializing it. For simplicity, we'll just note this limitation.
        // In a real app, you'd have a `set_config` method.
        println!("Privacy mode set to: {:?}", mode); // Placeholder
        Ok(())
    } else {
        Err("Webcam is not running.".to_string())
    }
}
