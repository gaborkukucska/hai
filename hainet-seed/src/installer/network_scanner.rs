//! # START OF FILE hainet-seed/src/installer/network_scanner.rs
//! Network scanner for discovering devices with SSH on the local network.
//! Uses nmap to scan for devices with port 22 (SSH) open.

use std::net::IpAddr;
use std::path::PathBuf;
use std::process::Command;
use regex::Regex;
use anyhow::{Result, Context, bail};

/// A candidate device discovered on the network with SSH enabled.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceCandidate {
    /// IP address of the device (e.g., "192.168.1.10")
    pub ip: String,
    /// Hostname from reverse DNS lookup (if available)
    pub hostname: Option<String>,
    /// MAC address (if available, requires root/sudo)
    pub mac_address: Option<String>,
}

/// Network scanner for discovering SSH-enabled devices on the local network.
pub struct NetworkScanner {
    nmap_path: PathBuf,
}

impl NetworkScanner {
    /// Create a new NetworkScanner.
    /// 
    /// # Errors
    /// Returns an error if nmap is not installed or not found in PATH.
    pub fn new() -> Result<Self> {
        // Check if nmap is installed
        let nmap_path = Self::find_nmap()?;
        
        // Verify nmap works
        let output = Command::new(&nmap_path)
            .arg("--version")
            .output()
            .context("Failed to execute nmap --version")?;
        
        if !output.status.success() {
            bail!("nmap is installed but not working properly");
        }
        
        Ok(Self { nmap_path })
    }
    
    /// Find nmap binary in PATH.
    fn find_nmap() -> Result<PathBuf> {
        // Try 'which nmap' on Unix-like systems
        if cfg!(unix) {
            let output = Command::new("which")
                .arg("nmap")
                .output()
                .context("Failed to run 'which nmap'")?;
            
            if output.status.success() {
                let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path_str.is_empty() {
                    return Ok(PathBuf::from(path_str));
                }
            }
        }
        
        // Try 'where nmap' on Windows
        if cfg!(windows) {
            let output = Command::new("where")
                .arg("nmap")
                .output()
                .context("Failed to run 'where nmap'")?;
            
            if output.status.success() {
                let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path_str.is_empty() {
                    return Ok(PathBuf::from(path_str));
                }
            }
        }
        
        bail!("nmap not found in PATH. Please install nmap first:\n\
               - Linux (Debian/Ubuntu): sudo apt install nmap\n\
               - Linux (Fedora/RHEL): sudo dnf install nmap\n\
               - Linux (Arch): sudo pacman -S nmap\n\
               - macOS: brew install nmap\n\
               - Termux: pkg install nmap");
    }
    
    /// Scan the local network for devices with SSH (port 22) open.
    /// 
    /// This performs the following steps:
    /// 1. Get the local IP address
    /// 2. Derive the subnet range (e.g., 192.168.1.0/24)
    /// 3. Run nmap to scan for devices with port 22 open
    /// 4. Parse the nmap output
    /// 5. Filter out the local machine
    /// 6. Return discovered devices
    /// 
    /// # Errors
    /// Returns an error if:
    /// - Cannot determine local IP address
    /// - nmap execution fails
    /// - Cannot parse nmap output
    pub fn scan_local_network(&self) -> Result<Vec<DeviceCandidate>> {
        println!("Scanning local network for devices with SSH...");
        
        // Step 1: Get local IP
        let local_ip = Self::get_local_ip()
            .context("Failed to determine local IP address")?;
        
        let local_ip_str = local_ip.to_string();
        println!("Local IP: {}", local_ip_str);
        
        // Step 2: Derive subnet
        let subnet = Self::derive_subnet(local_ip)
            .context("Failed to derive subnet range")?;
        
        println!("Scanning subnet: {}", subnet);
        
        // Step 3: Run nmap scan
        let output = Command::new(&self.nmap_path)
            .arg("-p")
            .arg("22")
            .arg("--open")      // Only show hosts with port 22 open
            .arg("-T4")         // Aggressive timing (faster scan)
            .arg("-oG")         // Greppable output
            .arg("-")           // Output to stdout
            .arg(&subnet)
            .output()
            .context("Failed to execute nmap")?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("nmap scan failed: {}", stderr);
        }
        
        // Step 4: Parse nmap output
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut devices = Self::parse_nmap_output(&stdout)?;
        
        // Step 5: Filter out the local machine
        devices.retain(|device| device.ip != local_ip_str);
        
        println!("Discovered {} devices with SSH enabled", devices.len());
        
        Ok(devices)
    }
    
    /// Parse nmap greppable output format.
    /// 
    /// Example format:
    /// ```text
    /// Host: 192.168.1.10 (hostname) Status: Up
    /// Host: 192.168.1.10 (hostname) Ports: 22/open/tcp//ssh///
    /// ```
    fn parse_nmap_output(output: &str) -> Result<Vec<DeviceCandidate>> {
        let mut devices = Vec::new();
        
        // Regex to match: Host: <IP> (<hostname>) ... Ports: 22/open/tcp
        let host_regex = Regex::new(r"Host: ([0-9.]+) \(([^)]*)\)").unwrap();
        let port_regex = Regex::new(r"Ports: 22/open/tcp").unwrap();
        
        for line in output.lines() {
            // Skip lines that don't contain port information
            if !port_regex.is_match(line) {
                continue;
            }
            
            // Extract IP and hostname
            if let Some(caps) = host_regex.captures(line) {
                let ip = caps.get(1).unwrap().as_str().to_string();
                let hostname_str = caps.get(2).unwrap().as_str();
                let hostname = if hostname_str.is_empty() {
                    None
                } else {
                    Some(hostname_str.to_string())
                };
                
                devices.push(DeviceCandidate {
                    ip,
                    hostname,
                    mac_address: None, // MAC requires root/sudo, not extracted for now
                });
            }
        }
        
        Ok(devices)
    }
    
    /// Get the local IP address of this machine.
    /// 
    /// Excludes loopback (127.0.0.1) and link-local (169.254.x.x) addresses.
    fn get_local_ip() -> Result<IpAddr> {
        use local_ip_address::local_ip;
        
        let ip = local_ip().context("Failed to get local IP address")?;
        
        // Validate it's not loopback or link-local
        match ip {
            IpAddr::V4(ipv4) => {
                if ipv4.is_loopback() {
                    bail!("Local IP is loopback (127.0.0.1), cannot determine actual network IP");
                }
                if ipv4.is_link_local() {
                    bail!("Local IP is link-local (169.254.x.x), not connected to network");
                }
            }
            IpAddr::V6(_) => {
                bail!("IPv6 not supported yet, please use IPv4 network");
            }
        }
        
        Ok(ip)
    }
    
    /// Derive the subnet range from a local IP address.
    /// 
    /// For IPv4: Replaces the last octet with 0 and appends /24.
    /// Example: 192.168.1.15 → 192.168.1.0/24
    /// 
    /// # Errors
    /// Returns an error if the IP is not IPv4 (IPv6 not supported yet).
    fn derive_subnet(ip: IpAddr) -> Result<String> {
        match ip {
            IpAddr::V4(ipv4) => {
                let octets = ipv4.octets();
                let subnet = format!("{}.{}.{}.0/24", octets[0], octets[1], octets[2]);
                Ok(subnet)
            }
            IpAddr::V6(_) => {
                bail!("IPv6 subnet derivation not implemented yet");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_derive_subnet_ipv4() {
        let ip: IpAddr = "192.168.1.15".parse().unwrap();
        let subnet = NetworkScanner::derive_subnet(ip).unwrap();
        assert_eq!(subnet, "192.168.1.0/24");
    }
    
    #[test]
    fn test_derive_subnet_different_range() {
        let ip: IpAddr = "10.0.5.42".parse().unwrap();
        let subnet = NetworkScanner::derive_subnet(ip).unwrap();
        assert_eq!(subnet, "10.0.5.0/24");
    }
    
    #[test]
    fn test_parse_nmap_output_single_device() {
        let output = "\
Host: 192.168.1.10 (laptop-01) Status: Up
Host: 192.168.1.10 (laptop-01) Ports: 22/open/tcp//ssh///
";
        let devices = NetworkScanner::parse_nmap_output(output).unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].ip, "192.168.1.10");
        assert_eq!(devices[0].hostname, Some("laptop-01".to_string()));
    }
    
    #[test]
    fn test_parse_nmap_output_multiple_devices() {
        let output = "\
Host: 192.168.1.10 (laptop-01) Status: Up
Host: 192.168.1.10 (laptop-01) Ports: 22/open/tcp//ssh///
Host: 192.168.1.20 (desktop-pc) Status: Up
Host: 192.168.1.20 (desktop-pc) Ports: 22/open/tcp//ssh///
Host: 192.168.1.30 () Status: Up
Host: 192.168.1.30 () Ports: 22/open/tcp//ssh///
";
        let devices = NetworkScanner::parse_nmap_output(output).unwrap();
        assert_eq!(devices.len(), 3);
        
        assert_eq!(devices[0].ip, "192.168.1.10");
        assert_eq!(devices[0].hostname, Some("laptop-01".to_string()));
        
        assert_eq!(devices[1].ip, "192.168.1.20");
        assert_eq!(devices[1].hostname, Some("desktop-pc".to_string()));
        
        assert_eq!(devices[2].ip, "192.168.1.30");
        assert_eq!(devices[2].hostname, None);
    }
    
    #[test]
    fn test_parse_nmap_output_no_devices() {
        let output = "\
# Nmap 7.80 scan initiated
# Ports scanned: TCP(1;22)
";
        let devices = NetworkScanner::parse_nmap_output(output).unwrap();
        assert_eq!(devices.len(), 0);
    }
    
    #[test]
    fn test_parse_nmap_output_closed_ports() {
        // Devices with port 22 closed should not appear
        let output = "\
Host: 192.168.1.10 (laptop-01) Status: Up
Host: 192.168.1.10 (laptop-01) Ports: 22/closed/tcp//ssh///
Host: 192.168.1.20 (desktop-pc) Status: Up
Host: 192.168.1.20 (desktop-pc) Ports: 22/open/tcp//ssh///
";
        let devices = NetworkScanner::parse_nmap_output(output).unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].ip, "192.168.1.20");
    }
}
