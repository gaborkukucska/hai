// START OF FILE hainet-seed/src/installer/platform.rs
//! Platform Detection Module
//! 
//! Detects operating system, architecture, and hardware capabilities.

use anyhow::{Result, anyhow};
use std::fmt;
use serde::{Deserialize, Serialize};
use tracing::{info, debug};

/// Supported platforms
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Platform {
    /// Linux (x86_64, aarch64)
    Linux { arch: Architecture },
    /// macOS (Intel, Apple Silicon)
    MacOS { arch: Architecture },
    /// Android via Termux
    AndroidTermux { arch: Architecture },
    /// Other/unsupported
    Other(String),
}

impl Platform {
    /// Detect current platform
    pub fn detect() -> Result<Self> {
        let os = std::env::consts::OS;
        let arch = Architecture::detect()?;
        
        debug!("Detected OS: {}, Architecture: {:?}", os, arch);
        
        match os {
            "linux" => {
                // Check if running in Termux (Android)
                if Self::is_termux() {
                    Ok(Platform::AndroidTermux { arch })
                } else {
                    Ok(Platform::Linux { arch })
                }
            }
            "macos" => Ok(Platform::MacOS { arch }),
            other => Ok(Platform::Other(other.to_string())),
        }
    }
    
    /// Check if running in Termux environment
    fn is_termux() -> bool {
        std::env::var("PREFIX")
            .map(|prefix| prefix.contains("com.termux"))
            .unwrap_or(false)
    }
    
    /// Check if platform is supported
    pub fn is_supported(&self) -> bool {
        !matches!(self, Platform::Other(_))
    }
    
    /// Get platform-specific Ollama install script URL
    pub fn ollama_install_script(&self) -> Option<&'static str> {
        match self {
            Platform::Linux { .. } => Some("https://ollama.com/install.sh"),
            Platform::MacOS { .. } => Some("https://ollama.com/download/Ollama-darwin.zip"),
            Platform::AndroidTermux { .. } => None, // Termux needs special handling
            Platform::Other(_) => None,
        }
    }
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Platform::Linux { arch } => write!(f, "Linux ({})", arch),
            Platform::MacOS { arch } => write!(f, "macOS ({})", arch),
            Platform::AndroidTermux { arch } => write!(f, "Android/Termux ({})", arch),
            Platform::Other(name) => write!(f, "Other ({})", name),
        }
    }
}

/// CPU Architecture
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Architecture {
    X86_64,
    Aarch64,
    Other(String),
}

impl Architecture {
    /// Detect current architecture
    pub fn detect() -> Result<Self> {
        let arch = std::env::consts::ARCH;
        
        Ok(match arch {
            "x86_64" => Architecture::X86_64,
            "aarch64" => Architecture::Aarch64,
            other => Architecture::Other(other.to_string()),
        })
    }
}

impl fmt::Display for Architecture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Architecture::X86_64 => write!(f, "x86_64"),
            Architecture::Aarch64 => write!(f, "aarch64"),
            Architecture::Other(name) => write!(f, "{}", name),
        }
    }
}

/// System tier based on hardware capabilities
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SystemTier {
    /// Minimal (< 2GB RAM) - Tier 1
    Tier1,
    /// Low (2-4GB RAM) - Tier 2
    Tier2,
    /// Medium (4-16GB RAM) - Tier 3
    Tier3,
    /// High (16GB+ RAM) - Tier 4
    Tier4,
}

impl SystemTier {
    /// Detect system tier based on available RAM
    pub fn detect() -> Result<Self> {
        let total_ram_gb = Self::get_total_ram_gb()?;
        
        info!("Total RAM: {:.2} GB", total_ram_gb);
        
        let tier = if total_ram_gb < 2.0 {
            SystemTier::Tier1
        } else if total_ram_gb < 4.0 {
            SystemTier::Tier2
        } else if total_ram_gb < 16.0 {
            SystemTier::Tier3
        } else {
            SystemTier::Tier4
        };
        
        Ok(tier)
    }
    
    /// Get total system RAM in GB
    fn get_total_ram_gb() -> Result<f64> {
        #[cfg(target_os = "linux")]
        {
            Self::get_ram_linux()
        }
        
        #[cfg(target_os = "macos")]
        {
            Self::get_ram_macos()
        }
        
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            // Default to 4GB if we can't detect
            Ok(4.0)
        }
    }
    
    #[cfg(target_os = "linux")]
    fn get_ram_linux() -> Result<f64> {
        use std::fs;
        
        let meminfo = fs::read_to_string("/proc/meminfo")
            .map_err(|e| anyhow!("Failed to read /proc/meminfo: {}", e))?;
        
        for line in meminfo.lines() {
            if line.starts_with("MemTotal:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let kb: u64 = parts[1].parse()
                        .map_err(|e| anyhow!("Failed to parse RAM value: {}", e))?;
                    let gb = kb as f64 / 1024.0 / 1024.0;
                    return Ok(gb);
                }
            }
        }
        
        Err(anyhow!("MemTotal not found in /proc/meminfo"))
    }
    
    #[cfg(target_os = "macos")]
    fn get_ram_macos() -> Result<f64> {
        use std::process::Command;
        
        let output = Command::new("sysctl")
            .arg("-n")
            .arg("hw.memsize")
            .output()
            .map_err(|e| anyhow!("Failed to run sysctl: {}", e))?;
        
        let bytes_str = String::from_utf8_lossy(&output.stdout);
        let bytes: u64 = bytes_str.trim().parse()
            .map_err(|e| anyhow!("Failed to parse RAM value: {}", e))?;
        
        let gb = bytes as f64 / 1024.0 / 1024.0 / 1024.0;
        Ok(gb)
    }
    
    /// Get recommended model for this tier
    pub fn recommended_model(&self) -> &'static str {
        match self {
            SystemTier::Tier1 => "gemma2:2b",
            SystemTier::Tier2 => "gemma2:4b",
            SystemTier::Tier3 | SystemTier::Tier4 => "gemma3:12b-it",
        }
    }
}

impl fmt::Display for SystemTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SystemTier::Tier1 => write!(f, "Tier 1 (Minimal)"),
            SystemTier::Tier2 => write!(f, "Tier 2 (Low)"),
            SystemTier::Tier3 => write!(f, "Tier 3 (Medium)"),
            SystemTier::Tier4 => write!(f, "Tier 4 (High)"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_platform_detection() {
        let platform = Platform::detect();
        assert!(platform.is_ok());
        
        let platform = platform.unwrap();
        assert!(platform.is_supported() || matches!(platform, Platform::Other(_)));
    }
    
    #[test]
    fn test_architecture_detection() {
        let arch = Architecture::detect();
        assert!(arch.is_ok());
    }
    
    #[test]
    fn test_system_tier_detection() {
        let tier = SystemTier::detect();
        assert!(tier.is_ok());
    }
    
    #[test]
    fn test_tier_model_mapping() {
        assert_eq!(SystemTier::Tier1.recommended_model(), "gemma2:2b");
        assert_eq!(SystemTier::Tier2.recommended_model(), "gemma2:4b");
        assert_eq!(SystemTier::Tier3.recommended_model(), "gemma3:12b-it");
        assert_eq!(SystemTier::Tier4.recommended_model(), "gemma3:12b-it");
    }
}
