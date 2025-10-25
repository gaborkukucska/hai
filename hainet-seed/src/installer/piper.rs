//! # START OF FILE hainet-seed/src/installer/piper.rs
//! Piper TTS auto-installer
//!
//! Automatically installs Piper TTS and downloads voice models
//! based on system capabilities and user preferences.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::installer::platform::{Platform, Architecture, SystemTier};

/// Piper TTS installer
pub struct PiperInstaller {
    platform: Platform,
}

impl PiperInstaller {
    /// Create new Piper installer
    pub fn new(platform: Platform) -> Self {
        Self { platform }
    }
    
    /// Check if Piper is installed
    pub fn is_installed(&self) -> bool {
        self.get_piper_path().is_some()
    }
    
    /// Check if Piper service is running/accessible
    pub fn is_running(&self) -> bool {
        if let Some(piper_path) = self.get_piper_path() {
            // Test Piper by running with --help
            Command::new(piper_path)
                .arg("--help")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        } else {
            false
        }
    }
    
    /// Install Piper TTS
    pub fn install(&self) -> Result<()> {
        println!("📢 Installing Piper TTS...");
        
        match &self.platform {
            Platform::Linux { .. } | Platform::AndroidTermux { .. } => self.install_linux(),
            Platform::MacOS { .. } => self.install_macos(),
            Platform::Other(os) if os == "windows" => self.install_windows(),
            Platform::Other(os) => {
                anyhow::bail!("Piper installation not supported on platform: {}", os)
            }
        }
    }
    
    /// Install Piper on Linux
    fn install_linux(&self) -> Result<()> {
        let home = dirs::home_dir()
            .context("Failed to determine home directory")?;
        
        let install_dir = home.join(".local/share/piper");
        let bin_dir = home.join(".local/bin");
        
        // Create directories
        std::fs::create_dir_all(&install_dir)?;
        std::fs::create_dir_all(&bin_dir)?;
        
        println!("📥 Downloading Piper binary...");
        
        // Determine architecture
        let arch = match &self.platform {
            Platform::Linux { arch } | Platform::MacOS { arch } | Platform::AndroidTermux { arch } => arch,
            Platform::Other(_) => anyhow::bail!("Unsupported platform for Piper installation"),
        };
        
        let arch_suffix = match arch {
            Architecture::X86_64 => "amd64",
            Architecture::Aarch64 => "arm64",
            _ => anyhow::bail!("Unsupported architecture for Piper: {:?}", arch),
        };
        
        // Download URL (using latest release from GitHub)
        let download_url = format!(
            "https://github.com/rhasspy/piper/releases/latest/download/piper_linux_{}.tar.gz",
            arch_suffix
        );
        
        let tar_path = install_dir.join("piper.tar.gz");
        
        // Download using curl
        let status = Command::new("curl")
            .arg("-L")
            .arg("-o")
            .arg(&tar_path)
            .arg(&download_url)
            .status()
            .context("Failed to download Piper. Is curl installed?")?;
        
        if !status.success() {
            anyhow::bail!("Failed to download Piper binary");
        }
        
        println!("📦 Extracting Piper...");
        
        // Extract tar.gz
        let status = Command::new("tar")
            .arg("-xzf")
            .arg(&tar_path)
            .arg("-C")
            .arg(&install_dir)
            .status()
            .context("Failed to extract Piper archive")?;
        
        if !status.success() {
            anyhow::bail!("Failed to extract Piper archive");
        }
        
        // Find the piper binary in extracted files
        let piper_binary = install_dir.join("piper/piper");
        
        if !piper_binary.exists() {
            anyhow::bail!("Piper binary not found after extraction");
        }
        
        // Make executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&piper_binary)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&piper_binary, perms)?;
        }
        
        // Create symlink in ~/.local/bin
        let symlink_path = bin_dir.join("piper");
        
        if symlink_path.exists() {
            std::fs::remove_file(&symlink_path)?;
        }
        
        #[cfg(unix)]
        std::os::unix::fs::symlink(&piper_binary, &symlink_path)?;
        
        // Clean up tar file
        let _ = std::fs::remove_file(&tar_path);
        
        println!("✅ Piper installed to {}", symlink_path.display());
        println!("💡 Make sure ~/.local/bin is in your PATH");
        
        Ok(())
    }
    
    /// Install Piper on macOS
    fn install_macos(&self) -> Result<()> {
        println!("🍎 Installing Piper via Homebrew...");
        
        // Check if Homebrew is installed
        let has_brew = Command::new("which")
            .arg("brew")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        
        if !has_brew {
            println!("❌ Homebrew not found. Please install Homebrew first:");
            println!("   /bin/bash -c \"$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\"");
            anyhow::bail!("Homebrew required for Piper installation on macOS");
        }
        
        // Try to install via Homebrew (if available in homebrew-core or tap)
        // Note: As of 2025, Piper may not be in homebrew-core yet
        // Alternative: Manual installation similar to Linux
        
        println!("⚠️  Piper not available in Homebrew. Attempting manual installation...");
        
        // Fall back to manual installation
        self.install_linux() // Same process works on macOS
    }
    
    /// Install Piper on Windows
    fn install_windows(&self) -> Result<()> {
        println!("🪟 Windows installation:");
        println!("Please download Piper manually from:");
        println!("https://github.com/rhasspy/piper/releases/latest");
        println!("Extract to C:\\\\Program Files\\\\Piper\\ and add to PATH");
        
        anyhow::bail!("Manual installation required on Windows")
    }
    
    /// Download a voice model
    pub fn download_model(&self, voice: &str) -> Result<()> {
        let home = dirs::home_dir()
            .context("Failed to determine home directory")?;
        
        let models_dir = home.join(".hainet/models/piper");
        std::fs::create_dir_all(&models_dir)?;
        
        println!("📥 Downloading voice model: {}...", voice);
        
        // Piper models are hosted on HuggingFace
        let base_url = "https://huggingface.co/rhasspy/piper-voices/resolve/main";
        
        // Download .onnx model file
        let model_url = format!("{}/{}.onnx", base_url, voice);
        let model_path = models_dir.join(format!("{}.onnx", voice));
        
        let status = Command::new("curl")
            .arg("-L")
            .arg("-o")
            .arg(&model_path)
            .arg(&model_url)
            .status()
            .context("Failed to download voice model")?;
        
        if !status.success() {
            anyhow::bail!("Failed to download voice model");
        }
        
        // Download .onnx.json config file
        let config_url = format!("{}/{}.onnx.json", base_url, voice);
        let config_path = models_dir.join(format!("{}.onnx.json", voice));
        
        let status = Command::new("curl")
            .arg("-L")
            .arg("-o")
            .arg(&config_path)
            .arg(&config_url)
            .status()
            .context("Failed to download voice model config")?;
        
        if !status.success() {
            println!("⚠️  Warning: Failed to download model config (may not be critical)");
        }
        
        println!("✅ Voice model downloaded: {}", voice);
        
        Ok(())
    }
    
    /// List installed voice models
    pub fn list_models(&self) -> Result<Vec<String>> {
        let home = dirs::home_dir()
            .context("Failed to determine home directory")?;
        
        let models_dir = home.join(".hainet/models/piper");
        
        if !models_dir.exists() {
            return Ok(Vec::new());
        }
        
        let mut models = Vec::new();
        
        for entry in std::fs::read_dir(models_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("onnx") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    models.push(stem.to_string());
                }
            }
        }
        
        models.sort();
        Ok(models)
    }
    
    /// Get recommended voice model based on system tier
    pub fn recommended_model(&self) -> &'static str {
        match SystemTier::detect() {
            Ok(SystemTier::Tier1) => "en_US-lessac-low",      // Low quality for constrained systems
            Ok(SystemTier::Tier2) => "en_US-lessac-medium",   // Medium quality
            Ok(SystemTier::Tier3) => "en_US-lessac-high",     // High quality
            Ok(SystemTier::Tier4) => "en_US-amy-medium",      // Alternative high-quality voice
            Err(_) => "en_US-lessac-medium",                  // Fallback to medium quality
        }
    }
    
    /// Get Piper version
    pub fn version(&self) -> Option<String> {
        let piper_path = self.get_piper_path()?;
        
        let output = Command::new(piper_path)
            .arg("--version")
            .output()
            .ok()?;
        
        if output.status.success() {
            String::from_utf8(output.stdout)
                .ok()
                .map(|s| s.trim().to_string())
        } else {
            None
        }
    }
    
    // Private helper methods
    
    /// Get Piper executable path
    fn get_piper_path(&self) -> Option<PathBuf> {
        // Check PATH
        if let Ok(path) = which::which("piper") {
            return Some(path);
        }
        
        // Check common installation locations
        let home = dirs::home_dir()?;
        let candidates = vec![
            home.join(".local/bin/piper"),
            PathBuf::from("/usr/local/bin/piper"),
            PathBuf::from("/usr/bin/piper"),
        ];
        
        candidates.into_iter().find(|p| p.exists())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_piper_installer_creation() {
        let platform = Platform::detect().expect("Failed to detect platform");
        let installer = PiperInstaller::new(platform.clone());
        
        // Just verify it can be created
        println!("Piper installer created for platform: {}", platform);
    }
    
    #[test]
    fn test_recommended_model() {
        let platform = Platform::detect();
        let installer = PiperInstaller::new(platform);
        
        let model = installer.recommended_model();
        assert!(!model.is_empty());
        println!("Recommended model: {}", model);
    }
}
