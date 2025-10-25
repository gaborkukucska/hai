//! # START OF FILE hainet-portal/src-tauri/src/lib.rs

mod admin_bridge;
pub mod stt_handler;
pub mod tts_handler;

use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

use admin_bridge::{AdminBridge, ChatMessage, ChatResponse, FileAttachment};
use stt_handler::{AudioData, TranscriptionResult};
use tts_handler::{TTSHandler, SynthesisRequest, SynthesisResponse};

/// Global Admin AI Bridge state
struct AppState {
    admin_bridge: Arc<RwLock<AdminBridge>>,
    tts_handler: Arc<RwLock<TTSHandler>>,
}

/// Send message to Admin AI
#[tauri::command]
async fn send_message(
    content: String,
    attachments: Vec<FileAttachment>,
    state: State<'_, AppState>,
) -> Result<ChatResponse, String> {
    let bridge = state.admin_bridge.read().await;
    bridge.send_message(content, attachments)
        .await
        .map_err(|e| e.to_string())
}

/// Get message history
#[tauri::command]
async fn get_history(state: State<'_, AppState>) -> Result<Vec<ChatMessage>, String> {
    let bridge = state.admin_bridge.read().await;
    bridge.get_history()
        .await
        .map_err(|e| e.to_string())
}

/// Clear message history
#[tauri::command]
async fn clear_history(state: State<'_, AppState>) -> Result<(), String> {
    let bridge = state.admin_bridge.read().await;
    bridge.clear_history()
        .await
        .map_err(|e| e.to_string())
}

/// Get current agent state
#[tauri::command]
async fn get_agent_state(state: State<'_, AppState>) -> Result<String, String> {
    let bridge = state.admin_bridge.read().await;
    bridge.get_state()
        .await
        .map_err(|e| e.to_string())
}

/// Transcribe audio to text via Admin AI
#[tauri::command]
async fn transcribe_audio(
    audio: AudioData,
    state: State<'_, AppState>,
) -> Result<TranscriptionResult, String> {
    let bridge = state.admin_bridge.read().await;
    bridge.transcribe_audio(audio)
        .await
        .map_err(|e| e.to_string())
}

/// Synthesize speech from text
#[tauri::command]
async fn synthesize_speech(
    request: SynthesisRequest,
    state: State<'_, AppState>,
) -> Result<SynthesisResponse, String> {
    let tts = state.tts_handler.read().await;
    tts.synthesize(request).await
}

/// Check if TTS is ready
#[tauri::command]
async fn tts_is_ready(state: State<'_, AppState>) -> Result<bool, String> {
    let tts = state.tts_handler.read().await;
    Ok(tts.is_ready())
}

/// List available TTS voices
#[tauri::command]
async fn list_tts_voices(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let tts = state.tts_handler.read().await;
    tts.list_voices()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  // Initialize Admin AI Bridge before building Tauri app
  let runtime = tokio::runtime::Runtime::new()
      .expect("Failed to create Tokio runtime");
  
  let admin_bridge = runtime.block_on(async {
      AdminBridge::new().await
          .expect("Failed to initialize Admin AI Bridge")
  });
  
  // Initialize TTS handler
  let tts_handler = TTSHandler::new();
  
  tauri::Builder::default()
    .setup(|app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }
      
      log::info!("HAI-Net Portal initialized successfully");
      
      Ok(())
    })
    .manage(AppState {
        admin_bridge: Arc::new(RwLock::new(admin_bridge)),
        tts_handler: Arc::new(RwLock::new(tts_handler)),
    })
    .invoke_handler(tauri::generate_handler![
        send_message,
        get_history,
        clear_history,
        get_agent_state,
        transcribe_audio,
        synthesize_speech,
        tts_is_ready,
        list_tts_voices,
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
