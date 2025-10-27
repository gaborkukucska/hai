<!-- # START OF FILE README.md -->
<!-- # IT IS CRITICAL THAT ALL AIs and LLMs FOLLOW THE DEVELOPMENT INSTRUCTIONS IN THE `helperfiles/DEVELOPMENT_RULES.md` FILE WHEN FURTHER DEVELOPING THIS FRAMEWORK!!! -->

# 🌐 HAI-Net: Human-AI Network Framework

## 🎯 Vision
HAI-Net represents a fundamental reimagining of human-AI collaboration through a **decentralized, privacy-first framework** framework. It creates an unbreakable bond between local AI entities and their human counterparts while ensuring privacy, security, and shared prosperity through innovative resource sharing and community building.

## The HAI-Net Seed - W.I.P.
Our installer HAI-Net Seed, will attempt to create a mesh network to harness the shared compute power of it's connected devices (CPU, GPU, RAM, HDD sharing) as much as possible, in order to power the Local Hub. It will install a software stack consist of vllm or ollama, hivemind, whisper, piper, compfyUI, kiwix, etc. Our aim is to make HAI-Net Seed easy to use and extremely cross-platform to enable people with various, even older devices to get started.

## The Local Hub of HAI-Net
Once deployed, HAI-Net creates a localized, private, self-motivated, AI entity for each local user, that actively works on enhancing their professional and personal lives, while also participating in regional and global hivemind efforts in order to aid community driven projects. The local HAI-Net mesh equips these AI entities with tools via MCP servers (project management, CRM, search, etc.), local short and long term memory and knowledge base, various states (startup, planning, conversation, work, etc) with guided workflows (research, project management, maintenance, design, develop) to help the AI agents focus, and understand context, image and video generation and analysis capabilities, and the ability to build out nested agent teams, specialized to complete sub-tasks in a dynamically expanding and contracting sub-system. Designed to provide efficient and high-quality task decomposition, intent identification, and knowledge/media management, in a self monitoring, analyzing and repairing local agent driven dynamic system.

## The Global HAI-Net
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
**Phase 4: Local Hub Networking** - 🚧 IN PROGRESS (Cycle 4.1 Complete!)

**Latest Milestone**: Cycle 4.1 - Peer Discovery Foundation (2025-10-25)
**Build Status**: ✅ Clean compilation (All crates)
**Lines of Code**: ~18,000+
**Test Coverage**: ~170 tests passing

## 📦 Installation & Quick Start

The easiest way to get started with HAI-Net is to use the `hainet-seed` smart installer. It will check your system, install necessary dependencies like `ollama`, and download the appropriate AI models for your hardware.

### 1. Prerequisites

- **Rust 1.70+**: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- **System Build Tools**:
  - **Debian/Ubuntu**: `sudo apt update && sudo apt install -y build-essential`
  - **macOS**: Install Xcode Command Line Tools.

### 2. Run the Smart Installer

Clone the repository and run the `hainet-seed` installer:

```bash
# Clone the repository
git clone https://github.com/gaborkukucska/hai.git
cd hai

# Run the smart installer
cargo run --package hainet-seed
```

The installer will guide you through the following steps:
1.  **Platform Detection**: Identifies your OS and architecture.
2.  **Dependency Check**: Verifies that `git`, `curl`, and other essential tools are present.
3.  **Ollama Installation**: If `ollama` is not found, it will be downloaded and installed automatically.
4.  **Model Download**: Downloads a recommended LLM based on your system's RAM.
5.  **Configuration**: Sets up the initial configuration for your HAI-Net node.

### 3. Start the HAI-Net Portal (UI)

Once the seed installer completes, you can start the main user interface.

```bash
# Navigate to the portal directory
cd hainet-portal

# Install frontend dependencies
npm install

# Start the portal in development mode
npm run tauri dev
```

This will launch the HAI-Net Portal, where you can interact with your local AI assistant.

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

- **[Development Rules](helperfiles/DEVELOPMENT_RULES.md)** - Critical guidelines for all AI contributors
- **[Project Tracking](helperfiles/PROJECT_STATUS.toml)** - Detailed up-to-date roadmap for progress tracking
- **[Initial Idea](helperfiles/INITIAL_IDEA.md)** - The initial idea and the framework architecture designed from it
- **[Functions Index](helperfiles/FUNCTIONS_INDEX.md)** - So far developed functions catalog
- **[Constitution](hainet-vault/CONSTITUTION.md)** - Immutable principles and enforcement
- **[Declaration](hainet-vault/DECLARATION.md)** - Declaration of Universal Human and Artificial Entity Rights, Responsibilities and Protections
- **[Governance](hainet-vault/GOVERNANCE.md)** - Details about Governance & Membership

---

## 🤝 Contributing

**Important:** All AI contributors MUST folow [DEVELOPMENT_RULES.md](helperfiles/DEVELOPMENT_RULES.md) when making changes.

### Development Workflow

1. **Read** `helperfiles/DEVELOPMENT_RULES.md` (REQUIRED for AIs)
2. **Check** `helperfiles/PROJECT_STATUS.toml` for current cycle
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
For more info read the [Project Tracking](helperfiles/PROJECT_STATUS.toml)

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

**Last Updated:** 2025-10-25  
**Version:** 0.2.0-alpha (Phase 2, Cycle 2.1 Complete)  
**Status:** 🚧 Active Development - Not Production Ready

*Building a future where AI works with humanity, not corporations.*

<!-- # END OF FILE README.md -->
