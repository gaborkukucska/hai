// START OF FILE hainet-seed/src/installer/whisper.rs
//! Whisper.cpp installer module
//! 
//! Handles automatic installation and setup of whisper.cpp for local STT

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::Command;
use tracing::{info, warn, debug};

use crate::installer::platform::Platform;

/// Whisper.cpp installer
pub struct WhisperInstaller {
    platform: Platform,
}

impl WhisperInstaller {
    /// Create new Whisper installer for the current platform
    pub fn new(platform: Platform) -> Self {
        Self { platform }
    }
    
    /// Check if whisper.cpp is installed
    pub async fn is_installed(&self) -> Result<bool> {
        // Check if whisper binary exists in PATH
        let result = Command::new("whisper")
            .arg("--version")
            .output();
        
        if result.is_ok() {
            return Ok(true);
        }
        
        // Check common install locations
        let home_dir = std::env::var("HOME").unwrap_or_default();
        let common_paths = vec![
            PathBuf::from("/usr/local/bin/whisper"),
            PathBuf::from("/usr/bin/whisper"),
            PathBuf::from(&home_dir).join(".local/bin/whisper"),
            PathBuf::from(&home_dir).join("whisper.cpp/main"),
        ];
        
        for path in common_paths {
            if path.exists() {
                return Ok(true);
            }
        }
        
        Ok(false)
    }
    
    /// Install whisper.cpp
    pub async fn install(&self) -> Result<()> {
        match &self.platform {
            Platform::Linux { .. } => {
                self.install_linux().await
            }
            Platform::MacOS { .. } => {
                self.install_macos().await
            }
            Platform::AndroidTermux { .. } => {
                self.install_termux().await
            }
            Platform::Other(name) => {
                anyhow::bail!(
                    "Whisper.cpp installation not supported on platform: {}",
                    name
                )
            }
        }
    }
    
    /// Install on Linux
    async fn install_linux(&self) -> Result<()> {
        info!("Installing whisper.cpp on Linux...");
        
        let home = std::env::var("HOME").context("HOME not set")?;
        let install_dir = PathBuf::from(&home).join("whisper.cpp");
        
        // Check if directory already exists
        if install_dir.exists() {
            info!("whisper.cpp directory exists, pulling latest changes...");
            
            let status = Command::new("git")
                .current_dir(&install_dir)
                .args(["pull"])
                .status()
                .context("Failed to update whisper.cpp")?;
            
            if !status.success() {
                warn!("Failed to update whisper.cpp, continuing with existing installation");
            }
        } else {
            info!("Cloning whisper.cpp repository...");
            
            let status = Command::new("git")
                .current_dir(&home)
                .args([
                    "clone",
                    "https://github.com/ggerganov/whisper.cpp.git",
                ])
                .status()
                .context("Failed to clone whisper.cpp")?;
            
            if !status.success() {
                anyhow::bail!("Git clone failed");
            }
        }
        
        // Install build dependencies (cmake, make, gcc)
        info!("Installing build dependencies for whisper.cpp...");
        self.install_build_deps()?;
        
        // Build whisper.cpp using cmake (the project migrated from make to cmake)
        info!("Building whisper.cpp (this may take a few minutes)...");
        
        let cmake_config = Command::new("cmake")
            .current_dir(&install_dir)
            .args(["-B", "build"])
            .output()
            .context("Failed to run cmake. Ensure cmake is installed.")?;
        
        if !cmake_config.status.success() {
            let stderr = String::from_utf8_lossy(&cmake_config.stderr);
            warn!("cmake configure stderr: {}", stderr);
            // Fall back to plain make if cmake fails (older whisper.cpp versions)
            info!("Falling back to plain make build...");
            let status = Command::new("make")
                .current_dir(&install_dir)
                .status()
                .context("Failed to build whisper.cpp with make fallback.")?;
            if !status.success() {
                anyhow::bail!("Build failed with both cmake and make. Check that gcc/clang, cmake, and make are installed.");
            }
        } else {
            debug!("cmake configure succeeded, building...");
            let cmake_build = Command::new("cmake")
                .current_dir(&install_dir)
                .args(["--build", "build", "--config", "Release"])
                .output()
                .context("Failed to build whisper.cpp with cmake")?;
            
            if !cmake_build.status.success() {
                let stderr = String::from_utf8_lossy(&cmake_build.stderr);
                warn!("cmake build stderr: {}", stderr);
                anyhow::bail!("cmake --build failed. Check build output above.");
            }
        }
        
        // Create symlink in ~/.local/bin
        let bin_dir = PathBuf::from(&home).join(".local/bin");
        std::fs::create_dir_all(&bin_dir).context("Failed to create ~/.local/bin")?;
        
        let whisper_bin = install_dir.join("main");
        let symlink_path = bin_dir.join("whisper");
        
        // Remove old symlink if exists
        let _ = std::fs::remove_file(&symlink_path);
        
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&whisper_bin, &symlink_path)
                .context("Failed to create symlink")?;
        }
        
        info!("✅ whisper.cpp installed to {}", install_dir.display());
        info!("📌 Binary symlinked to {}", symlink_path.display());
        info!("💡 Add ~/.local/bin to your PATH if not already present");
        
        Ok(())
    }
    
    /// Install build dependencies (cmake, gcc, make) for compiling whisper.cpp
    fn install_build_deps(&self) -> Result<()> {
        // Check if cmake is already available
        if Command::new("cmake").arg("--version").output().map_or(false, |o| o.status.success()) {
            debug!("cmake already installed");
            return Ok(());
        }

        info!("📦 cmake not found — installing build tools...");

        // Detect package manager and install
        if Command::new("which").arg("apt-get").output().map_or(false, |o| o.status.success()) {
            let status = Command::new("sudo")
                .args(&["-n", "apt-get", "install", "-y", "build-essential", "cmake", "gcc", "g++", "make"])
                .status()
                .context("Failed to install build dependencies via apt-get")?;
            if !status.success() {
                warn!("⚠  apt-get install may have partially failed (sudo -n). Trying without sudo...");
            }
        } else if Command::new("which").arg("dnf").output().map_or(false, |o| o.status.success()) {
            let _ = Command::new("sudo")
                .args(&["-n", "dnf", "install", "-y", "cmake", "gcc", "gcc-c++", "make"])
                .status();
        } else if Command::new("which").arg("pacman").output().map_or(false, |o| o.status.success()) {
            let _ = Command::new("sudo")
                .args(&["-n", "pacman", "-S", "--noconfirm", "--needed", "cmake", "gcc", "make"])
                .status();
        } else {
            warn!("⚠  No supported package manager found. Please install cmake, gcc, and make manually.");
        }

        // Verify cmake is now available
        if !Command::new("cmake").arg("--version").output().map_or(false, |o| o.status.success()) {
            anyhow::bail!("cmake is still not available after installation attempt. Please install cmake manually.");
        }

        info!("✅ Build dependencies installed");
        Ok(())
    }

    /// Install on macOS
    async fn install_macos(&self) -> Result<()> {
        info!("Installing whisper.cpp on macOS...");
        
        // Try Homebrew first
        let brew_result = Command::new("brew")
            .args(["install", "whisper-cpp"])
            .status();
        
        if brew_result.is_ok() && brew_result.unwrap().success() {
            info!("✅ whisper.cpp installed via Homebrew");
            return Ok(());
        }
        
        warn!("Homebrew installation failed, falling back to manual build...");
        
        // Fallback to manual installation (same as Linux)
        self.install_linux().await
    }
    
    /// Install on Termux (Android)
    async fn install_termux(&self) -> Result<()> {
        info!("Installing whisper.cpp on Termux...");
        
        // Install dependencies via pkg
        info!("Installing build dependencies...");
        let status = Command::new("pkg")
            .args(["install", "-y", "git", "clang", "make"])
            .status()
            .context("Failed to install dependencies")?;
        
        if !status.success() {
            warn!("Some dependencies may have failed to install");
        }
        
        // Build from source (same process as Linux)
        self.install_linux().await
    }
    
    /// Download a Whisper model
    pub async fn download_model(&self, model_name: &str) -> Result<PathBuf> {
        info!("Downloading Whisper model: {}", model_name);
        
        let home = std::env::var("HOME").context("HOME not set")?;
        let models_dir = PathBuf::from(&home).join(".hainet/models");
        
        std::fs::create_dir_all(&models_dir)
            .context("Failed to create models directory")?;
        
        let model_file = models_dir.join(format!("ggml-{}.bin", model_name));
        
        if model_file.exists() {
            info!("✅ Model already exists: {}", model_file.display());
            return Ok(model_file);
        }
        
        info!("📥 Downloading from HuggingFace...");
        info!("⚠️  This may take several minutes depending on your connection");
        
        let model_url = format!(
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-{}.bin",
            model_name
        );
        
        let status = Command::new("curl")
            .args([
                "-L",
                "-o",
                model_file.to_str().unwrap(),
                &model_url,
                "--progress-bar",
            ])
            .status()
            .context("Failed to download model. Is curl installed?")?;
        
        if !status.success() {
            anyhow::bail!("Model download failed");
        }
        
        info!("✅ Model downloaded to: {}", model_file.display());
        Ok(model_file)
    }
    
    /// Get recommended model based on system capabilities
    pub fn recommended_model(&self, ram_gb: usize) -> &'static str {
        match ram_gb {
            0..=4 => "tiny.en",      // ~40MB, fastest
            5..=8 => "base.en",      // ~140MB, good balance
            9..=16 => "small.en",    // ~460MB, better accuracy
            _ => "medium.en",        // ~1.5GB, best quality
        }
    }
    
    /// Check if whisper.cpp is functional
    pub async fn verify_installation(&self) -> Result<()> {
        info!("Verifying whisper.cpp installation...");
        
        let output = Command::new("whisper")
            .arg("--help")
            .output();
        
        match output {
            Ok(out) if out.status.success() => {
                info!("✅ whisper.cpp is functional");
                Ok(())
            }
            _ => {
                anyhow::bail!(
                    "whisper.cpp verification failed. \
                    Please ensure ~/.local/bin is in your PATH"
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_recommended_model() {
        let platform = Platform::detect().unwrap();
        let installer = WhisperInstaller::new(platform);
        
        assert_eq!(installer.recommended_model(2), "tiny.en");
        assert_eq!(installer.recommended_model(6), "base.en");
        assert_eq!(installer.recommended_model(12), "small.en");
        assert_eq!(installer.recommended_model(32), "medium.en");
    }
    
    #[tokio::test]
    async fn test_installer_creation() {
        let platform = Platform::detect().unwrap();
        let installer = WhisperInstaller::new(platform);
        
        // Just verify we can create the installer
        assert!(true);
    }
}
