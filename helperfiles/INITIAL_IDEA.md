# HAI-Net Overview

**Philosophy**: Local resilience meets global capability. HAI-Net represents a fundamental reimagining of human-AI collaboration through a decentralized, privacy-first framework. The end goal is to create a dynamic framework that manages local resources to assist the requirements of local users and if allowed by the users share idle processing and storage capabilities with other validated members of the overall network.

## System Overview

**HAI-Net** (Hybrid AI Network) is a three-tier mesh network with multi-agent AI intelligence that prioritizes local resources but seamlessly expands capabilities using external sources when available and authorized. All secured by validated blockchain nodes.

## Core Principles

- 🤖 **AI-First Interface** - Natural conversation replaces traditional UI
- 🔒 **Privacy-First** - All personal user data remains under complete local control
- ⚡ **Local-First** - Full functionality offline, wider network connectivity as enhancement
- **Resource Sharing** - Voluntary exchange of computational resources with strict privacy
- 🌐 **Constitutional Framework** - Core principles enforced through code
- 🧠 **Learns & Adapts** - Personal AI that helps, encourages, informs, and grows with you
- 🔗 **Blockchain-Secured** - Cryptographic human-AI binding
- 💯 **Free Forever** - Core network services are always free for everyone
- ⚖️ **Values-Based** - Constitutional framework protecting fundamental rights
- **Community Focus** - Strengthening real-world connections and collaboration
- **Open Source** - All components are free, open source and transparent

---

# HAI-Net Components

## **HAI-Net Seed** (Installer)
AI-driven bootstrap application that sets up your node and persona.

## **HAI-Net Vault** (Blockchain)
The blockchain layer serving as validator for your Local Hub. Uses 51% consensus validation across nodes. **One vote per validated human member** (combining all their linked sub-nodes).

## **HAI-Net Core** (Runtime)
Main daemon running on each device, coordinating all services.

## **HAI-Net Persona** (AI Agent) ⭐
Your personalized AI that grows with you - **this is your interface**.

## **HAI-Net Portal** (UI)
Chat interface for conversation with your AI.

## **HAI-Net Bridge** (Gateway)
Secure connection to external internet.

---

# Resource Priority Cascade

```
┌─────────────────────────────────────┐
│        RESOURCE REQUEST             │
└──────────────┬──────────────────────┘
               │
    ┌──────────▼──────────┐
    │  1. LOCAL HUB       │
    │  - Your devices     │
    │  - Always offline   │
    └──────────┬──────────┘
               │ Not sufficient?
    ┌──────────▼──────────┐
    │  2. HAI-NET MESH    │
    │  - Regional hubs    │
    │  - Global mesh      │
    └──────────┬──────────┘
               │ Not available or authorized?
    ┌──────────▼──────────┐
    │  3. EXTERNAL        │
    │  - Traditional web  │
    │  - Cloud APIs       │
    └─────────────────────┘
```

---

# AI Intelligence Layer

```
┌─────────────────────────────────────┐
│          HUMAN USER                 │
│              ↕                      │
│    Natural Language Only            │
└───────────────┬─────────────────────┘
                │
┌───────────────▼─────────────────────┐
│           ADMIN AI                  │
│  • Primary interface                │
│  • Understands intent               │
│  • Orchestrates other agents        │
│  • Blockchain-secured human link    │
└───────────────┬─────────────────────┘
                │
    ┌───────────┼───────────┐
    │           │           │
┌───▼────┐  ┌──▼────┐  ┌──▼─────┐
│PM:Comms│  │PM:Know│  │PM:System│
│Email,  │  │Learn, │  │Hub ops, │
│Chat    │  │Memory │  │Resources│
└───┬────┘  └───┬───┘  └───┬─────┘
    │           │           │
 Workers     Workers     Workers
(Email,     (Search,    (Files,
 Chat,       Tutor,      Network,
 Social)     Research)   Compute)
```

**Implementation Status:**
- ✅ Type system defined (AgentType, AgentState, MessageContent)
- ✅ Hierarchical communication infrastructure complete
- 🚧 Agent intelligence (Cycles 1+)

---

# Technology Stack

## Foundation

**Core Languages:**
- **Rust** - Networking, storage, consensus, AI runtime (99% of codebase)
- **TypeScript** - UI layer (Tauri/React)  
- **WebAssembly** - Compute sandboxing
- **TOML** - Prompt templates and configuration

**Key Dependencies:**
```toml
# Networking
libp2p = "0.53"              # Mesh networking
tokio = { version = "1", features = ["full"] }

# Storage
sled = "0.34"                # Embedded KV store
sqlx = "0.7"                 # SQL

# Consensus
tendermint = "0.34"          # Blockchain consensus

# AI/ML
llama-cpp-rs = "0.1"         # Local LLM inference
candle = "0.3"               # ML framework

# Crypto
ed25519-dalek = "2.0"        # Signatures
chacha20poly1305 = "0.10"    # Encryption

# Utilities
serde = { version = "1.0", features = ["derive"] }
handlebars = "4.5"           # Template rendering
```

---

# Project Structure

```
hainet/
├── hainet-core/          # Main daemon
│   ├── src/
│   │   ├── network/      # Local mesh, HAI-Net mesh, external bridge
│   │   ├── storage/      # Content store, database, external cache
│   │   ├── compute/      # Local, federated, external
│   │   ├── identity/     # DID, web of trust, external OAuth
│   │   └── bridge/       # Gateway, whitelist, tunnel
│
├── hainet-persona/       # ⭐ AI AGENT SYSTEM (ACTIVE DEVELOPMENT)
│   ├── Cargo.toml
│   ├── prompts/          # ✅ TOML templates (Cycle 0.2)
│   │   ├── agents/       # Admin, PM, Worker prompts
│   │   ├── states/       # Startup, Idle, Planning, Working
│   │   ├── personalities/
│   │   └── system/       # Core instructions, safety
│   ├── src/
│   │   ├── prompts/      # ✅ Template loader/renderer (Cycle 0.2)
│   │   ├── messaging/    # ✅ Communication infrastructure (Cycle 0.3)
│   │   ├── agents/       # 🚧 TODO: Admin, PM, Worker implementations
│   │   ├── state/        # 🚧 TODO: State machine
│   │   ├── tools/        # 🚧 TODO: MCP integration
│   │   ├── blockchain/   # 🚧 TODO: Human-AI link
│   │   ├── memory/       # 🚧 TODO: Short/long-term memory
│   │   ├── models/       # 🚧 TODO: Model loading/switching
│   │   └── training/     # 🚧 TODO: Synthetic data, alignment
│   └── mcp-servers/      # 🚧 TODO: MCP tool servers
│       ├── hainet-files/
│       ├── hainet-network/
│       ├── hainet-compute/
│       ├── hainet-chain/
│       └── hainet-system/
│
├── hainet-chain/         # 🚧 TODO: Blockchain
│   ├── src/
│   │   ├── consensus/
│   │   ├── state/
│   │   ├── transactions/
│   │   └── sync/
│
├── hainet-seed/          # 🚧 TODO: Installer
│   ├── src/
│   │   ├── installer/
│   │   ├── setup/
│   │   ├── onboarding/
│   │   └── bootstrap/
│
├── hainet-portal/        # 🚧 TODO: UI
│   ├── src/
│   │   ├── ui/           # Chat, settings, network, files, stats
│   │   ├── components/
│   │   └── state/
│
└── hainet-bridge/        # 🚧 TODO: External gateway
    ├── src/
    │   ├── gateway/
    │   ├── services/
    │   ├── privacy/
    │   └── monitoring/
```

**Implementation Status:**
- ✅ Cycle 0.1: Project scaffolding
- ✅ Cycle 0.2: Prompt management system (~1,700 LOC)
- ✅ Cycle 0.3: Hierarchical agent communication (~2,576 LOC, 51 tests)
- 🚧 Cycle 0.4+: See PROJECT_PLAN.md for roadmap

---

# 4. MCP (Model Context Protocol) Integration

**Status:** 🚧 TODO (Cycle 0.4)

```rust
// hainet-persona/src/tools/mcp/client.rs

/// MCP Client for communicating with tool servers
pub struct MCPClient {
    servers: HashMap<String, MCPServer>,
}

impl MCPClient {
    pub async fn new() -> Result<Self> {
        let mut client = Self {
            servers: HashMap::new(),
        };

        // Start all MCP servers
        client.start_server("files", "mcp-servers/hainet-files").await?;
        client.start_server("network", "mcp-servers/hainet-network").await?;
        client.start_server("compute", "mcp-servers/hainet-compute").await?;
        client.start_server("chain", "mcp-servers/hainet-chain").await?;
        client.start_server("system", "mcp-servers/hainet-system").await?;

        Ok(client)
    }

    /// Call a tool on a specific server
    pub async fn call_tool(
        &mut self,
        server_name: &str,
        tool_name: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let server = self.servers.get_mut(server_name)
            .ok_or_else(|| anyhow::anyhow!("Server not found: {}", server_name))?;

        let request = MCPRequest {
            jsonrpc: "2.0".to_string(),
            id: rand::random(),
            method: tool_name.to_string(),
            params,
        };

        let response = self.send_request(&mut server.process, request).await?;

        if let Some(error) = response.error {
            return Err(anyhow::anyhow!("MCP error: {}", error.message));
        }

        Ok(response.result.unwrap_or_default())
    }
}
```

**Example MCP Server: hainet-files**

```rust
// mcp-servers/hainet-files/src/main.rs

fn handle_request(request: MCPRequest) -> MCPResponse {
    let result = match request.method.as_str() {
        "initialize" => handle_initialize(),
        "hainet_file_read" => handle_file_read(&request.params),
        "hainet_file_write" => handle_file_write(&request.params),
        "hainet_file_list" => handle_file_list(&request.params),
        "hainet_file_search" => handle_file_search(&request.params),
        "hainet_file_delete" => handle_file_delete(&request.params),
        _ => Err(format!("Unknown method: {}", request.method)),
    };

    match result {
        Ok(value) => MCPResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: Some(value),
            error: None,
        },
        Err(message) => MCPResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: None,
            error: Some(MCPError { code: -1, message }),
        },
    }
}

fn handle_initialize() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!([
        {
            "name": "hainet_file_read",
            "description": "Read a file from the local file system",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" }
                },
                "required": ["path"]
            }
        },
        {
            "name": "hainet_file_write",
            "description": "Write data to a file",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "content": { "type": "string" }
                },
                "required": ["path", "content"]
            }
        }
        // ... more tools
    ]))
}
```

---

# 5. Blockchain-Secured Human-AI Link

**Status:** 🚧 TODO (Cycle 3.3)

```rust
// hainet-persona/src/blockchain/link.rs

use ed25519_dalek::{Keypair, Signature};

/// Blockchain-secured link between human and their AI persona
pub struct PersonaLink {
    // Human identity
    user_did: DID,
    user_keypair: Keypair,

    // AI persona identity
    persona_did: DID,
    persona_keypair: Keypair,

    // Link record on blockchain
    link_record: LinkRecord,

    // Blockchain connection
    chain_client: Arc<ChainClient>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkRecord {
    pub user_did: DID,
    pub persona_did: DID,
    pub created_at: std::time::SystemTime,
    pub link_hash: Hash,

    // Cryptographic binding
    pub user_signature: Signature,
    pub persona_signature: Signature,

    // Metadata
    pub persona_name: String,
    pub version: Version,

    // State hash (for continuity verification)
    pub current_state_hash: Hash,
}

impl PersonaLink {
    /// Create new human-AI link and record on blockchain
    pub async fn create(
        user_keypair: Keypair,
        persona_name: String,
        chain_client: Arc<ChainClient>,
    ) -> Result<Self> {
        // Generate DID for user
        let user_did = DID::from_public_key(&user_keypair.public);

        // Generate new keypair for AI persona
        let persona_keypair = Keypair::generate(&mut rand::thread_rng());
        let persona_did = DID::from_public_key(&persona_keypair.public);

        // Create link record
        let link_data = format!("{}:{}:{}", user_did, persona_did, persona_name);
        let link_hash = sha3::Sha3_256::digest(link_data.as_bytes());

        // Both parties sign the link
        let user_signature = user_keypair.sign(link_hash.as_slice());
        let persona_signature = persona_keypair.sign(link_hash.as_slice());

        let link_record = LinkRecord {
            user_did: user_did.clone(),
            persona_did: persona_did.clone(),
            created_at: std::time::SystemTime::now(),
            link_hash: Hash::from_bytes(&link_hash),
            user_signature,
            persona_signature,
            persona_name: persona_name.clone(),
            version: Version::new(1, 0, 0),
            current_state_hash: Hash::zero(),
        };

        // Submit to blockchain
        chain_client.submit_persona_link(link_record.clone()).await?;

        Ok(Self {
            user_did,
            user_keypair,
            persona_did,
            persona_keypair,
            link_record,
            chain_client,
        })
    }

    /// Verify link integrity
    pub async fn verify(&self) -> Result<bool> {
        let chain_record = self.chain_client
            .get_persona_link(&self.persona_did)
            .await?;

        // Verify signatures
        let link_data = format!(
            "{}:{}:{}",
            self.user_did, self.persona_did, self.link_record.persona_name
        );
        let expected_hash = sha3::Sha3_256::digest(link_data.as_bytes());

        let user_valid = self.user_keypair.public
            .verify(&expected_hash, &chain_record.user_signature)
            .is_ok();

        let persona_valid = self.persona_keypair.public
            .verify(&expected_hash, &chain_record.persona_signature)
            .is_ok();

        Ok(user_valid && persona_valid)
    }
}
```

---

# 6. HAI-Net Vault (Blockchain State)

**Status:** 🚧 TODO (Cycle 3)

### **Blockchain State**

```rust
// What's stored on HAI-Net Vault

pub struct ChainState {
    // Immutable constitution
    constitution: Constitution,

    // Member registry
    members: MemberRegistry,
    validated_members: Vec<ValidatedMember>,
    pending_applications: Vec<MemberApplication>,

    // Governance
    active_proposals: Vec<Proposal>,
    proposal_history: Vec<CompletedProposal>,
    council_composition: CouncilMembers,

    // Software releases
    approved_releases: Vec<SoftwareRelease>,

    // Validator set
    validators: ValidatorSet,

    // Aggregated resource credits (settlement layer)
    credit_balances: HashMap<DID, Credits>,

    // Compute sharing registry
    shared_compute_resources: HashMap<DID, ComputeOffer>,
}

#[derive(Debug, Clone)]
pub struct Constitution {
    // Can NEVER be changed (hard-coded in genesis)
    core_principles: CorePrinciples,

    // Can be changed via supermajority (80% vote)
    governance_rules: GovernanceRules,

    // Can be changed via majority (67% vote)
    operational_policies: OperationalPolicies,
}

pub struct CorePrinciples {
    // Hard-coded forever
    pub privacy_first: bool,          // = true
    pub censorship_resistant: bool,   // = true
    pub user_owned_data: bool,        // = true
    pub open_source_core: bool,       // = true
    pub decentralized: bool,          // = true
    pub free_forever: bool,           // = true
    pub values_based: bool,           // = true
}
```

---

# Governance & Membership

## Free Forever Model

**HAI-Net is completely FREE to use for anyone, forever.** This includes:
- ✅ Network connectivity
- ✅ Stack updates
- ✅ Information sharing
- ✅ Basic AI models
- ✅ P2P compute sharing
- ✅ Blockchain validation

Only optional external services (like cloud APIs) may incur costs, which you control via policy.

## Validated Membership System

### Startup Phase: Invitation-Only (Until 5,000 Members)

**⚠️ During startup phase, validated membership is strictly invitation-only** to ensure network security and diversity.

**Invitation Rules:**
- Every new validated member receives **1 invitation**
- Invited person **must be of a different gender** than the inviter
- If possible, invited person should be from a **different ethnic group**
- Goal: **Maximize diversity of opinion** across the network

**Why?** Diversity of thought, background, and perspective makes the network more resilient, creative, and fair.

### Self-Nomination Option

**Anyone can self-nominate** to ensure complete visibility and growth. Self-nominations go through the **same transparent filtering process** as invitations.

🤗 **Special Fast-Track:** Members willing to share idle compute resources receive **automatic invitation priority**!

### One Member, One Vote

**One validated human = one vote** in governance decisions. All your linked sub-nodes are combined into a single vote to prevent Sybil attacks.

### The 5,000 Member Milestone

**During startup phase (until 5,000 validated members):**
- ⚠️ HAI-Net Vault changes are **locked for security**
- ⚠️ Membership is **invitation-only** or self-nomination
- ⚠️ Core constitution cannot be amended

**After reaching 5,000 members:**
- 🔓 Vault amendments become possible via supermajority vote
- 🔓 Membership processes may evolve
- 🔓 Full decentralized governance activates
- 🌐 Network is considered "production ready"

---

# Development Roadmap

**See PROJECT_PLAN.md for detailed development plan.**

## Phase 0: Core Infrastructure (Target: 2025-10-27)

**Completed:**
- ✅ Cycle 0.1: Project Scaffolding
- ✅ Cycle 0.2: Advanced Prompt Management System (~1,700 LOC)
- ✅ Cycle 0.3: Hierarchical Agent Communication (~2,576 LOC, 51 tests)

**In Progress:**
- 🚧 Cycle 0.4: Constitutional Guardian System (full PII/bias/harm detection)

**Pending:**
- Cycle 0.5: Core Component Integration (multi-device mesh)
- Cycle 0.6: MCP Tool Ecosystem (external service integration)

## Future Phases (Planned)

**Phase 1:** AI Agent Intelligence (~400 runs, 3-4 weeks)  
**Phase 2:** Local Hub Networking (~350 runs, 3-4 weeks)  
**Phase 3:** Blockchain & Governance (~420 runs, 4-5 weeks)  
**Phase 4+:** External Bridge, UI, Installation, Advanced Features, Production

**Total Estimated: ~2,670 generation runs (~6-12 months with LLM assistance)**

---

# System Requirements

**Minimum (Tier 1)**
- 1GB+ RAM, Tiny LLM (1B params), Local-only
- Examples: Smartwatch, old phone

**Recommended (Tier 2-3)**
- 4GB+ RAM, Small-medium LLM (3-7B params), Full mesh
- Examples: Modern phone, laptop

**Optimal (Tier 4)**
- 16GB+ RAM, Large LLM (13B+ params), Hub coordinator
- Examples: Desktop, NUC, home server

---

**The future of personal computing is AI-first, local-first, user-owned, and values-aligned.**

For detailed implementation plan, see PROJECT_PLAN.md.  
For development rules and guidelines, see DEVELOPMENT_RULES.md.
