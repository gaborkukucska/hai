//! # START OF FILE hainet-portal/src-tauri/src/lib.rs

mod admin_bridge;

use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

use admin_bridge::{AdminBridge, ChatMessage, ChatResponse, FileAttachment};

/// Global Admin AI Bridge state
struct AppState {
    admin_bridge: Arc<RwLock<AdminBridge>>,
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .setup(|app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }
      
      // Initialize Admin AI Bridge
      let runtime = tokio::runtime::Runtime::new()
          .expect("Failed to create Tokio runtime");
      
      let admin_bridge = runtime.block_on(async {
          AdminBridge::new().await
              .expect("Failed to initialize Admin AI Bridge")
      });
      
      // Store in app state
      app.manage(AppState {
          admin_bridge: Arc::new(RwLock::new(admin_bridge)),
      });
      
      log::info!("HAI-Net Portal initialized successfully");
      
      Ok(())
    })
    .invoke_handler(tauri::generate_handler![
        send_message,
        get_history,
        clear_history,
        get_agent_state,
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
