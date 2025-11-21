//! # START OF FILE hainet-portal/src-tauri/src/lib.rs

mod admin_bridge;
pub mod stt_handler;
pub mod tts_handler;
mod vision_handler;
mod video_handler;
mod settings_handler;
mod settings_storage;
mod metrics_handler;
mod metrics_storage;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{Manager, State};
use tiny_http::Server;
use tokio::sync::RwLock;

use admin_bridge::{AdminBridge, ChatMessage, ChatResponse, FileAttachment};
use stt_handler::{AudioData, TranscriptionResult};
use tts_handler::{TTSHandler, SynthesisRequest, SynthesisResponse};
use vision_handler::VisionState;
use settings_handler::SystemInfo;
use settings_storage::SettingsStorage;
use metrics_storage::MetricsStorage;
use sysinfo::System;
use hainet_persona::agents::metrics::MetricsCollector;

/// Global Admin AI Bridge state
struct AppState {
    admin_bridge: Arc<RwLock<AdminBridge>>,
    tts_handler: Arc<RwLock<TTSHandler>>,
    /// Keep the log guard alive for the lifetime of the application
    _log_guard: tracing_appender::non_blocking::WorkerGuard,
}

/// Metrics collector state
type MetricsState = Arc<RwLock<MetricsCollector>>;

/// Metrics storage state
type MetricsStorageState = Arc<RwLock<MetricsStorage>>;

/// Settings storage state
type SettingsState = Arc<RwLock<SettingsStorage>>;

/// State for managing video streaming servers
struct VideoStreamingState(pub Arc<Mutex<HashMap<u16, Arc<Server>>>>);


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

/// Get list of active agents
#[tauri::command]
async fn get_active_agents(state: State<'_, AppState>) -> Result<Vec<hainet_persona::messaging::AgentInfo>, String> {
    let bridge = state.admin_bridge.read().await;
    bridge.get_active_agents()
        .await
        .map_err(|e| e.to_string())
}

/// Get list of active projects
#[tauri::command]
async fn get_active_projects(state: State<'_, AppState>) -> Result<Vec<hainet_persona::projects::ProjectInfo>, String> {
    let bridge = state.admin_bridge.read().await;
    bridge.get_active_projects()
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

// ========== Project Management Commands ==========

/// Pause a project
#[tauri::command]
async fn pause_project(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let bridge = state.admin_bridge.read().await;
    bridge.pause_project(project_id)
        .await
        .map_err(|e| e.to_string())
}

/// Resume a paused project
#[tauri::command]
async fn resume_project(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let bridge = state.admin_bridge.read().await;
    bridge.resume_project(project_id)
        .await
        .map_err(|e| e.to_string())
}

/// Stop/cancel a project
#[tauri::command]
async fn stop_project(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let bridge = state.admin_bridge.read().await;
    bridge.stop_project(project_id)
        .await
        .map_err(|e| e.to_string())
}

/// Rename a project
#[tauri::command]
async fn rename_project(
    project_id: String,
    new_title: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let bridge = state.admin_bridge.read().await;
    bridge.rename_project(project_id, new_title)
        .await
        .map_err(|e| e.to_string())
}

/// Delete a project
#[tauri::command]
async fn delete_project(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let bridge = state.admin_bridge.read().await;
    bridge.delete_project(project_id)
        .await
        .map_err(|e| e.to_string())
}

/// Export a project to a tar.gz file
#[tauri::command]
async fn export_project(
    project_id: String,
    export_path: String,
    state: State<'_, AppState>,
) -> Result<hainet_persona::projects::ExportMetadata, String> {
    let bridge = state.admin_bridge.read().await;
    bridge.export_project(project_id, export_path)
        .await
        .map_err(|e| e.to_string())
}

/// Import a project from a tar.gz file
#[tauri::command]
async fn import_project(
    import_path: String,
    state: State<'_, AppState>,
) -> Result<hainet_persona::projects::ImportResult, String> {
    let bridge = state.admin_bridge.read().await;
    bridge.import_project(import_path)
        .await
        .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logging - MUST keep guard alive for file logging to work!
    let log_guard = hainet_core::logging::initialize_logging("hainet-portal", "debug")
        .expect("Failed to initialize logging");

  // Initialize Admin AI Bridge before building Tauri app
  let runtime = tokio::runtime::Runtime::new()
      .expect("Failed to create Tokio runtime");
  
  let (admin_bridge, metrics_collector, metrics_storage, settings_storage) = runtime.block_on(async {
      let admin_bridge = AdminBridge::new().await
          .expect("Failed to initialize Admin AI Bridge");
      
      // Initialize database directory
      let data_dir = dirs::data_dir()
          .expect("Failed to get data directory")
          .join("hainet-portal");
      std::fs::create_dir_all(&data_dir)
          .expect("Failed to create data directory");
      
      // Initialize MetricsCollector with database path
      let metrics_db_path = data_dir.join("metrics.db");
      let metrics_collector = MetricsCollector::new(
          &format!("sqlite://{}?mode=rwc", metrics_db_path.display())
      )
      .await
      .expect("Failed to initialize MetricsCollector");
      
      // Initialize MetricsStorage with database path
      let metrics_storage_path = data_dir.join("metrics_history.db");
      let metrics_storage = MetricsStorage::new(
          &format!("sqlite://{}?mode=rwc", metrics_storage_path.display())
      )
          .await
          .expect("Failed to initialize MetricsStorage");
      
      // Initialize SettingsStorage with database path
      let settings_db_path = data_dir.join("settings.db");
      let settings_storage = SettingsStorage::new(
          &format!("sqlite://{}?mode=rwc", settings_db_path.display())
      )
      .await
      .expect("Failed to initialize SettingsStorage");
      
      (admin_bridge, metrics_collector, metrics_storage, settings_storage)
  });
  
  // Wrap MetricsCollector in Arc<RwLock<>> for shared state
  let metrics_state: MetricsState = Arc::new(RwLock::new(metrics_collector));
  
  // Wrap MetricsStorage in Arc<RwLock<>> for shared state
  let metrics_storage_state: MetricsStorageState = Arc::new(RwLock::new(metrics_storage));
  
  // Wrap SettingsStorage in Arc<RwLock<>> for shared state
  let settings_state: SettingsState = Arc::new(RwLock::new(settings_storage));
  
  // Initialize TTS handler
  let tts_handler = TTSHandler::new();

  // Initialize Video Streaming state
  let video_streaming_state = VideoStreamingState(Arc::new(Mutex::new(HashMap::new())));

  // Initialize SystemInfo state
  let system_info_state = SystemInfo {
      sys: Mutex::new(System::new_all()),
  };
  
  tauri::Builder::default()
    .plugin(tauri_plugin_dialog::init())
    .plugin(tauri_plugin_fs::init())
    .setup(|app| {
        tracing::info!("HAI-Net Portal initialized successfully");

        // Get metrics collector and storage from managed state
        let metrics_for_broadcast = app.state::<MetricsState>().inner().clone();
        let metrics_for_snapshot = app.state::<MetricsState>().inner().clone();
        let storage_for_snapshot = app.state::<MetricsStorageState>().inner().clone();

        // Start metrics broadcast service for real-time updates
        metrics_handler::start_metrics_broadcast(
            app.handle().clone(),
            metrics_for_broadcast
        );
        tracing::info!("Metrics broadcast service started");

        // Start metrics snapshot recording task for historical analytics
        metrics_handler::start_metrics_snapshot_task(
            metrics_for_snapshot,
            storage_for_snapshot
        );
        tracing::info!("Metrics snapshot recording service started");

        Ok(())
    })
    .manage(AppState {
        admin_bridge: Arc::new(RwLock::new(admin_bridge)),
        tts_handler: Arc::new(RwLock::new(tts_handler)),
        _log_guard: log_guard, // Keep logger alive for app lifetime
    })
    .manage(metrics_state)
    .manage(metrics_storage_state)
    .manage(settings_state)
    .manage(VisionState(Mutex::new(None)))
    .manage(video_streaming_state)
    .manage(system_info_state)
    .invoke_handler(tauri::generate_handler![
        send_message,
        get_history,
        clear_history,
        get_agent_state,
        get_active_agents,
        get_active_projects,
        pause_project,
        resume_project,
        stop_project,
        rename_project,
        delete_project,
        export_project,
        import_project,
        transcribe_audio,
        synthesize_speech,
        tts_is_ready,
        list_tts_voices,
        vision_handler::list_webcam_devices,
        vision_handler::start_webcam,
        vision_handler::stop_webcam,
        vision_handler::capture_frame,
        vision_handler::set_privacy_mode,
        video_handler::stream_video,
        video_handler::stop_video_stream,
        settings_handler::get_settings,
        settings_handler::update_settings,
        settings_handler::save_device_preference,
        settings_handler::get_device_preferences,
        settings_handler::get_default_device,
        settings_handler::get_model_preferences,
        settings_handler::save_model_preference,
        settings_handler::get_model_preference,
        settings_handler::get_system_status,
        metrics_handler::get_agent_metrics,
        metrics_handler::get_agent_metrics_by_type,
        metrics_handler::get_metrics_summary,
        metrics_handler::export_metrics_json,
        metrics_handler::export_metrics_csv,
        metrics_handler::get_historical_metrics,
        metrics_handler::get_metrics_trend,
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
