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


### Recent Achievements (2025-10-23)

✅ **Phase 2.1 COMPLETE** - Core Portal Foundation (Tauri + React)  
✅ **Portal Backend**: AdminBridge with 4 IPC commands (send_message, get_history, clear_history, get_agent_state)  
✅ **Portal Frontend**: ChatInterface with file attachments, message history, auto-scroll  
✅ **Ubuntu 24.04 Compatibility**: Resolved webkit2gtk-4.0 → 4.1 compatibility issues  
✅ **Phase 1 COMPLETE**: Project-based agentic system with Admin AI, PM, and Worker agents  
✅ **MCP Integration**: hainet-files server fully operational (10/10 tests passing)

### Previous Achievements

✅ **Phase 0 COMPLETE** (2025-10-21) - All infrastructure cycles 0.1-0.6  
✅ **Cycle 1.3 COMPLETE** (2025-10-22) - Admin AI Planning & PM Creation  
✅ **Cycle 1.2 COMPLETE** (2025-10-22) - Enhanced Agent State Machines  
✅ **Cycle 1.1 COMPLETE** (2025-10-22) - Project Management Infrastructure

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


# 4. Blockchain-Secured Human-AI Link

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