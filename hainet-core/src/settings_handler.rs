// <!-- # START OF FILE hainet-portal/src-tauri/src/settings_handler.rs -->
//! Crate for handling settings and system status.

use serde::{Serialize, Deserialize};
use sysinfo::{System, Disks};

use tracing::{info, error, warn, debug};
use tokio::sync::RwLock;
use std::sync::{Arc, Mutex};
use crate::settings_storage::{SettingsStorage, DevicePreference, ModelPreference};

pub struct SystemInfo {
    pub sys: Mutex<System>,
}

/// Settings storage state type
pub type SettingsState = Arc<RwLock<SettingsStorage>>;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Settings {
    pub theme: String,
    pub audio_input_device: Option<String>,
    pub video_input_device: Option<String>,
    pub stt_model: Option<String>,
    pub tts_model: Option<String>,
    pub vision_model: Option<String>,
    // Privacy settings
    pub pii_detection: bool,
    pub bias_detection: bool,
    pub harm_detection: bool,
    // Notification settings
    pub enable_notifications: bool,
    pub enable_sound: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            audio_input_device: None,
            video_input_device: None,
            stt_model: None,
            tts_model: None,
            vision_model: None,
            pii_detection: true,
            bias_detection: true,
            harm_detection: true,
            enable_notifications: true,
            enable_sound: true,
        }
    }
}

impl Settings {
    /// Convert to key-value pairs for storage
    fn to_pairs(&self) -> Vec<(&str, String)> {
        vec![
            ("theme", self.theme.clone()),
            ("audio_input_device", self.audio_input_device.clone().unwrap_or_default()),
            ("video_input_device", self.video_input_device.clone().unwrap_or_default()),
            ("stt_model", self.stt_model.clone().unwrap_or_default()),
            ("tts_model", self.tts_model.clone().unwrap_or_default()),
            ("vision_model", self.vision_model.clone().unwrap_or_default()),
            ("pii_detection", self.pii_detection.to_string()),
            ("bias_detection", self.bias_detection.to_string()),
            ("harm_detection", self.harm_detection.to_string()),
            ("enable_notifications", self.enable_notifications.to_string()),
            ("enable_sound", self.enable_sound.to_string()),
        ]
    }
    
    /// Load from key-value pairs
    async fn from_storage(storage: &SettingsStorage) -> Result<Self, String> {
        let mut settings = Settings::default();
        
        if let Ok(Some(theme)) = storage.get_setting("theme").await {
            settings.theme = theme;
        }
        if let Ok(Some(device)) = storage.get_setting("audio_input_device").await {
            if !device.is_empty() {
                settings.audio_input_device = Some(device);
            }
        }
        if let Ok(Some(device)) = storage.get_setting("video_input_device").await {
            if !device.is_empty() {
                settings.video_input_device = Some(device);
            }
        }
        if let Ok(Some(model)) = storage.get_setting("stt_model").await {
            if !model.is_empty() {
                settings.stt_model = Some(model);
            }
        }
        if let Ok(Some(model)) = storage.get_setting("tts_model").await {
            if !model.is_empty() {
                settings.tts_model = Some(model);
            }
        }
        if let Ok(Some(model)) = storage.get_setting("vision_model").await {
            if !model.is_empty() {
                settings.vision_model = Some(model);
            }
        }
        if let Ok(Some(val)) = storage.get_setting("pii_detection").await {
            settings.pii_detection = val.parse().unwrap_or(true);
        }
        if let Ok(Some(val)) = storage.get_setting("bias_detection").await {
            settings.bias_detection = val.parse().unwrap_or(true);
        }
        if let Ok(Some(val)) = storage.get_setting("harm_detection").await {
            settings.harm_detection = val.parse().unwrap_or(true);
        }
        if let Ok(Some(val)) = storage.get_setting("enable_notifications").await {
            settings.enable_notifications = val.parse().unwrap_or(true);
        }
        if let Ok(Some(val)) = storage.get_setting("enable_sound").await {
            settings.enable_sound = val.parse().unwrap_or(true);
        }
        
        Ok(settings)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SystemStatus {
    pub cpu_usage: f32,
    pub memory_usage: u64,
    pub total_memory: u64,
    pub disk_usage: u64,
    pub total_disk: u64,
}


pub async fn get_settings(storage: &crate::SettingsState) -> Result<Settings, String> {
    let storage = storage.read().await;
    Settings::from_storage(&*storage).await
}

pub async fn update_settings(
    settings: Settings,
    storage: &crate::SettingsState
) -> Result<(), String> {
    let storage = storage.read().await;
    
    // Convert to owned pairs to avoid lifetime issues
    let pairs_owned = settings.to_pairs();
    let pairs: Vec<(&str, &str)> = pairs_owned
        .iter()
        .map(|(k, v)| (*k, v.as_str()))
        .collect();
    
    storage.save_settings_batch(pairs)
        .await
        .map_err(|e| format!("Failed to save settings: {}", e))?;
    
    info!("Settings saved successfully");
    Ok(())
}

pub async fn save_device_preference(
    device: DevicePreference,
    storage: &crate::SettingsState
) -> Result<(), String> {
    let storage = storage.read().await;
    
    storage.save_device_preference(&device)
        .await
        .map_err(|e| format!("Failed to save device preference: {}", e))?;
    
    info!("Device preference saved: {} - {}", device.device_type, device.device_name);
    Ok(())
}

pub async fn get_device_preferences(
    device_type: String,
    storage: &crate::SettingsState
) -> Result<Vec<DevicePreference>, String> {
    let storage = storage.read().await;
    
    storage.get_device_preferences(&device_type)
        .await
        .map_err(|e| format!("Failed to get device preferences: {}", e))
}

pub async fn get_default_device(
    device_type: String,
    storage: &crate::SettingsState
) -> Result<Option<DevicePreference>, String> {
    let storage = storage.read().await;
    
    storage.get_default_device(&device_type)
        .await
        .map_err(|e| format!("Failed to get default device: {}", e))
}

pub async fn get_model_preferences(
    storage: &crate::SettingsState
) -> Result<Vec<ModelPreference>, String> {
    info!("[Backend] get_model_preferences called");
    let storage = storage.read().await;
    
    let prefs = storage.get_all_model_preferences()
        .await
        .map_err(|e| {
            error!("[Backend] Failed to get model preferences: {}", e);
            format!("Failed to get model preferences: {}", e)
        })?;
    
    info!("[Backend] Returning {} model preferences", prefs.len());
    for pref in &prefs {
        debug!("[Backend]   - {}: {} (fallback: {})", pref.agent_type, pref.preferred_family, pref.allow_fallback);
    }
    
    Ok(prefs)
}

pub async fn save_model_preference(
    agent_type: String,
    family: String,
    allow_fallback: bool,
    storage: &crate::SettingsState
) -> Result<(), String> {
    info!("[Backend] save_model_preference called: {} -> {} (fallback: {})", agent_type, family, allow_fallback);
    let storage = storage.read().await;
    
    storage.save_model_preference(&agent_type, &family, allow_fallback)
        .await
        .map_err(|e| {
            error!("[Backend] Failed to save model preference: {}", e);
            format!("Failed to save model preference: {}", e)
        })?;
    
    info!("[Backend] Model preference saved successfully: {} -> {}", agent_type, family);
    
    // Sync preference to hainet-persona database
    if let Err(e) = sync_preference_to_persona(&agent_type, &family).await {
        warn!("Failed to sync preference to hainet-persona: {}", e);
        // Don't fail the request - Portal settings were saved successfully
    }
    
    Ok(())
}

/// Sync a model preference to hainet-persona's database
async fn sync_preference_to_persona(agent_type: &str, model_family: &str) -> Result<(), String> {
    // Determine database path (same as in admin_bridge.rs)
    let home_dir = dirs::home_dir()
        .ok_or_else(|| "Cannot determine home directory".to_string())?;
    let hainet_dir = home_dir.join(".hainet");
    let data_dir = hainet_dir.join("data");
    
    // Create directories if they don't exist
    std::fs::create_dir_all(&data_dir)
        .map_err(|e| format!("Failed to create data directory: {}", e))?;
    
    let settings_db_path = data_dir.join("user_settings.db");
    let db_connection_string = format!("sqlite://{}?mode=rwc", settings_db_path.display());
    
    // Create UserSettingsManager
    let user_settings = hainet_persona::UserSettingsManager::new(&db_connection_string).await
        .map_err(|e| format!("Failed to create UserSettingsManager: {}", e))?;
    
    // Set the preference
    user_settings.set_model_preference(agent_type, model_family).await
        .map_err(|e| format!("Failed to set preference in hainet-persona: {}", e))?;
    
    info!("Synced preference for {} to {} in hainet-persona database", agent_type, model_family);
    
    Ok(())
}

pub async fn get_model_preference(
    agent_type: String,
    storage: &crate::SettingsState
) -> Result<Option<ModelPreference>, String> {
    let storage = storage.read().await;
    
    storage.get_model_preference(&agent_type)
        .await
        .map_err(|e| format!("Failed to get model preference: {}", e))
}

pub fn get_system_status(system_info: &crate::settings_handler::SystemInfo) -> SystemStatus {
    let mut sys = system_info.sys.lock().unwrap();
    sys.refresh_cpu();
    sys.refresh_memory();

    // Use new API for CPU usage (sysinfo 0.30+)
    let cpu_usage = sys.global_cpu_info().cpu_usage();
    let total_memory = sys.total_memory();
    let memory_usage = sys.used_memory();

    // Use new Disks API (sysinfo 0.30+)
    let disks = Disks::new_with_refreshed_list();
    let (disk_usage, total_disk) = disks.iter().fold((0, 0), |(used, total), disk| {
        (used + (disk.total_space() - disk.available_space()), total + disk.total_space())
    });

    SystemStatus {
        cpu_usage,
        memory_usage,
        total_memory,
        disk_usage,
        total_disk,
    }
}
// <!-- # END OF FILE hainet-portal/src-tauri/src/settings_handler.rs -->
