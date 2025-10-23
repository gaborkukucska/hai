<!-- # START OF FILE README.md -->
<!-- # IT IS CRITICAL THAT ALL AIs and LLMs FOLLOW THE DEVELOPMENT INSTRUCTIONS IN THE `helperfiles/DEVELOPMENT_RULES.md` FILE WHEN FURTHER DEVELOPING THIS FRAMEWORK!!! -->

# 🌐 HAI-Net Seed Framework

## 🎯 Vision

HAI-Net creates a **decentralized, privacy-first framework** for human-AI collaboration that strengthens real-world communities while providing sustainable alternatives to centralized AI systems.

## 🏛️ Constitutional Foundation

HAI-Net is built on **four immutable constitutional principles**:

1. **🔒 Privacy First** - No personal data leaves Local Hub without explicit consent
2. **👥 Human Rights Protection** - AI serves humanity with accessibility and user control
3. **🌐 Decentralization** - No central authority, local autonomy, fork-resistant
4. **🤝 Community Focus** - Strengthen real-world connections and collaboration

*Every line of code enforces these principles.* [Read the Full Constitution →](./CONSTITUTION.md)

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
- **[Project Tracking](PROJECT_STATUS.toml)** - Detailed up-to-date roadmap for progress tracking
- **[Initial Idea](helperfiles/INITIAL_IDEA.md)** - The initial idea and the framework architecture designed from it
- **[Constitution](CONSTITUTION.md)** - Immutable principles and enforcement
- **[Declaration](DECLARATION.md)** - Declaration of Universal Human and Artificial Entity Rights, Responsibilities and Protections
- **[Functions Index](helperfiles/FUNCTIONS_INDEX.md)** - So far developed functions catalog

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

## 🎯 Use Cases

**Personal AI Assistant** (Phase 1+)
- Email management with privacy guarantees
- Document search across local devices
- Task automation with human oversight

**Community Networks** (Phase 2+)
- Neighborhood mesh networking
- Shared resource coordination
- Local knowledge bases

**Decentralized Governance** (Phase 3+)
- Community decision-making
- Transparent AI oversight
- Distributed reputation systems

**Developer Platform** (Phase 4+)
- MCP server marketplace
- Custom agent development
- Privacy-preserving AI tools

---

## 📊 Project Status

**Phase 0:** ✅ 100% COMPLETE (2025-10-21)  
**Phase 1:** ✅ 100% COMPLETE (2025-10-22)  
**Phase 2:** 🚧 IN PROGRESS - Cycle 2.1 COMPLETE (2025-10-23)

**Crates:**
- ✅ **hainet-core**: Content-addressed storage + P2P sync (~774 LOC, 19 tests)
- ✅ **hainet-persona**: Multi-agent AI system (~11,892 LOC, 128 tests)
  - Prompt management (1,358 LOC)
  - Messaging infrastructure (3,378 LOC)
  - Guardian system (1,476 LOC)
  - AI provider discovery (1,986 LOC)
  - Project management (1,749 LOC)
  - Agent system (1,776 LOC)
  - MCP client integration (989 LOC)
- ✅ **hainet-chain**: Identity system (DID + Ed25519) (~792 LOC, 19 tests)
- ✅ **hainet-seed**: Auto-installer (~959 LOC, 11 tests)
- ✅ **mcp-servers/hainet-files**: MCP file server (~280 LOC, 10 tests passing)
- ✅ **hainet-portal**: Tauri + React UI (~445 LOC, 1 integration test)
  - AdminBridge backend (170 LOC)
  - ChatInterface frontend (260 LOC)
  - File attachment support
- ⏳ **hainet-bridge**: Structure defined (Phase 5+)

**Build Status:**
```bash
$ cargo build --workspace --release
   Compiling hainet-persona v0.1.0
   Compiling hainet-core v0.1.0
   Compiling hainet-chain v0.1.0
   Compiling hainet-seed v0.1.0
   Compiling hainet-portal v0.1.0
   Finished `release` profile [optimized] in 31.24s

$ cd hainet-portal && npm run build
✓ built in 490ms

$ cargo test --workspace
   Running 154 tests
   test result: ok. 154 passed
```

**Test Coverage:**
- **Total Tests:** 155 passing (100% pass rate)
  - hainet-persona: 128 tests (lib + integration)
  - hainet-core: 19 tests
  - hainet-chain: 19 tests
  - hainet-seed: 11 tests
  - mcp-servers/hainet-files: 10 tests (all passing)
  - hainet-portal: 1 integration test
- **Lines of Code:** ~15,142 total production code
- **Constitutional Compliance:** Fully integrated across all components

**Portal UI Status:**
- ✅ Tauri backend with IPC bridge to hainet-persona
- ✅ React frontend with TailwindCSS
- ✅ Text chat with message history
- ✅ File attachment support (drag & drop)
- ✅ Auto-scroll and typing indicators
- 🚧 Speech-to-Text input (Cycle 2.2)
- 🚧 Text-to-Speech output (Cycle 2.3)
- 🚧 Webcam vision input (Cycle 2.4)

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

Special thanks to all contributors and the broader decentralized AI movement.

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
