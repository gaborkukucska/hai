<!-- # START OF FILE docs/INSTALLATION_GUIDE.md -->
# HAI-Net Installation Guide

Complete guide for installing HAI-Net in single-device or multi-device mesh configurations.

---

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Quick Start (Single Device)](#quick-start-single-device)
3. [Multi-Device Mesh Setup](#multi-device-mesh-setup)
4. [Manual Configuration](#manual-configuration)
5. [Troubleshooting](#troubleshooting)
6. [Advanced Topics](#advanced-topics)

---

## Prerequisites

### Hardware Requirements

**Minimum (Tier 1 - Basic Operation):**
- CPU: 2 cores
- RAM: 4GB
- Disk: 20GB free space
- OS: Linux, macOS, or Windows (WSL2)

**Recommended (Tier 2 - Smooth Experience):**
- CPU: 4+ cores
- RAM: 8GB+
- Disk: 50GB+ free space
- GPU: Optional (for faster inference)

**Optimal (Tier 3/4 - Best Performance):**
- CPU: 8+ cores
- RAM: 16GB+
- Disk: 100GB+ free space
- GPU: NVIDIA RTX series (CUDA support)

### Software Requirements

**Required:**
- **Rust 1.70+**: Install from https://rustup.rs
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```

**Linux:**
```bash
# Debian/Ubuntu
sudo apt update
sudo apt install -y build-essential pkg-config libssl-dev \
    libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev \
    librsvg2-dev nmap

# Fedora
sudo dnf install -y gcc gcc-c++ make openssl-devel \
    webkit2gtk4.1-devel gtk3-devel libappindicator-gtk3-devel \
    librsvg2-devel nmap

# Arch Linux
sudo pacman -S base-devel webkit2gtk gtk3 libappindicator-gtk3 \
    librsvg nmap
```

**macOS:**
```bash
# Install Xcode Command Line Tools
xcode-select --install

# Install Homebrew (if not installed)
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# Install nmap
brew install nmap
```

**Windows:**
- Use WSL2 with Ubuntu 22.04+ and follow Linux instructions
- Native Windows support coming in Phase 7

---

## Quick Start (Single Device)

Perfect for trying HAI-Net on a single computer.

### Step 1: Clone the Repository

```bash
git clone https://github.com/gaborkukucska/hai.git
cd hai
```

### Step 2: Run the Smart Installer

```bash
cargo run --package hainet-seed --bin hainet-seed install
```

The installer will:
1. ✅ Detect your platform and hardware capabilities
2. ✅ Install Ollama (if not present)
3. ✅ Download appropriate AI model (based on your RAM)
4. ✅ Install Whisper.cpp (for speech-to-text)
5. ✅ Download Whisper model (tiny/base/small/medium based on RAM)
6. ✅ Install Piper TTS (for text-to-speech)
7. ✅ Download voice model (quality based on RAM)
8. ❓ Prompt: Set up multi-device mesh? (Y/n)

**For single-device setup, answer `n` to the mesh prompt.**

### Step 3: Verify Installation

```bash
# Check Ollama is running
ollama list

# Check Whisper is installed
which whisper

# Check Piper is installed
which piper
```

### Step 4: Start HAI-Net Portal (UI)

```bash
cd hainet-portal
npm install
npm run tauri dev
```

The HAI-Net Portal will open, and you can start interacting with your AI assistant.

---

## Multi-Device Mesh Setup

Set up HAI-Net across multiple devices (laptops, desktops, servers, mobile devices) to create a distributed mesh network.

### Architecture Overview

```
┌─────────────────────────────────────────────────────┐
│              HAI-Net Mesh Network                   │
├─────────────────────────────────────────────────────┤
│                                                      │
│  👑 Master Node (Desktop PC - RTX3060)              │
│     • Coordination & orchestration                  │
│     • Blockchain consensus                          │
│     • Primary AI inference                          │
│     • Storage coordination                          │
│     • User interface (Portal)                       │
│                                                      │
│  ⚙️  Slave Nodes (MacBooks, Laptops)                │
│     • Secondary inference                           │
│     • Distributed storage                           │
│     • Blockchain validation                         │
│     • Compute task execution                        │
│                                                      │
│  📱 Mobile Nodes (Galaxy phones/tablets)            │
│     • UI-only access (minimal resources)            │
│     • Connect to master for processing              │
│     • Remote access gateway                         │
│                                                      │
└─────────────────────────────────────────────────────┘
```

### Prerequisites for Mesh Setup

**All devices must:**
- Be on the same local network (Wi-Fi or Ethernet)
- Have SSH server enabled (port 22 open)
- Have a user account with sudo privileges
- Accept SSH connections (firewall configured)

**Enable SSH on each device:**

```bash
# Linux (Debian/Ubuntu)
sudo apt install openssh-server
sudo systemctl enable ssh
sudo systemctl start ssh

# macOS
sudo systemsetup -setremotelogin on

# Termux (Android - requires Termux app)
pkg install openssh
sshd
```

### Step-by-Step Mesh Deployment

#### **1. Run Installer on Primary Device**

Start on your most powerful device (this will likely become the Master):

```bash
cd hai
cargo run --package hainet-seed --bin hainet-seed install
```

#### **2. Respond to Mesh Setup Prompt**

When asked:
```
🌐 Set up multi-device mesh network? (Y/n):
```

Answer **`Y`** (or press Enter for default yes).

#### **3. Network Scanning**

The installer will:
- Install `nmap` if missing
- Scan your local network (e.g., 192.168.1.0/24)
- Discover all devices with SSH enabled (port 22 open)

Example output:
```
🔍 Discovering devices on local network...
✅ Discovered 5 devices with SSH enabled:
  [1] 192.168.1.10 (desktop)
  [2] 192.168.1.20 (macbook-pro)
  [3] 192.168.1.21 (macbook-air)
  [4] 192.168.1.50 (galaxy-s21)
  [5] 192.168.1.51 (galaxy-tab)
```

#### **4. Capability Assessment**

When prompted:
```
🔍 Assess device capabilities via SSH? (Y/n):
```

Answer **`Y`** to assess hardware.

**Provide SSH credentials (or skip devices):**
```
Username (default: current user, type 'skip' to ignore): tom
Password: ********
```

> [!TIP]
> If `nmap` discovers devices that are not part of your mesh (e.g., smart TVs, routers, or random IoT devices with port 22 open), you can explicitly ignore them by typing `skip` for the username. The installer will instantly bypass them without attempting an SSH connection.

The installer will SSH into each device and detect:
- CPU cores
- RAM (GB)
- GPU presence
- Available disk space
- OS and architecture

Example output:
```
📊 Device Capabilities Summary:
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Device: desktop (192.168.1.10)
  CPU: 8 cores
  RAM: 16.0 GB
  GPU: NVIDIA RTX3060
  Disk: 500.0 GB available
  OS: Linux (x86_64)
  Score: 152.0
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Device: macbook-pro (192.168.1.20)
  CPU: 8 cores
  RAM: 16.0 GB
  GPU: None
  Disk: 250.0 GB available
  OS: Darwin (arm64)
  Score: 106.0
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Device: galaxy-s21 (192.168.1.50)
  CPU: 8 cores
  RAM: 1.5 GB
  GPU: None
  Disk: 64.0 GB available
  OS: Linux (aarch64)
  Score: 12.4
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

🎯 Recommended Master Node: desktop (192.168.1.10)
   Score: 152.0 (Best hardware for coordination)
```

**Capability Scoring Formula:**
```
Score = (RAM_GB × 10 × 0.4) + (GPU_Present × 100 × 0.3) + 
        (CPU_Cores × 5 × 0.2) + (Disk_GB × 0.1)
```

- **RAM**: 40% weight (most important for AI inference)
- **GPU**: 30% weight (accelerates inference if present)
- **CPU**: 20% weight (parallel task execution)
- **Disk**: 10% weight (storage capacity)

#### **5. Role Assignment**

The installer automatically assigns roles:

**Master Node** (Highest score, ≥2GB RAM):
- HAI-Net Core (Master mode) - Mesh coordination
- HAI-Net Chain (Blockchain) - Consensus
- HAI-Net Bridge (Gateway) - External connectivity
- HAI-Net Portal (UI) - Primary interface

**Slave Nodes** (≥2GB RAM):
- HAI-Net Core (Slave mode) - Compute tasks
- HAI-Net Chain (Validator) - Blockchain validation

**UI-Only Nodes** (<2GB RAM, e.g., mobile devices):
- HAI-Net Portal (UI only) - Remote access
- Connects to master for all processing

Example output:
```
📋 Role Assignment:
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
👑 Master: desktop (192.168.1.10)
   Slave: macbook-pro (192.168.1.20)
   Slave: macbook-air (192.168.1.21)
📱 UI-Only: galaxy-s21 (192.168.1.50)
📱 UI-Only: galaxy-tab (192.168.1.51)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

#### **6. SSH Key Setup**

When prompted:
```
🚀 Deploy HAI-Net to discovered devices? (Y/n):
```

Answer **`Y`** to proceed.

The installer will:
1. Generate an Ed25519 SSH key pair (`~/.ssh/hainet-mesh`)
2. Display the public key location
3. Provide manual key setup instructions:

```
📋 SSH Key Setup:
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Public key location: /home/tom/.ssh/hainet-mesh.pub

For automatic deployment, copy the key to each device:

  Device: desktop (192.168.1.10)
  $ ssh-copy-id tom@192.168.1.10

  Device: macbook-pro (192.168.1.20)
  $ ssh-copy-id tom@192.168.1.20

Or manually append the key to ~/.ssh/authorized_keys on each device
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

**Copy keys to all devices** before continuing.

#### **7. Deployment Confirmation**

After key setup, the installer will display the deployment plan:

```
📋 Deployment Plan:
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
👑 Master - desktop (192.168.1.10)
  • hainet-core (Master mode)
  • hainet-chain (Blockchain)
  • hainet-bridge (Gateway)
  • hainet-portal (UI)
⚙️  Slave - macbook-pro (192.168.1.20)
  • hainet-core (Slave mode)
  • hainet-chain (Validator)
📱 UI-Only - galaxy-s21 (192.168.1.50)
  • hainet-portal (UI only - connects to home hub)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

⚠️  Ready to deploy. Continue? (Y/n):
```

#### **8. Current Limitation (Phase 6A)**

**⚠️ IMPORTANT**: The actual remote deployment step is currently **a placeholder**. 

The installer will display:
```
⚠️  Placeholder: Actual deployment steps
   Target: tom@192.168.1.10
   Role: Master
   Arch: x86_64

✓ Deployment to desktop complete (mock)
```

**This means:**
- ✅ Network scanning works
- ✅ SSH capability assessment works
- ✅ Role assignment works
- ✅ SSH key generation works
- ❌ **Binary deployment is not yet implemented**
- ❌ **Service configuration is not yet implemented**

---

## Manual Configuration (Current Workaround)

Until automatic deployment is complete, you can manually set up HAI-Net on each device:

### On the Master Node:

```bash
# Build HAI-Net
cd hai
cargo build --release

# Start Master services
export HAINET_ROLE=master
./target/release/hainet-core &
./target/release/hainet-chain &
./target/release/hainet-bridge &

# Start Portal UI
cd hainet-portal
npm install
npm run tauri dev
```

### On Slave Nodes:

```bash
# Clone and build HAI-Net
git clone https://github.com/gaborkukucska/hai.git
cd hai
cargo build --release

# Configure master IP
export HAINET_MASTER_IP=192.168.1.10

# Start Slave services
export HAINET_ROLE=slave
./target/release/hainet-core &
./target/release/hainet-chain &
```

### On Mobile Devices (Termux):

```bash
# Install Termux from F-Droid
# Install required packages
pkg install rust git nodejs

# Clone repository
git clone https://github.com/gaborkucska/hai.git
cd hai

# Build Portal only
cd hainet-portal
npm install
npm run dev
```

Access via browser: `http://192.168.1.10:3000` (master IP)

---

## Configuration Files

### hainet.toml (Project Root)

Main configuration file for HAI-Net:

```toml
[network]
role = "master"  # Or "slave", "standalone"
master_ip = "192.168.1.10"  # Only for slaves
port = 8080

[ai]
provider = "ollama"
endpoint = "http://localhost:11434"

[ai.defaults]
temperature = 0.7
max_tokens = 2048

[ai.admin]
model_size = "4B"
temperature = 0.7

[ai.pm]
model_size = "4B"
temperature = 0.3

[ai.worker]
model_size = "4B"
temperature = 0.1

[ai.guardian]
model_size = "7B"
temperature = 0.2

[storage]
base_path = "~/.hainet/storage"
max_cache_gb = 10

[blockchain]
consensus = "tendermint"
validator = true  # Only for master/slaves
```

---

## Troubleshooting

### Installation Issues

**Problem:** `cargo build` fails with linking errors
```
Solution:
# Linux - Install missing libraries
sudo apt install -y libssl-dev pkg-config

# macOS - Install Xcode Command Line Tools
xcode-select --install
```

**Problem:** Ollama installation fails
```
Solution:
# Manual Ollama installation
curl -fsSL https://ollama.ai/install.sh | sh

# Verify installation
ollama --version
```

**Problem:** Portal fails to start with webkit errors (Linux)
```
Solution:
# Ubuntu 24.04+
sudo apt install -y libwebkit2gtk-4.1-dev

# Older Ubuntu (20.04/22.04)
sudo apt install -y libwebkit2gtk-4.0-dev
```

### Mesh Setup Issues

**Problem:** No devices found during network scan
```
Solution:
1. Verify devices are on the same network:
   ip addr show  # Linux
   ifconfig      # macOS
   
2. Check SSH is enabled:
   sudo systemctl status ssh  # Linux
   sudo systemsetup -getremotelogin  # macOS
   
3. Test SSH connectivity:
   ssh user@192.168.1.XX
```

**Problem:** SSH authentication fails
```
Solution:
1. Verify SSH service is running:
   sudo systemctl start ssh  # Linux
   
2. Check firewall allows SSH (port 22):
   sudo ufw allow 22  # Linux
   
3. Test password authentication:
   ssh -o PreferredAuthentications=password user@ip
```

**Problem:** Device capability assessment hangs
```
Solution:
# SSH timeout issue - check network latency
ping -c 4 192.168.1.XX

# If high latency, use manual capability input instead
```

### Runtime Issues

**Problem:** Ollama not found after installation
```
Solution:
# Add to PATH
echo 'export PATH=$PATH:~/.local/bin' >> ~/.bashrc
source ~/.bashrc

# Verify
which ollama
```

**Problem:** AI model download is very slow
```
Solution:
# Use a smaller model for testing
ollama pull gemma2:2b  # Instead of 12b

# Or download in background
ollama pull gemma3:12b-it &
```

---

## Advanced Topics

### Cross-Platform Deployment

For deploying HAI-Net across different architectures (x86_64, ARM64, ARMv7):

```bash
# Install cross-compilation targets
rustup target add aarch64-unknown-linux-gnu
rustup target add armv7-unknown-linux-gnueabihf

# Build for ARM64 (e.g., MacBooks, Android)
cargo build --release --target aarch64-unknown-linux-gnu

# Build for ARMv7 (e.g., Raspberry Pi)
cargo build --release --target armv7-unknown-linux-gnueabihf
```

### Custom Model Configuration

Override default models in `hainet.toml`:

```toml
[ai.admin]
model = "llama3.1:8b"  # Specific model override
temperature = 0.8
max_tokens = 4096

[ai.guardian]
model = "qwen2.5:14b"  # Specialized reasoning model
temperature = 0.2
```

### Distributed Storage Configuration

For multi-device mesh, configure storage replication:

```toml
[storage]
replication_factor = 2  # Copies per file
max_storage_gb = 50     # Per device
sync_interval_secs = 300  # 5 minutes
```

---

## What's Next?

**Phase 7: Production Deployment** (Upcoming)
- Complete remote binary deployment
- Automatic service configuration (systemd/launchd)
- Health monitoring and auto-recovery
- Mesh network visualization

**Phase 8: Advanced Mesh Features**
- Cross-subnet mesh networking
- NAT traversal for remote devices
- Mobile device optimization
- Cloud fallback integration

---

## Getting Help

- **Documentation**: `helperfiles/` folder
- **GitHub Issues**: https://github.com/gaborkukucska/hai/issues
- **Development Rules**: `helperfiles/0_DEVELOPMENT_RULES.md`
- **Project Status**: `helperfiles/3_PROJECT_STATUS.toml`

---

**Last Updated**: 2025-10-31  
**Version**: 0.16-alpha  
**Phase**: 6A Complete - Installer Framework Ready

<!-- # END OF FILE docs/INSTALLATION_GUIDE.md -->
