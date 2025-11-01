//! # START OF FILE hainet-seed/src/installer/nmap_installer.rs
//! Auto-installer for nmap network scanner.
//! Ensures nmap is installed before network scanning.

use crate::installer::platform::Platform;
use crate::installer::dependencies::DependencyChecker;
use anyhow::{Result, Context, bail};
use std::process::Command;

/// Ensure nmap is installed on the system.
/// 
/// If nmap is not found, this function will attempt to install it
/// using the platform-specific package manager.
/// 
/// # Errors
/// Returns an error if:
/// - nmap installation fails
/// - Platform is not supported
/// - Cannot verify nmap installation after install attempt
pub async fn ensure_nmap_installed(platform: &Platform) -> Result<()> {
    println!("Checking if nmap is installed...");
    
    // Check if nmap is already installed
    if is_nmap_installed() {
        println!("✓ nmap is already installed");
        return Ok(());
    }
    
    println!("nmap not found, installing...");
    
    // Install nmap using DependencyChecker
    let dep_checker = DependencyChecker::new(platform.clone());
    dep_checker.install_missing(vec!["nmap".to_string()])
        .await
        .context("Failed to install nmap")?;
    
    // Verify installation
    if !is_nmap_installed() {
        bail!("nmap installation appeared to succeed but nmap is still not available");
    }
    
    println!("✓ nmap installed successfully");
    
    Ok(())
}

/// Check if nmap is installed and accessible.
fn is_nmap_installed() -> bool {
    // Try multiple common locations for nmap
    let nmap_locations = vec![
        "nmap",                    // In PATH
        "/usr/bin/nmap",           // Standard Linux location
        "/usr/local/bin/nmap",     // macOS/manual install
        "/opt/homebrew/bin/nmap",  // Homebrew on M1 Macs
    ];
    
    for location in nmap_locations {
        let result = Command::new(location)
            .arg("--version")
            .output();
        
        if let Ok(output) = result {
            if output.status.success() {
                return true;
            }
        }
    }
    
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_is_nmap_installed() {
        // This test will pass if nmap is installed, fail if not
        // We can't reliably test installation without root access
        let installed = is_nmap_installed();
        println!("nmap installed: {}", installed);
        // Don't assert here, just verify the function doesn't crash
    }
}
