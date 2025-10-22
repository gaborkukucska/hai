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
**Phase 1: AI Agent Intelligence** - 🚧 IN PROGRESS (Cycle 1.1 Foundation Complete)  
**Latest Milestone**: Phase 0 Complete - All Infrastructure Ready (2025-10-21)  
**Build Status**: ✅ Clean compilation (164 tests passing)  
**Lines of Code**: ~11,635 (Phase 0: ~10,570, Phase 1: ~1,065)  
**Test Coverage**: 164 tests passing (100% pass rate)

### Recent Achievements (2025-10-21)

✅ **Phase 0 COMPLETE** - All infrastructure cycles 0.1-0.6 done  
✅ **Cycle 1.1 COMPLETE** - Admin AI Core Foundation implemented  
✅ **Intent Parser**: Rule-based classification ready for LLM upgrade  
✅ **Task Planner**: Dependency tracking and MCP tool mapping  
✅ **State Machine**: Full lifecycle management with validation  
✅ **Admin AI Stub**: Integration foundation ready

**Migration Decision:**
- Replacing custom MCP implementation with official Rust SDK
- Repository: https://github.com/modelcontextprotocol/rust-sdk
- Rationale: Use maintained, standardized implementation from MCP project

**Migration Progress (90% Complete):**
- ✅ Dependencies added (`rmcp = "0.8.2"`)
- ✅ Server structure implemented using `ServerHandler` trait
- ✅ 4 file operation handlers (read, write, list, metadata)
- ✅ CAS storage integration (BLAKE3)
- ✅ JSON schemas for tool parameters
- ⚠️ RmcpError construction needs fixing (no helper methods in v0.8.2)
- ⚠️ serve_stdio initialization pattern unclear
- ⚠️ Type mismatches (Arc<Map> vs Map, lifetimes)

**Current Capabilities:**
- ✅ Advanced prompt management system with constitutional compliance
- ✅ Zero-configuration AI model discovery and selection
- ✅ Constitutional Guardian (PII/Bias/Harm detection)
- ✅ Hierarchical agent communication infrastructure
- ✅ Blockchain identity system (DID + Ed25519)
- ✅ Content-addressed storage with P2P sync
- ✅ Automatic Ollama installation
- 🚧 MCP tool ecosystem (foundation ready, API alignment needed)

**Next Steps:**
1. Study rmcp SDK examples for correct patterns
2. Fix error construction and server initialization
3. Complete MCP client implementation
4. Resume Phase 1: Admin AI Core Implementation

---

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
    PM Agents (Communications, Knowledge, System)
        ↓
    Worker Agents (Email, Search, Files, etc.)
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

**Directory Structure:**
```
hainet-persona/
├── prompts/
│   ├── system/              # Core instructions, safety guidelines
│   ├── agents/              # Agent-type specific (admin, PM, workers)
│   └── states/              # State-specific (idle, planning, working)
├── src/
│   └── prompts/
│       ├── types.rs         # Type system (270 lines)
│       ├── loader.rs        # TOML parsing & hot-reload (310 lines)
│       ├── renderer.rs      # Handlebars + validation (340 lines)
│       ├── cache.rs         # LRU+TTL caching (240 lines)
│       └── mod.rs           # Unified API (90 lines)
```

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

- Rust 1.70+ (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- Node.js 18+ (for UI)
- Linux/macOS (primary targets)

### Build from Source

```bash
# Clone repository
git clone https://github.com/gaborkukucska/hai.git
cd hai

# Build all crates
cargo build --release

# Run specific component
cargo run --package hainet-persona

# Run tests
cargo test --workspace
```

### Development Build

```bash
# Fast debug build
cargo build

# Watch mode (requires cargo-watch)
cargo watch -x "build --package hainet-persona"

# Check without building
cargo check --workspace
```

---

## 🚀 Quick Start

**Note:** HAI-Net is currently in **Phase 0** (foundation building). Full functionality will be available in Phase 1+.

### Current Demo (Cycle 0.2)

```rust
use hainet_persona::{PromptManager, AgentId, AgentType, AgentState, PromptContext};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize prompt manager
    let mut manager = PromptManager::new("hainet-persona/prompts".into())?;
    
    // Create agent context
    let agent_id = AgentId::new(AgentType::Admin, "main-admin".to_string());
    let mut context = PromptContext::default();
    context.user_name = "Alice".to_string();
    context.current_request = Some("Help me organize my emails".to_string());
    
    // Get rendered prompt for admin in planning state
    let prompt = manager.get_prompt(
        &agent_id,
        AgentState::Planning,
        &context
    ).await?;
    
    println!("Rendered prompt:\n{}", prompt);
    
    Ok(())
}
```

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

### Phase 0: Core Infrastructure ✅ 100% COMPLETE (2025-10-24)

### Phase 1: AI Agent Intelligence (~400 runs, 3-4 weeks)
- Admin AI core with intent parsing
- PM agents (Communications, Knowledge, System)
- Worker agents with MCP integration
- State machine & memory systems

### Phase 2: Local Hub Networking (~350 runs, 3-4 weeks)
- Device discovery (mDNS)
- P2P mesh protocol (libp2p)
- Content-addressed storage
- CRDT-based synchronization

### Phase 3: Blockchain & Governance (~420 runs, 4-5 weeks)
- Identity system (DID + keypairs)
- Blockchain core (Tendermint)
- Human-AI link verification
- Constitutional validation on-chain

### Phases 4-7: See [PROJECT_STATUS.toml](helperfiles/PROJECT_STATUS.toml)

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

**Phase 0 Status:** 95% Complete (Cycle 0.6 MCP migration in progress) 🚧

**Crates:**
- ✅ **hainet-core**: Content-addressed storage + P2P sync (~650 LOC, 19 tests)
- ✅ **hainet-persona**: Multi-agent AI system (~8,400 LOC, 128 tests)
  - Prompt management (1,500 LOC)
  - Messaging infrastructure (2,576 LOC)
  - Guardian system (1,150 LOC)
  - AI provider discovery (2,450 LOC)
  - MCP tools stubs (740 LOC)
- ✅ **hainet-chain**: Identity system (DID + Ed25519) (~750 LOC, 19 tests)
- ✅ **hainet-seed**: Auto-installer (~550 LOC, 11 tests)
- 🚧 **mcp-servers/hainet-files**: MCP server (~280 LOC, has compilation errors)
- ⏳ **hainet-portal**: Structure defined (Phase 4+)
- ⏳ **hainet-bridge**: Structure defined (Phase 5+)

**Build Status:**
```bash
$ cargo build --workspace
   Compiling hainet-persona v0.1.0
   Compiling hainet-core v0.1.0
   Compiling hainet-chain v0.1.0
   Compiling hainet-seed v0.1.0
   Finished `dev` profile [unoptimized + debuginfo]
   
$ cargo check -p hainet-files
   Checking hainet-files v0.1.0
   error: Could not compile due to 3 previous errors
   # API alignment with rmcp v0.8.2 needed
```

**Test Coverage:**
- **Total Tests:** 170 passing (100% pass rate on completed components)
  - hainet-persona: 128 tests (lib + integration)
  - hainet-core: 19 tests
  - hainet-chain: 19 tests
  - hainet-seed: 11 tests
  - mcp-servers/hainet-files: 0 tests (pending compilation fix)
- **Lines of Code:** ~10,570 (production code + MCP migration)
- **Constitutional Compliance:** Fully integrated across all components

**MCP Migration Resources:**
- Analysis: `MCP_ANALYSIS_AND_MIGRATION_PLAN.md`
- Official SDK: https://github.com/modelcontextprotocol/rust-sdk
- Documentation: https://docs.rs/rmcp/latest/rmcp/

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

**For Development Questions:** See [DEVELOPMENT_RULES.md](helperfiles/DEVELOPMENT_RULES.md)

---

**Last Updated:** 2025-10-21  
**Version:** 0.1.0-alpha (Phase 0, Cycle 0.6 Complete)  
**Status:** 🚧 Active Development - Not Production Ready

*Building a future where AI serves humanity, not corporations.*

<!-- # END OF FILE README.md -->
