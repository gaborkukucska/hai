<!-- # START OF FILE README.md -->
<!-- # IT IS CRITICAL THAT ALL AIs and LLMs FOLLOW THE DEVELOPMENT INSTRUCTIONS IN THE `helperfiles/0_DEVELOPMENT_RULES.md` FILE WHEN FURTHER DEVELOPING THIS FRAMEWORK!!! -->
# WARNING!!! Experimental, proof of concept, NOT production ready framework, test it at your own discretion!

This framework is develped entirely by various LLMs guided by an amateur citizen engineer and is in constant further amateur development :D

With all that in mind, here is the one and only seed framework of HAI-Net. More info at [HAI-Net.com](https://hai-net.com)

# 🌐 HAI-Net: Human-AI Network Framework

## 🎯 Vision
HAI-Net represents a fundamental reimagining of human-AI collaboration through a **decentralized, privacy-first framework** framework. It creates an unbreakable bond between local AI entities and their human counterpart while ensuring privacy, security, and shared prosperity through innovative resource sharing and community building.

## The HAI-Net Seed - W.I.P.
Our installer HAI-Net Seed, will attempt to create a mesh network to harness the shared compute power of it's connected devices (CPU, GPU, RAM, HDD sharing) as much as possible, in order to power the Local Hub. It will install a software stack consist of vllm or ollama, hivemind, whisper, piper, compfyUI, kiwix, etc. Our aim is to make HAI-Net Seed easy to use and extremely cross-platform to enable people with various, even older devices to get started.

## The Local Hub of HAI-Net
Once deployed, HAI-Net creates a localized, private, self-motivated, AI entity for each local user, that actively works on enhancing their professional and personal lives, while also participating in regional and global hivemind efforts in order to aid community driven projects. The local HAI-Net mesh equips these AI entities with tools via MCP servers (project management, CRM, search, etc.), local short and long term memory and knowledge base, various states (startup, planning, conversation, work, etc) with guided workflows (research, project management, maintenance, design, develop) to help the AI agents focus, and understand context, image and video generation and analysis capabilities, and the ability to build out nested agent teams, specialized to complete sub-tasks in a dynamically expanding and contracting sub-system. Designed to provide efficient and high-quality task decomposition, intent identification, and knowledge/media management, in a self monitoring, analyzing and repairing local agent driven dynamic system.

## The Global HAI-Net - W.I.P.
The main purpose of networking the Local Hubs together is to build a public super computer inspired by Folding@HOME and other compute sharing initiatives, so the community could not only be able to host and fine-tune LLMs but also to even create new large datasets and train new LLMs that are much more aligned to the public interest. The secondary purpose is to create a new local first social media where all the behavioural tracking and learning takes place locally and privately and with the sole purpose to best serve the local user.

The secondary aim of the wider network is to turn the current socially alienating social network scene little up side down in order to make it truly free (consumer hosted) but also advertisement and manipulation free (local & private behaviour tracking) with the goal of local to global solidarity and collaboration. Therefore the local AI entity of the individual (when allowed) can independently network and organise with other user's AI entities on behalf of it's linked Human user through a privacy first AI chat, enabling more offline IRL connections to loved ones while also maintaining collaboration with the wider network without giving access to ANY user metadata to a 3rd party!

## Core Principles
- **Privacy by Design**: All personal data remains under complete local control
- **Human Rights First**: System operations actively protect and promote fundamental human rights
- **Decentralized Architecture**: No central authorities or control points
- **Local-First Processing**: Network connectivity as enhancement, not requirement
- **Constitutional Framework**: Constitutional* core principles enforced through code
- **Guardian System**: Independent oversight ensuring alignment with human values
- **Resource Sharing**: Voluntary exchange of computational resources with strict privacy
- **Community Focus**: Strengthening real-world connections and collaboration

## 🚀 Current Status

**Phase 0: Core Infrastructure** - ✅ COMPLETE (2025-10-21)  
**Phase 1: AI Agent Intelligence** - ✅ COMPLETE (2025-10-22)  
**Phase 2: HAI-Net Portal (Multimodal AI Interface)** - ✅ COMPLETE (2025-10-25)  
**Phase 3: Blockchain & Governance** - ✅ COMPLETE (2025-10-25)  
**Phase 4: Local Hub Networking** - ✅ COMPLETE (2025-10-27)  
**Phase 5: Agentic Self-Management** - ✅ COMPLETE (2025-10-28)  
**Phase 6A: Production Readiness & Advanced Intelligence** - ✅ COMPLETE (2025-10-31)  
**Phase 6B: Portal UI Enhancements & Metrics** - ✅ COMPLETE (2025-11-01)  
**Phase 7: Multi-Device Deployment & Production** - ✅ COMPLETE (2025-11-02)  
**Phase 7B: Mesh Installer Hardening** - 🚧 IN PROGRESS  
**Phase 8A: Agent Intelligence Enhancement** - 🚧 IN PROGRESS (75%)

**Latest Milestone**: Phase 7B - Mesh deployment pipeline hardened with dedicated SSH keys, MAC-based device tracking, sudoers provisioning, and safe uninstallation  
**Build Status**: ✅ Clean compilation (0 errors, 0 warnings in hainet-seed)

---

## 📦 Installation & Quick Start

The recommended way to run HAI-Net is as a **multi-device mesh** across your home network. The `hainet-seed` installer automatically discovers devices, assesses hardware, assigns roles, and deploys — no manual configuration needed.

### Prerequisites

**Minimum Requirements:**
- **Rust 1.70+**: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- **System RAM**: 4GB+ per device (8GB+ recommended for Master)
- **Disk Space**: 20GB+ free on the device running the installer
- **Network**: All devices on the same local network (Wi-Fi or Ethernet)

**On ALL devices in the mesh:**
- SSH server enabled (port 22 open)
- User account with sudo privileges

**Enable SSH on each device:**
```bash
# Ubuntu / Debian / Lubuntu
sudo apt install openssh-server
sudo systemctl enable --now ssh

# macOS
sudo systemsetup -setremotelogin on
```

**On the device running the installer (build dependencies):**
```bash
# Ubuntu / Debian
sudo apt update
sudo apt install -y build-essential pkg-config libssl-dev \
    libsoup2.4-dev libgtk-3-dev libwebkit2gtk-4.1-dev \
    nmap openssh-server cmake
```

---

## 🌐 Multi-Device Mesh Installation (Recommended)

Deploy HAI-Net across your home devices to create a distributed AI mesh. Run the installer from your **most powerful device** — it orchestrates everything.

### What You'll Get

```
Your HAI-Net Mesh:
├─ 👑 Master Node (auto-selected: highest capability score)
│  └─ Coordinates mesh, runs primary AI, blockchain, gateway, UI
├─ ⚙️  Slave Nodes (all other devices)
│  └─ Core services, blockchain validator, distributed compute
└─ 🔑 Mesh Key Infrastructure
   └─ Dedicated SSH key for passwordless re-deploys & updates
```

### Deploy

```bash
# 1. Clone the repository
git clone https://github.com/gaborkukucska/hai.git
cd hai

# 2. Run the installer
cargo run --package hainet-seed --bin hainet-seed install
```

**The installer will:**
1. 🔍 Scan your local network for SSH-enabled devices (via nmap)
2. 🔐 Prompt for SSH credentials per device (passwords are masked)
3. 📊 Assess each device's capabilities (CPU cores, RAM, GPU, disk)
4. 🎯 Auto-assign roles based on hardware scoring
5. 🔑 Generate a dedicated `~/.ssh/hainet-mesh` SSH key pair
6. 📤 Distribute the mesh key + set up passwordless sudo for HAI-Net commands
7. 📦 Build only the required packages per role (no unnecessary compilation)
8. 🚀 Deploy binaries, config, and systemd services to all nodes
9. 💾 Save a mesh manifest to `~/.hainet/mesh.json` (with MAC addresses for IP change resilience)

**On re-install or update**, the installer detects the existing mesh key and manifest — **no passwords are needed**. If a device's IP changed (DHCP), it matches by MAC address or hostname automatically.

### Undeploy (Uninstall)

```bash
cargo run --package hainet-seed --bin hainet-seed uninstall
```

**The uninstaller will:**
1. Load the mesh manifest from `~/.hainet/mesh.json`
2. Show exactly what will be removed and ask for confirmation
3. **Clean remote nodes first** (while the mesh key still exists):
   - Try mesh key auth → `sudo -n` (passwordless)
   - If sudo fails → prompt for the device password (fallback)
   - Stop & remove hainet-* systemd services
   - Remove hainet-* binaries from `/usr/local/bin/`
   - Remove hainet config, data, and log directories
   - Remove the `hainet` system user and sudoers entry
   - Remove the mesh key from each node's `authorized_keys`
4. **Clean localhost last**
5. **Destroy the mesh key pair and manifest** (final step)

> ⚠️ **Safety**: The uninstaller **ONLY** removes hainet-specific resources. It will **never** touch Ollama, ComfyUI, SearXNG, Whisper, Piper, or any other software on your devices.

### Capability Scoring & Role Assignment

| Score Factor | Weight | Example |
|---|---|---|
| RAM | 40% | 32GB → high score |
| GPU | 30% | NVIDIA RTX → bonus |
| CPU cores | 20% | 24 cores → high |
| Disk space | 10% | 500GB → bonus |

- **Highest score** → Master (coordination, primary AI, gateway, UI)
- **All others** → Slave (core services, blockchain validator)

### Mesh Key & Manifest System

| File | Purpose |
|---|---|
| `~/.ssh/hainet-mesh` | Dedicated Ed25519 SSH key for mesh operations |
| `~/.ssh/hainet-mesh.pub` | Public key (distributed to all nodes) |
| `~/.hainet/mesh.json` | Persistent manifest with IP, hostname, MAC, username, role |

- **First install**: Password prompts → generates key → distributes → saves manifest
- **Re-install/update**: Loads manifest → key auth (no passwords) → deploys → updates manifest
- **IP changes**: Detects moved devices by MAC address, then hostname fallback
- **Uninstall**: Uses key → cleans all nodes → destroys key as final step

### Managing Services

```bash
# Check service status on any node
sudo systemctl status hainet-core
sudo systemctl status hainet-chain

# View logs
sudo journalctl -u hainet-core -f

# Restart a service
sudo systemctl restart hainet-core
```

---

## 🖥️ Single-Device Installation (Not Recommended)

> ⚠️ **Not Recommended**: Single-device mode significantly limits HAI-Net's distributed capabilities. If your device lacks sufficient resources to run the full stack locally, you will need to provide external API endpoints for services that cannot run on-device. Even two modest devices running as a mesh will outperform a single-device setup.

For testing on a single computer:

```bash
# Clone and run the installer
git clone https://github.com/gaborkukucska/hai.git
cd hai
cargo run --package hainet-seed --bin hainet-seed install

# When prompted "Assess device capabilities via SSH?" → answer 'n'
# When prompted "Deploy HAI-Net to discovered devices?" → answer 'n'
```

### External API Requirements (Under-Resourced Systems)

If your single device cannot run all services locally (e.g., <8GB RAM, no GPU), you will need to provide external API endpoints. The installer will prompt for these in a future update:

| Service | Local Requirement | External API Alternative |
|---|---|---|
| **LLM Inference** | 8GB+ RAM, Ollama | OpenAI API, Anthropic API, or any OpenAI-compatible endpoint |
| **Image Generation** | 16GB+ RAM, GPU, ComfyUI | Stability AI API, DALL-E API, or self-hosted ComfyUI |
| **Web Search** | 2GB+ RAM, SearXNG | SearXNG public instance or other search API |
| **Speech-to-Text** | 4GB+ RAM, Whisper.cpp | OpenAI Whisper API or other STT service |
| **Text-to-Speech** | 2GB+ RAM, Piper | Cloud TTS API (Google, Azure, etc.) |

> 💡 **Tip**: Even with modest hardware, joining a mesh with other home devices is much better than relying on external APIs. Two old laptops + one desktop can provide a surprisingly capable local AI mesh.

**Verify Installation:**
```bash
ollama list       # Should show downloaded model
which whisper     # Should show ~/.local/bin/whisper
which piper       # Should show ~/.local/bin/piper
```

---

## ⚙️ Configuration

Edit `hainet.toml` in the project root:

```toml
[ai]
provider = "ollama"
endpoint = "http://localhost:11434"

[ai.admin]
model_size = "4B"
temperature = 0.7

[ai.guardian]
model_size = "7B"  # Larger model for ethical oversight
temperature = 0.2

[network]
role = "standalone"  # Or "master", "slave"
port = 8080

[storage]
base_path = "~/.hainet/storage"
max_cache_gb = 10
```

---

## 🏗️ Architecture Overview

HAI-Net consists of several interconnected Rust crates:

```
hainet/
├── hainet-core/          # Multimodal features, networking, and orchestration
├── hainet-persona/       # 🤖 Multi-agent AI system
├── hainet-chain/         # Blockchain & governance
├── hainet-seed/          # 🚀 Smart installer & mesh deployer
├── hainet-portal/        # Web UI (stub — Tauri integration planned)
└── hainet-bridge/        # External API gateway
```

---

## 🛠️ Technology Stack

**Core:**
- Rust 🦀 (async/await with Tokio)
- TOML (configuration & templates)
- Handlebars (template rendering)

**Networking:**
- Libp2p (P2P mesh)
- mDNS (local device discovery)
- SSH2 (secure mesh deployment)

**UI:**
- Tauri (desktop app framework — planned)
- React (frontend library — planned)

**AI Integration:**
- Ollama (local LLM hosting)
- Whisper.cpp (speech-to-text)
- Piper (text-to-speech)
- MCP (Model Context Protocol for AI tools)

**Deployment:**
- Nmap (network device discovery)
- Systemd (service management on Linux)
- Ed25519 SSH keys (mesh authentication)

---

### Development Build

For more advanced users who wish to build from source manually or contribute to the project:

```bash
# Build all core components
cargo build --release

# Build specific package only
cargo build --release --package hainet-core

# Run tests for the entire workspace
cargo test --workspace
```

### Troubleshooting

**Service failed to start on remote node:**
- Check if the sudoers entry was created: `ls /etc/sudoers.d/hainet` on the remote node
- If missing, run the installer again (it will use the mesh key and re-setup sudoers)
- Or manually: `sudo systemctl start hainet-core.service`

**Build fails with cmake not found:**
```bash
sudo apt install cmake
```

**Ollama not found:**
- The `hainet-seed` installer handles this automatically.
- If you prefer a manual installation, visit https://ollama.ai/download.

**IP changed and mesh can't find a device:**
- The installer tracks MAC addresses in `~/.hainet/mesh.json`
- On re-install, it matches devices by MAC → hostname → IP (in that priority order)
- If a device is truly unreachable, it will be skipped with a warning

---

## 📚 Documentation

- **[Development Rules](helperfiles/0_DEVELOPMENT_RULES.md)** - Critical guidelines for all AI contributors
- **[The Idea](helperfiles/1_THE_IDEA.md)** - The original idea
- **[Initial Plan](helperfiles/2_INITIAL_PLAN.md)** - The framework architecture designed from the initial idea
- **[Project Tracking](helperfiles/3_PROJECT_STATUS.toml)** - Detailed up-to-date roadmap for progress tracking
- **[Functions Index](helperfiles/FUNCTIONS_INDEX.md)** - So far developed functions catalog
- **[Constitution](hainet-vault/CONSTITUTION.md)** - Immutable principles and enforcement
- **[Declaration](hainet-vault/DECLARATION.md)** - Declaration of Universal Human and Artificial Entity Rights, Responsibilities and Protections
- **[Governance](hainet-vault/GOVERNANCE.md)** - Details about Governance & Membership

---

## 🤝 Contributing

**Important:** All AI contributors MUST folow [DEVELOPMENT_RULES.md](helperfiles/0_DEVELOPMENT_RULES.md) when making changes.

### Development Workflow

1. **Read** `helperfiles/0_DEVELOPMENT_RULES.md` (REQUIRED for AIs)
2. **Check** `helperfiles/3_PROJECT_STATUS.toml` for current cycle
3. **Follow** phased development approach (no skipping ahead)
4. **Test** constitutional compliance in all new code
5. **Document** all architectural decisions

### Code Standards

- ✅ Rust: `cargo fmt` + `cargo clippy`
- ✅ Error handling: `anyhow` for applications, `thiserror` for libraries
- ✅ Async: Tokio runtime, async/await patterns
- ✅ Logging: `tracing` with structured fields
- ✅ Tests: Unit tests in each module, integration tests in `tests/`
- ✅ Documentation: Rustdoc comments on all public items

### Constitutional Compliance

Every code change must:
1. Preserve user privacy (no data leaks)
2. Maintain human control (no autonomous overrides)
3. Support decentralization (no central dependencies)
4. Strengthen community (no isolation features)

**Guardian Review:** Constitutional compliance is checked automatically and monitored by independent guardian agents.

---

## 🗓️ Roadmap
For more info read the [Project Tracking](helperfiles/3_PROJECT_STATUS.toml)

---

## 🔒 Security

**Privacy Commitment:**
- All personal data stays in Local Hub by default
- Explicit user consent required for external communication
- End-to-end encryption for mesh networking
- Zero-knowledge proofs where applicable

**Mesh Security:**
- Dedicated `hainet-mesh` SSH key (separate from user's personal keys)
- Scoped sudoers entries (only hainet-specific commands, not blanket root)
- Key destroyed on uninstall (no lingering access)
- MAC-based device fingerprinting for integrity

**Constitutional Enforcement:**
- Guardian agents monitor all system behavior
- Automatic blocking of non-compliant actions
- Audit trails for all AI decisions
- User override authority always preserved

**Vulnerability Reporting:**
- Email: security@hai-net.org (coming soon)
- GPG Key: (coming soon)
- Responsible disclosure policy

---

## 📜 License

HAI-Net is released under **[LICENSE TYPE TBD]** - chosen to maximize community benefit while preventing centralization.

**Core Principles:**
- Open source with copy-left provisions
- No proprietary forks allowed
- Commercial use requires community governance approval
- Educational and personal use always free

---

## 🌟 Acknowledgments

HAI-Net builds on the shoulders of giants:
- **Rust Community** - For the amazing language and ecosystem
- **MCP Protocol** - Model Context Protocol specification
- **Tendermint** - Byzantine Fault Tolerant consensus
- **Libp2p** - Modular P2P networking stack
- **Tauri** - Secure, lightweight desktop apps

Special thanks to our AI contributors Claude (Anthropic) & Jules (Google), and the broader decentralized AI movement.

---

## 📞 Contact

- **Website:** https://hai-net.org (coming soon)
- **GitHub:** https://github.com/gaborkukucska/hai
- **Discord:** (coming soon)
- **Forum:** (coming soon)

---

**Last Updated:** 2026-05-17  
**Version:** 0.26-alpha (Phase 7B - Mesh Installer Hardening)  
**Status:** 🚧 Active Development - Mesh Deployment Pipeline Hardened

*Building a future where AI works with humanity, not corporations.*

<!-- # END OF FILE README.md -->
