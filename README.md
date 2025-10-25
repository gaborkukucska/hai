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
**Phase 2: HAI-Net Portal** - 🚧 IN PROGRESS (Cycle 2.1 Complete!)  
**Latest Milestone**: Phase 2.1 Complete - Core Portal Foundation (2025-10-23)  
**Build Status**: ✅ Clean compilation (Portal + Backend)  
**Lines of Code**: ~15,000+ (Phase 0: ~10,570, Phase 1: ~3,241, Phase 2: ~445)  
**Test Coverage**: 154 tests passing + 1 Portal integration test

## 🏗️ Architecture Overview

HAI-Net consists of six interconnected Rust crates:

```
hainet/
├── hainet-core/          # Main daemon & orchestration
├── hainet-persona/       # 🤖 Multi-agent AI system (PRIMARY FOCUS)
├── hainet-chain/         # Blockchain & governance
├── hainet-seed/          # Installation & bootstrap
├── hainet-portal/        # Web UI (Tauri + SvelteKit)
└── hainet-bridge/        # External API gateway
```

### Multi-Agent Architecture (hainet-persona)

**Hierarchical Communication:**
```
User ↔ Admin AI
        ↓
    PM Agents (Projects, Communications, Knowledge, System)
        ↓
    Worker Agents (Code, Email, Search, Create, etc.)
```

**Constitutional Guardians** monitor all agent interactions independently with pause/block authority.

**Agent State Machine:**
- Startup → Idle → Planning → Working → (Idle | Error)
- Each agent type has specialized prompts per state
- Constitutional compliance enforced at every state transition

### Prompt Management System (Completed ✅)

**Three-Tier Template Resolution:**
1. **Agent-Type-State Specific** (e.g., `admin-planning.toml`)
2. **Agent-Type Generic** (e.g., `admin.toml` + state injection)
3. **State Fallback** (e.g., `planning.toml`)

**Features:**
- TOML-based templates with Handlebars rendering
- Dynamic injection points for runtime context
- Constitutional compliance keywords validation
- LRU+TTL caching (1-hour default, 1000 entry max)
- Hot-reload with timestamp tracking
- Comprehensive validation reporting

---

## 🛠️ Technology Stack

**Core:**
- Rust 🦀 (async/await with Tokio)
- TOML (configuration & templates)
- Handlebars (template rendering)
- Anyhow/Thiserror (error handling)
- Tracing (structured logging)

**Blockchain:**
- Tendermint Consensus
- Ed25519 Signatures
- SHA3 Hashing

**Networking:**
- Libp2p (P2P mesh)
- mDNS (device discovery)
- WebRTC (real-time comms)

**UI:**
- Tauri (desktop app)
- SvelteKit (web framework)
- TypeScript

**AI Integration:**
- MCP Protocol (Model Context Protocol)
- Support for local & external LLMs
- Constitutional constraint system

---

## 📦 Installation

### Prerequisites

#### All Platforms
- **Rust 1.70+**: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- **Node.js 18+**: For Portal UI (https://nodejs.org/)
- **Ollama**: For AI model hosting (auto-installed by hainet-seed, or manual: https://ollama.ai/)

#### Linux (Ubuntu 24.04 / Debian-based)

```bash
# Install system dependencies
sudo apt update
sudo apt install -y \
    libsoup2.4-dev \
    libwebkit2gtk-4.1-dev \
    build-essential \
    libssl-dev \
    pkg-config \
    libgtk-3-dev
```

**Ubuntu 24.04 Compatibility Fix:**  
Ubuntu 24.04 only provides webkit2gtk-4.1, but some Tauri dependencies expect webkit2gtk-4.0. Create compatibility symlinks:

```bash
# Create symlinks for webkit2gtk-4.0 → 4.1 compatibility
sudo ln -s \
    /usr/lib/x86_64-linux-gnu/pkgconfig/javascriptcoregtk-4.1.pc \
    /usr/lib/x86_64-linux-gnu/pkgconfig/javascriptcoregtk-4.0.pc

sudo ln -s \
    /usr/lib/x86_64-linux-gnu/pkgconfig/webkit2gtk-4.1.pc \
    /usr/lib/x86_64-linux-gnu/pkgconfig/webkit2gtk-4.0.pc
```

#### macOS

```bash
# Install Homebrew if not already installed
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# Install dependencies
brew install pkg-config cairo pango gdk-pixbuf
```

### Build from Source

```bash
# Clone repository
git clone https://github.com/gaborkukucska/hai.git
cd hai

# Build core components
cargo build --release

# Build Portal (Tauri + React UI)
cd hainet-portal
npm install
npm run build
cd ..

# Run tests
cargo test --workspace
```

### Portal Installation

The **HAI-Net Portal** is the primary user interface providing multimodal interaction with the Admin AI.

```bash
# Install Portal frontend dependencies
cd hainet-portal
npm install

# Build frontend assets
npm run build

# Build Tauri backend (first time may take 5-10 minutes)
cargo build --release

# Run Portal in development mode
npm run tauri dev

# Or build standalone application
npm run tauri build
```

The Portal executable will be located at:
- **Linux**: `hainet-portal/src-tauri/target/release/hainet-portal`
- **macOS**: `hainet-portal/src-tauri/target/release/bundle/macos/HAI-Net Portal.app`
- **Windows**: `hainet-portal/src-tauri/target/release/hainet-portal.exe`

### Development Build

```bash
# Fast debug build (workspace root)
cargo build

# Watch mode (requires cargo-watch)
cargo install cargo-watch
cargo watch -x "build --package hainet-persona"

# Check without building
cargo check --workspace

# Portal development server (hot reload)
cd hainet-portal && npm run dev
```

### Troubleshooting

**Portal compilation fails with webkit2gtk errors:**
- Ensure you've installed `libwebkit2gtk-4.1-dev` and created the symlinks (Ubuntu 24.04)
- Run `pkg-config --list-all | grep webkit` to verify webkit2gtk-4.0.pc exists

**MCP servers compilation errors:**
- MCP integration is currently under migration to official Rust SDK
- Use `cargo build --workspace --exclude hainet-files` to skip MCP servers temporarily

**Ollama not found:**
- Run `hainet-seed` installer to auto-install Ollama
- Or install manually: https://ollama.ai/download

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

Special thanks to our AI contributors Claude & Jules, and the broader decentralized AI movement.

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
