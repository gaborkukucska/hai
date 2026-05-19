use serde_json::{Value, json};
use tracing::{error, debug};

use crate::{
    AppState, MetricsState, MetricsStorageState, SettingsState,
    admin_bridge, metrics_handler, settings_handler, tts_handler
};

pub async fn handle_invoke(
    cmd: &str,
    args: Value,
    app_state: &AppState,
    metrics_state: &MetricsState,
    metrics_storage: &MetricsStorageState,
    settings_state: &SettingsState,
) -> Result<Value, String> {
    debug!("Routing API invoke: {}", cmd);
    
    match cmd {
        // --- Admin Bridge ---
        "send_message" => {
            let content = args.get("content").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let attachments_val = args.get("attachments").cloned().unwrap_or(Value::Array(vec![]));
            let attachments: Vec<admin_bridge::FileAttachment> = serde_json::from_value(attachments_val).unwrap_or_default();
            
            let bridge = app_state.admin_bridge.read().await;
            let res = bridge.send_message(content, attachments).await.map_err(|e| e.to_string())?;
            Ok(serde_json::to_value(res).unwrap())
        },
        "get_history" => {
            let bridge = app_state.admin_bridge.read().await;
            let res = bridge.get_history().await.map_err(|e| e.to_string())?;
            Ok(serde_json::to_value(res).unwrap())
        },
        "clear_history" => {
            let bridge = app_state.admin_bridge.read().await;
            bridge.clear_history().await.map_err(|e| e.to_string())?;
            Ok(json!({}))
        },
        "get_agent_state" => {
            let bridge = app_state.admin_bridge.read().await;
            let res = bridge.get_state().await.map_err(|e| e.to_string())?;
            Ok(json!(res))
        },
        "get_active_agents" => {
            let bridge = app_state.admin_bridge.read().await;
            let res = bridge.get_active_agents().await.map_err(|e| e.to_string())?;
            Ok(serde_json::to_value(res).unwrap())
        },
        "get_active_projects" => {
            let bridge = app_state.admin_bridge.read().await;
            let res = bridge.get_active_projects().await.map_err(|e| e.to_string())?;
            Ok(serde_json::to_value(res).unwrap())
        },
        "pause_project" => {
            let id = args.get("project_id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let bridge = app_state.admin_bridge.read().await;
            bridge.pause_project(id).await.map_err(|e| e.to_string())?;
            Ok(json!({}))
        },
        "resume_project" => {
            let id = args.get("project_id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let bridge = app_state.admin_bridge.read().await;
            bridge.resume_project(id).await.map_err(|e| e.to_string())?;
            Ok(json!({}))
        },
        "stop_project" => {
            let id = args.get("project_id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let bridge = app_state.admin_bridge.read().await;
            bridge.stop_project(id).await.map_err(|e| e.to_string())?;
            Ok(json!({}))
        },
        "rename_project" => {
            let id = args.get("project_id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let name = args.get("new_title").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let bridge = app_state.admin_bridge.read().await;
            bridge.rename_project(id, name).await.map_err(|e| e.to_string())?;
            Ok(json!({}))
        },
        "delete_project" => {
            let id = args.get("project_id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let bridge = app_state.admin_bridge.read().await;
            bridge.delete_project(id).await.map_err(|e| e.to_string())?;
            Ok(json!({}))
        },
        
        // --- Settings ---
        "get_settings" => {
            let res = settings_handler::get_settings(settings_state).await?;
            Ok(serde_json::to_value(res).unwrap())
        },
        "update_settings" => {
            let settings: settings_handler::Settings = serde_json::from_value(args.get("settings").cloned().unwrap_or_default()).map_err(|e| e.to_string())?;
            settings_handler::update_settings(settings, settings_state).await?;
            Ok(json!({}))
        },
        "get_model_preferences" => {
            let res = settings_handler::get_model_preferences(settings_state).await?;
            Ok(serde_json::to_value(res).unwrap())
        },
        "save_model_preference" => {
            let agent = args.get("agent_type").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let family = args.get("family").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let fallback = args.get("allow_fallback").and_then(|v| v.as_bool()).unwrap_or(true);
            settings_handler::save_model_preference(agent, family, fallback, settings_state).await?;
            Ok(json!({}))
        },
        "get_system_status" => {
            // Because we removed the system_info state, we'll just implement it locally here to avoid passing a mutex
            use sysinfo::{System, Disks};
            let mut sys = System::new_all();
            sys.refresh_cpu();
            sys.refresh_memory();
            let cpu_usage = sys.global_cpu_info().cpu_usage();
            let total_memory = sys.total_memory();
            let memory_usage = sys.used_memory();
            let disks = Disks::new_with_refreshed_list();
            let (disk_usage, total_disk) = disks.iter().fold((0, 0), |(used, total), disk| {
                (used + (disk.total_space() - disk.available_space()), total + disk.total_space())
            });
            let status = settings_handler::SystemStatus {
                cpu_usage, memory_usage, total_memory, disk_usage, total_disk,
            };
            Ok(serde_json::to_value(status).unwrap())
        },

        // --- Metrics ---
        "get_metrics_summary" => {
            let res = metrics_handler::get_metrics_summary(metrics_state).await?;
            Ok(serde_json::to_value(res).unwrap())
        },
        "get_agent_metrics" => {
            let id = args.get("agent_id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let res = metrics_handler::get_agent_metrics_by_type(id, metrics_state).await?;
            Ok(serde_json::to_value(res).unwrap())
        },
        "get_metrics_trend" => {
            let agent = args.get("agent_type").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let interval = args.get("interval").and_then(|v| v.as_str()).unwrap_or("3600").to_string();
            let res = metrics_handler::get_metrics_trend(agent, interval, None, metrics_storage).await?;
            Ok(serde_json::to_value(res).unwrap())
        },
        
        // --- STT/TTS ---
        "tts_is_ready" => {
            let tts = app_state.tts_handler.read().await;
            Ok(json!(tts.is_ready()))
        },
        "list_tts_voices" => {
            let tts = app_state.tts_handler.read().await;
            let res = tts.list_voices()?;
            Ok(serde_json::to_value(res).unwrap())
        },
        "synthesize_speech" => {
            let req: tts_handler::SynthesisRequest = serde_json::from_value(args.get("request").cloned().unwrap_or_default()).map_err(|e| e.to_string())?;
            let tts = app_state.tts_handler.read().await;
            let res = tts.synthesize(req).await?;
            Ok(serde_json::to_value(res).unwrap())
        },
        
        // Fallback for unimplemented endpoints
        _ => {
            error!("Unimplemented API command: {}", cmd);
            Err(format!("Command '{}' not yet ported to HTTP API", cmd))
        }
    }
}
