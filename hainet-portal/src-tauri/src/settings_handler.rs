// <!-- # START OF FILE hainet-portal/src-tauri/src/settings_handler.rs -->
//! Crate for handling settings and system status.

use serde::{Serialize, Deserialize};
use sysinfo::{System, Disks};
use tauri::State;
use std::sync::Mutex;

pub struct SystemInfo {
    pub sys: Mutex<System>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Settings {
    pub theme: String,
    pub audio_input_device: Option<String>,
    pub video_input_device: Option<String>,
    pub stt_model: Option<String>,
    pub tts_model: Option<String>,
    pub vision_model: Option<String>,
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
        }
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


#[tauri::command]
pub fn get_settings() -> Settings {
    // In the future, this will load from a config file.
    Settings::default()
}

#[tauri::command]
pub fn update_settings(settings: Settings) -> Result<(), String> {
    // In the future, this will save to a config file.
    println!("Updating settings: {:?}", settings);
    Ok(())
}

#[tauri::command]
pub fn get_system_status(system_info: State<SystemInfo>) -> SystemStatus {
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
