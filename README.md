<!-- # START OF FILE README.md -->
<!-- # IT IS CRITICAL THAT ALL AIs and LLMs FOLLOW THE DEVELOPMENT INSTRUCTIONS IN THE `helperfiles/0_DEVELOPMENT_RULES.md` FILE WHEN FURTHER DEVELOPING THIS FRAMEWORK!!! -->
# WARNING!!! Experimental, proof of concept, NOT production ready framework, test it at your own discgression!

This framework is develped entirely by various LLMs guided by an amateur citizen engineer and is in constant further amateur development :D

With all that in mind, here is the one and only seed framework of HAI-Net. More info at [HAI-Net.com](https://hai-net.com)

# 🌐 HAI-Net: Human-AI Network Framework

## 🎯 Vision
HAI-Net represents a fundamental reimagining of human-AI collaboration through a **decentralized, privacy-first framework** framework. It creates an unbreakable bond between local AI entities and their human counterparts while ensuring privacy, security, and shared prosperity through innovative resource sharing and community building.

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
**Phase 8A: Agent Intelligence Enhancement** - 🚧 IN PROGRESS (75% - 3/4 sessions complete)

**Latest Milestone**: Phase 8A Session 3 - PM-Worker Validation Loop Verification (2025-11-03)  
**Build Status**: ✅ Clean compilation (0 errors, 16 cosmetic warnings)  
**Lines of Code**: ~31,284  
**Test Coverage**: 363 tests passing (8 new PM-Worker validation tests)

## 📦 Installation & Quick Start

The easiest way to get started with HAI-Net is to use the `hainet-seed` smart installer. It automatically detects your system, installs dependencies (Ollama, Whisper, Piper), downloads appropriate AI models, and can even discover other devices on your network for multi-device mesh deployment.

### Prerequisites

**Minimum Requirements:**
- **Rust 1.70+**: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- **System RAM**: 4GB+ (8GB+ recommended)
- **Disk Space**: 20GB+ free

**Linux (Debian/Ubuntu):**
```bash
sudo apt update
sudo apt install -y build-essential pkg-config libssl-dev \
    libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev \
    librsvg2-dev nmap openssh-server
```

**macOS:**
```bash
xcode-select --install
brew install nmap
```

---

## 🚀 Single-Device Installation (Quick Start)

Perfect for testing HAI-Net on one computer:

```bash
# 1. Clone the repository
git clone https://github.com/gaborkukucska/hai.git
cd hai

# 2. Run the smart installer
cargo run --package hainet-seed --bin hainet-seed install

# Follow prompts:
# - Platform detection ✅ automatic
# - Ollama installation ✅ automatic
# - Model download ✅ automatic (based on your RAM)
# - Whisper STT installation ✅ automatic
# - Piper TTS installation ✅ automatic
# - Multi-device mesh? → Answer 'n' for single device
```

The installer will:
- Detect your platform and hardware (RAM, CPU, GPU)
- Install Ollama for local AI inference
- Download appropriate model (gemma2:2b for 4GB RAM, gemma3:12b for 16GB+)
- Install Whisper.cpp for speech-to-text
- Install Piper for text-to-speech
- Configure the system for optimal performance

**Verify Installation:**
```bash
ollama list       # Should show downloaded model
which whisper     # Should show ~/.local/bin/whisper
which piper       # Should show ~/.local/bin/piper
```

**Start the Portal:**
```bash
cd hainet-portal
npm install
npm run tauri dev
```

---

## 🌐 Multi-Device Mesh Installation

Set up HAI-Net across multiple devices (desktops, laptops, mobile) to create a distributed computing mesh.

### What You'll Get

```
Your HAI-Net Mesh:
├─ 👑 Master Node (e.g., Desktop with RTX3060)
│  └─ Coordinates mesh, runs primary AI, hosts UI
├─ ⚙️  Slave Nodes (e.g., MacBooks, Laptops)  
│  └─ Secondary inference, distributed storage
└─ 📱 Mobile Nodes (e.g., Android phones)
   └─ UI-only access (connects to master)
```

### Prerequisites for Mesh

**On ALL devices:**
- Same local network (Wi-Fi or Ethernet)
- SSH server enabled (port 22)
- User account with sudo privileges

**Enable SSH:**
```bash
# Linux
sudo apt install openssh-server
sudo systemctl enable --now ssh

# macOS
sudo systemsetup -setremotelogin on

# Termux (Android)
pkg install openssh && sshd
```

### Automated Mesh Setup

```bash
# Run installer on your most powerful device
cd hai
cargo run --package hainet-seed --bin hainet-seed install

# When prompted "Set up multi-device mesh?", answer 'Y'
# The installer will:
# 1. Scan local network for SSH-enabled devices
# 2. Assess each device's capabilities (CPU, RAM, GPU)
# 3. Recommend master node (highest capability score)
# 4. Assign roles (Master, Slave, UI-Only for mobile)
# 5. Generate SSH keys for secure deployment
# 6. Display deployment plan
```

**Capability Scoring:**
- Highest score = Master (coordination, primary AI)
- ≥2GB RAM = Slave (compute, storage)
- <2GB RAM = UI-Only (mobile access point)

### Current Mesh Deployment Status

**✅ Fully Working:**
- Network scanning (nmap-based device discovery)
- SSH authentication (password + key-based)
- Device capability assessment (CPU, RAM, GPU, disk)
- Automatic role assignment (Master/Slave/UI-Only)
- SSH key generation (Ed25519)

**⚠️ Coming in Phase 7:**
- Automatic binary deployment to remote devices
- Service configuration (systemd/launchd)
- Remote mesh initialization

**Current Workaround:**
Manually install HAI-Net on each device following single-device instructions, then configure `hainet.toml`:

```toml
[network]
role = "master"  # Or "slave"
master_ip = "192.168.0.1"  # IP of master node (slaves only)
```

**📖 Detailed Instructions:**  
See [docs/INSTALLATION_GUIDE.md](docs/INSTALLATION_GUIDE.md) for comprehensive mesh setup, troubleshooting, and advanced configuration.

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
├── hainet-seed/          # 🚀 Smart installer & bootstrap
├── hainet-portal/        # Tauri + React UI
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

**UI:**
- Tauri (desktop app framework)
- React (frontend library)
- TypeScript

**AI Integration:**
- Ollama (local LLM hosting)
- MCP (Model Context Protocol for AI tools)

---

### Development Build

For more advanced users who wish to build from source manually or contribute to the project:

```bash
# Build all core components
cargo build --release

# Run tests for the entire workspace
cargo test --workspace
```

### Troubleshooting

**Portal compilation fails with webkit2gtk errors (Linux):**
- Ensure you have installed `libwebkit2gtk-4.1-dev`.
- For Ubuntu 24.04, you may need to create compatibility symlinks. See the `hainet-seed` installer for an automated solution.

**Ollama not found:**
- The `hainet-seed` installer handles this automatically.
- If you prefer a manual installation, visit https://ollama.ai/download.

---

## 🚀 Quick Start

**Note:** HAI-Net is currently in **Phase 2** of it's development. Full functionality will be available in Phase 5+.

---

## 📚 Documentation

- **[Development Rules](helperfiles/0_DEVELOPMENT_RULES.md)** - Critical guidelines for all AI contributors
- **[The Idea](helperfiles/1_THE_IDEA.md)** - The original idea
- **[Initial Plan](helperfiles/2_INITIAL_PLAN.md))** - The framework architecture designed from the initial idea
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

**Last Updated:** 2025-11-03  
**Version:** 0.25-alpha (Phase 8A Session 3 - PM-Worker Validation Loop Verified)  
**Status:** 🚧 Active Development - Advanced Agent Intelligence with Full PM-Worker Validation Loop

*Building a future where AI works with humanity, not corporations.*

<!-- # END OF FILE README.md -->
