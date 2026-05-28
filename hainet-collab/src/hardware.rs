//! # START OF FILE hainet-collab/src/hardware.rs
//! Hardware Profiler — Ported from PPLPWR's HardwareProfiler.ts
//! Uses `sysinfo` crate for CPU/RAM and nvidia-smi for GPU detection.

use serde::{Deserialize, Serialize};
use sysinfo::System;
use tracing::{info, debug, warn};

/// Hardware profile of the local system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareProfile {
    pub cpu_cores: u32,
    pub cpu_model: String,
    pub ram_total_gb: f64,
    pub ram_available_gb: f64,
    pub gpu: Option<GpuInfo>,
    pub disk_total_gb: f64,
    pub disk_available_gb: f64,
    pub os: String,
    pub arch: String,
    /// Weighted capability score (RAM 40%, GPU 30%, CPU 20%, Disk 10%)
    pub capability_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    pub name: String,
    pub vram_mb: u64,
    pub cuda_version: Option<String>,
    pub driver_version: Option<String>,
    pub temperature_c: Option<f32>,
    pub utilization_pct: Option<f32>,
}

impl HardwareProfile {
    /// Detect local hardware capabilities
    pub fn detect() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();

        let cpu_cores = sys.cpus().len() as u32;
        let cpu_model = sys.cpus().first()
            .map(|c| c.brand().to_string())
            .unwrap_or_else(|| "Unknown".to_string());
        let ram_total_gb = sys.total_memory() as f64 / 1_073_741_824.0;
        let ram_available_gb = sys.available_memory() as f64 / 1_073_741_824.0;

        let disk_total_gb: f64 = sysinfo::Disks::new_with_refreshed_list()
            .iter()
            .map(|d| d.total_space() as f64 / 1_073_741_824.0)
            .sum();
        let disk_available_gb: f64 = sysinfo::Disks::new_with_refreshed_list()
            .iter()
            .map(|d| d.available_space() as f64 / 1_073_741_824.0)
            .sum();

        let os = System::name().unwrap_or_else(|| "Unknown".to_string());
        let arch = std::env::consts::ARCH.to_string();

        let gpu = detect_nvidia_gpu();

        let capability_score = calculate_score(ram_total_gb, &gpu, cpu_cores, disk_total_gb);

        let profile = HardwareProfile {
            cpu_cores,
            cpu_model,
            ram_total_gb,
            ram_available_gb,
            gpu,
            disk_total_gb,
            disk_available_gb,
            os,
            arch,
            capability_score,
        };

        info!(
            cpu_cores,
            ram_gb = format!("{:.1}", ram_total_gb),
            has_gpu = profile.gpu.is_some(),
            score = format!("{:.1}", capability_score),
            "Hardware profile detected"
        );

        profile
    }

    /// Check if this hardware can run GPU compute tasks
    pub fn has_gpu(&self) -> bool {
        self.gpu.is_some()
    }

    /// Check if GPU VRAM meets minimum requirement
    pub fn meets_vram_requirement(&self, min_vram_gb: f32) -> bool {
        self.gpu.as_ref()
            .map(|g| (g.vram_mb as f32 / 1024.0) >= min_vram_gb)
            .unwrap_or(false)
    }

    /// Calculate the maximum safe context window length based on physical memory
    pub fn max_safe_context_length(&self) -> usize {
        let vram = self.gpu.as_ref().map(|g| g.vram_mb).unwrap_or(0);
        let ram = self.ram_total_gb;
        
        if vram >= 16384 || ram >= 32.0 {
            32768 // 32k for high-end (16GB+ VRAM or 32GB+ RAM)
        } else if vram >= 8192 || ram >= 16.0 {
            16384 // 16k for mid-range (8GB+ VRAM or 16GB+ RAM)
        } else if vram >= 4096 || ram >= 8.0 {
            8192 // 8k for low-end (4GB+ VRAM or 8GB+ RAM)
        } else {
            4096 // 4k absolute fallback
        }
    }
}

/// Calculate capability score (same formula as HAI-Net + NoSlop)
/// RAM 40%, GPU 30%, CPU 20%, Disk 10%
fn calculate_score(ram_gb: f64, gpu: &Option<GpuInfo>, cpu_cores: u32, disk_gb: f64) -> f64 {
    let ram_score = ram_gb * 10.0 * 0.4;
    let gpu_score = if gpu.is_some() { 100.0 } else { 0.0 } * 0.3;
    let cpu_score = cpu_cores as f64 * 5.0 * 0.2;
    let disk_score = disk_gb * 0.1;
    ram_score + gpu_score + cpu_score + disk_score
}

/// Detect NVIDIA GPU using nvidia-smi CLI
fn detect_nvidia_gpu() -> Option<GpuInfo> {
    let output = std::process::Command::new("nvidia-smi")
        .args(["--query-gpu=name,memory.total,driver_version,temperature.gpu,utilization.gpu", "--format=csv,noheader,nounits"])
        .output()
        .ok()?;

    if !output.status.success() {
        debug!("nvidia-smi not available or failed");
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.trim().lines().next()?;
    let parts: Vec<&str> = line.split(", ").collect();

    if parts.len() < 5 {
        warn!("Unexpected nvidia-smi output format");
        return None;
    }

    Some(GpuInfo {
        name: parts[0].to_string(),
        vram_mb: parts[1].trim().parse().unwrap_or(0),
        cuda_version: detect_cuda_version(),
        driver_version: Some(parts[2].trim().to_string()),
        temperature_c: parts[3].trim().parse().ok(),
        utilization_pct: parts[4].trim().parse().ok(),
    })
}

fn detect_cuda_version() -> Option<String> {
    let output = std::process::Command::new("nvidia-smi")
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Parse "CUDA Version: X.Y" from nvidia-smi header
    for line in stdout.lines() {
        if let Some(pos) = line.find("CUDA Version:") {
            let version = line[pos + 14..].trim().split_whitespace().next()?;
            return Some(version.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capability_score() {
        // High-spec machine
        let score = calculate_score(64.0, &Some(GpuInfo {
            name: "RTX 3060".to_string(),
            vram_mb: 12288,
            cuda_version: None,
            driver_version: None,
            temperature_c: None,
            utilization_pct: None,
        }), 16, 2048.0);
        assert!(score > 280.0);

        // Low-spec machine (no GPU)
        let low_score = calculate_score(4.0, &None, 2, 128.0);
        assert!(low_score < 50.0);
        assert!(score > low_score);
    }

    #[test]
    fn test_vram_requirement() {
        let mut profile = HardwareProfile::detect();
        // Override GPU for testing
        profile.gpu = Some(GpuInfo {
            name: "Test GPU".to_string(),
            vram_mb: 8192, // 8 GB
            cuda_version: None,
            driver_version: None,
            temperature_c: None,
            utilization_pct: None,
        });

        assert!(profile.meets_vram_requirement(4.0));
        assert!(profile.meets_vram_requirement(8.0));
        assert!(!profile.meets_vram_requirement(16.0));
    }
}
