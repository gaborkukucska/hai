<!-- # START OF FILE docs/SUBPROJECT_LEARNINGS.md -->
# HAI-Net Sub-Project Learnings & Status

> **Purpose**: Consolidated reference of all learnings, known issues, and status from the five research projects being integrated into HAI-Net.
> **Last Updated**: 2026-05-17

---

## 1. TrippleEffect — Agentic Core R&D

**Version**: v2.50 | **Language**: Python (FastAPI) | **LoC**: ~29K | **Status**: Stable, battle-tested

### Architecture Learnings

- **Agent Hierarchy**: Admin AI → PM Agents → Worker Agents is the proven pattern. Each level has distinct state machines and tool access controls.
- **State Machine is Critical**: The `startup → planning → work → report → wait` lifecycle with strict transition validation prevents agents from getting stuck. Persistent state sets (`pm_manage`, `worker_work`) prevent aggressive resets.
- **Loop Detection Saves Resources**: `detect_autoregressive_loop()` catches LLMs repeating themselves (min 20 chars, 4+ repetitions) and terminates the stream early. Also has a hard 32K char output limit to prevent KV cache exhaustion.
- **Cross-Cycle Duplicate Detection**: If an agent makes the same tool call 4 consecutive times, force a state transition. This breaks infinite retry loops.
- **Failover Chain**: Same model on alternate API → alternative model → external provider (OpenRouter). This multi-level failover keeps agents running even when local models crash.
- **Context Management**: Bounded workspace trees and auto-summarization are essential for small context window models (8K-32K). Without these, agents lose track of what they're doing within 3-4 cycles.
- **Native JSON Tool Calling + XML Fallback**: Always try native tool calling first (Ollama's `tools` parameter). Fall back to XML parsing for models that don't support it. Both paths must be robust.
- **ChatML Template Injection**: Some Ollama models ship with broken chat templates. TrippleEffect now forces a standardized ChatML template on all models to prevent multi-turn conversation degradation.
- **PM Startup Retry**: If a PM agent outputs only `<think>` tags without a valid kickoff plan, inject framework feedback and retry the same state. This prevents PMs from "thinking forever."

### Known Issues (to carry forward)

- **Provider Configuration Gap**: System must detect absence of configured LLM providers and block startup until one is set. UI modal triggers post-login.
- **Tool Scope by State**: PM agents in `startup` state must be restricted to only `file_system` and `tool_information` tools. Giving them operational tools causes them to skip planning.
- **NoneType in Tool Info**: `get_detailed_usage()` can return None for some tools, causing infinite loops in PM startup. Always guard against None returns.

### Key Files to Reference

| File | Purpose |
|---|---|
| `src/agents/core.py` | Agent base class, streaming, tool parsing (718 lines) |
| `src/agents/cycle_handler.py` | The heart of autonomous operation |
| `src/agents/constants.py` | All state/status constants |
| `src/agents/agent_tool_parser.py` | XML tool call extraction |
| `src/llm_providers/failover_handler.py` | Model/provider failover chain |
| `src/tools/executor.py` | Tool execution with auth levels |
| `docs/FRAMEWORK.md` | Comprehensive framework documentation |
| `docs/TOOL_MAKING.md` | How to create new tools |

---

## 2. PPLPWR — Compute Sharing Router

**Version**: Beta | **Language**: Node.js/TypeScript | **LoC**: ~50K | **Status**: Core architecture complete

### Architecture Learnings

- **Hardware Profiling**: Use `nvidia-smi` for GPU stats (VRAM, CUDA version, driver). Use `systeminformation` for CPU/RAM. Cross-reference both for accurate profiles.
- **Idle Detection**: Client-side activity reporting via WebSocket is reliable for web apps. Desktop (Electron) needs native OS hooks for true idle detection (`desktop-idle` npm package).
- **Weighted Scheduling**: When sharing compute across multiple networks (Petals, Prime Intellect), weighted time-sharing based on user preferences works well. Each network gets proportional time.
- **Thermal Monitoring**: Pause compute tasks if GPU > 85°C. Check battery state — never run compute on battery power. These are essential for user trust.
- **Network Adapter Pattern**: Abstract each compute network (Petals, Prime Intellect, etc.) behind a common adapter interface. This makes adding new networks trivial. Key methods: `start()`, `stop()`, `getStatus()`, `isCompatible(hardwareProfile)`.
- **AI Agent for Decisions**: Using an LLM (Ollama/Gemini) to evaluate network announcements and decide whether to participate is effective. The agent considers hardware compatibility, user policy, and announcement quality.
- **Policy System**: Users need fine-grained control: autonomy level (ask/notify/silent), max concurrent tasks, never-allowed actions, approval requirements. Without this, users don't trust automated compute sharing.

### Known Issues (to carry forward)

- **Matrix Connector**: Implemented but needs testing with real Matrix accounts. Being replaced by HAI-Net gossip, so this is now deprecated.
- **Secure Credential Storage**: Tokens stored in `.env` — needs proper keychain/vault integration.
- **Multi-GPU Support**: Not yet implemented — schedule different networks across different GPUs.
- **Workload Sandboxing**: Compute tasks run as raw Python processes. Should use Docker/Firecracker for security.

### Key Files to Reference

| File | Purpose |
|---|---|
| `server.js` | Main server with all compute logic (~800 lines) |
| `server.ts` | TypeScript server with API endpoints |
| `src/server/core/HardwareProfiler.ts` | GPU/CPU detection |
| `src/server/core/Scheduler.ts` | Weighted network scheduling |
| `src/server/core/IdleDetector.ts` | Activity-based idle detection |
| `src/server/adapters/PetalsAdapter.ts` | Compute network adapter example |
| `docs/ARCHITECTURE.md` | System architecture |
| `docs/unfinished_features_report.md` | What's left to do |

---

## 3. NoSlop — Media Creation & Social

**Version**: 0.04 | **Language**: Python (FastAPI) + Next.js | **LoC**: ~24K + 52K seed | **Status**: In Development

### Architecture Learnings

- **Multi-Device Seed Deployer (Most Mature)**: NoSlop's seed is the gold standard for multi-device deployment. Key patterns:
  - Network scanning with SSH credential caching
  - Service discovery for existing Ollama/ComfyUI/PostgreSQL instances
  - Weighted capability scoring (RAM 40%, GPU 30%, CPU 20%, Disk 10%) for master election
  - Role assignment: Master (coordinator), Compute (GPU), Storage (disk), Client (UI-only)
  - SSH-based remote deployment with proper permission management
  - NFS shared storage for model files across nodes
  - Apt lock detection and retry for package installations
  - SSH command timeout enforcement (paramiko limitation workaround)
- **ComfyUI Integration**: Generate valid ComfyUI workflow JSON from natural language prompts using an LLM. Save workflows to shared storage for worker access.
- **Worker Specializations**: Different media tasks need different workers: ScriptWriter, PromptEngineer, StoryboardArtist, VideoEditor, ColorGrader, ResearchAgent. Each has different resource requirements.
- **Context Optimization for Local LLMs**: Strict 5-message history limit for chat context. "On-Demand" status injection (LLM only sees status if it detects intent). Concise status reports for token efficiency.
- **Proactive Admin AI**: Session priming with time-of-day context, active projects, and user preferences creates a much better UX than a passive "waiting for input" state.

### Known Issues (to carry forward)

- **NFS Mount Paths**: Workers must use `/mnt/noslop` (generic mount point), NOT the master's device-specific storage path.
- **SFTP Permission Fix**: Create directories with `sudo`, then immediately `chown` to the SSH user. SFTP cannot write to root-owned directories.
- **Frontend Node.js Version**: Next.js requires >= v20.9.0. Many Ubuntu systems ship with v18. Must install from NodeSource.
- **Deployment Excludes**: Always exclude `node_modules`, `.next`, `__pycache__`, `venv` from SSH file transfers. Transferring `node_modules` creates thousands of slow `test -d` SSH commands.

### Key Files to Reference

| File | Purpose |
|---|---|
| `seed/deployer.py` | Multi-device deployment orchestrator |
| `seed/ssh_manager.py` | SSH/SFTP with timeout management |
| `seed/role_assigner.py` | Hardware-based role assignment |
| `seed/storage_manager.py` | NFS/SMB shared storage |
| `seed/installers/` | Service installers (Ollama, ComfyUI, etc.) |
| `backend/admin_ai.py` | Admin AI with personality system |
| `backend/worker_agent.py` | Base worker with retry logic |
| `backend/workflow_generator.py` | ComfyUI workflow generation |
| `seed/MULTI_DEVICE_IMPLEMENTATION.md` | Multi-device deployment design |

---

## 4. gChat — Privacy-First Social Mesh

**Version**: 1.5.0 | **Language**: Node.js + React/TypeScript | **LoC**: ~45K+ | **Status**: Stable/Feature Rich

### Architecture Learnings

- **Daisy-Chain Gossip Protocol**: Posts propagate through trusted peers via TTL-limited hops (default 6). Each node re-stamps the senderId before forwarding, preserving origin privacy. This is superior to simple flooding.
- **Trusted Peer Firewall**: ALL incoming packets from untrusted peers are dropped EXCEPT connection requests. This is the foundation of the privacy model. Media from untrusted sources goes through trusted peer relay.
- **Adaptive Download Manager (AIMD)**: Congestion control for media downloads over Tor:
  - Additive Increase: +0.1 concurrency per successful chunk with RTT < 2s
  - Multiplicative Decrease: halve concurrency on timeout
  - Dynamic timeout: Base 60s + (avg_RTT × 4) to account for Tor circuit creation
  - Max 10 retries per chunk before triggering mesh recovery
  - 5-second penalty box (backoff) for failed chunks
- **Pure Streaming Proxy**: Relay nodes forward media chunks WITHOUT downloading the full file. `_relayState` tracks the source node and listeners. Privacy is preserved because relay nodes never see the origin.
- **Mesh Recovery Protocol**: When source goes offline:
  1. Broadcast `MEDIA_RELAY_REQUEST` to all trusted peers
  2. If a peer has the media, it responds with `MEDIA_RECOVERY_FOUND`
  3. Requester switches download source to the responding peer
  4. 45-second retry interval (Tor is slow; 10s was too aggressive causing ERR_CANCELED loops)
  5. 10-minute total timeout for recovery
- **Relay Dedup**: Response cache prevents "already told you" spam (5-minute window). History keys prevent re-forwarding the same request (10-second window).
- **Handle.Tripcode Identity**: `publicKey → SHA3-256 → Base32 → first 6 chars`. Deterministic, collision-resistant, human-readable. Example: `User.x7z9ab`.
- **Inventory Sync**: Nodes exchange post hash inventories to identify missing content, then request only what's needed. This keeps nodes in sync after disconnection periods.
- **Dual Onion Services**: Public onion for mesh routing, private authenticated onion for remote admin access. Critical for nodes that need maintenance without physical access.
- **Ephemeral Relay**: After completing a relay download, enter `completed_serving` state with 2-minute grace period for downstream peers to request chunks from RAM. Clean up after ACK or timeout.
- **Packet Schema Validation**: All 32 packet types validated with Zod discriminated union. Invalid packets are dropped silently. This prevents protocol-level attacks.

### Known Issues (to carry forward)

- **LocalStorage → IndexedDB**: All data stored in localStorage (5MB limit). Migration to IndexedDB is critical for scale.
- **Ephemeral Messages Don't Delete**: `isEphemeral: true` flag exists but messages aren't actually deleted from storage. Need garbage collector.
- **Feed Performance**: Filtering/sorting entire post array on every render. Need virtualization for 100+ posts.
- **Gossip Flooding**: Simple 6-hop flood. Should move to probabilistic gossip at scale.
- **Multi-Node Deploy**: The automated LAN deployer (SSH-based) is being replaced by HAI-Net Seed.

### Key Files to Reference

| File | Purpose |
|---|---|
| `services/networkService.ts` | The entire gossip protocol (1,407 lines) |
| `services/packetSchema.ts` | All 32 packet types with Zod validation |
| `services/cryptoService.ts` | Ed25519 signing, X25519 encryption |
| `services/kv.ts` | Key-value storage abstraction |
| `services/mediaStorage.ts` | IndexedDB/localStorage media persistence |
| `server.js` | Node.js backend with Tor management (~45K lines) |
| `docs/ARCHITECTURE.md` | Dual-onion architecture, gossip protocol |
| `docs/SECURITY.md` | Cryptographic primitives and threat model |
| `docs/IDENTITY_SYSTEM.md` | Handle.Tripcode system |
| `docs/PROJECT_STATUS.md` | Completed features and known issues |

---

## 5. TropoMesh — Hardware Backbone Specification

**Version**: Spec only | **Language**: N/A (documentation) | **Status**: Design phase

### Key Specifications

- **Phase Zero Ground Nodes**: $440-$7,200 hardware. RPi 5 (8GB) as base compute. Jetson Orin Nano for ML workloads.
- **Connectivity**: WiFi 7 (6 GHz), LoRa (long-range low-bandwidth), existing internet (Phase Zero).
- **Power**: Solar panels + battery for off-grid operation.
- **Future**: Tropospheric airships with laser inter-links for backbone.
- **Software**: The Phase Zero ground node software IS HAI-Net itself. All TropoMesh software needs are covered by this integration plan.

### Integration Implications

- HAI-Net must cross-compile for ARM64 (`aarch64-unknown-linux-gnu`)
- Must support resource-constrained mode (disable heavy features on RPi)
- LoRa integration via Meshtastic protocol (serial interface)
- Power management awareness (battery/solar monitoring)

### Key Files to Reference

| File | Purpose |
|---|---|
| `README.md` | Complete 81K specification document |

---

## 6. Cross-Project Patterns to Preserve

### Common Development Rules (from all projects)

1. Write file location in first line: `<!-- # START OF FILE path/file.ext -->`
2. Never remove functional code — complete what's missing
3. Make ALL hardcoded values configurable via environment variables
4. Use consistent logging format throughout
5. Maintain comprehensive DEBUG and INFO logging for observability
6. Follow phased implementation tracked in status files
7. Update documentation at the end of every development session

### Common Anti-Patterns Discovered

1. **Don't canonicalize non-existent paths** — Use structural checks instead (HAI-Net Session 54-55)
2. **Don't transfer `node_modules` over SSH** — Exclude build artifacts from deployment (NoSlop Session 3)
3. **Don't use 10s retry intervals over Tor** — 45s minimum to avoid ERR_CANCELED avalanche (gChat v1.4.0)
4. **Don't give PM agents operational tools during startup** — They skip planning (TrippleEffect v2.50)
5. **Don't trust LLM output length** — Hard-cap at 32K chars to prevent KV cache exhaustion (TrippleEffect)
6. **Don't use Matrix for decentralized coordination** — gChat's gossip is more privacy-preserving and eliminates the Matrix homeserver dependency (PPLPWR → HAI-Net decision)

### Shared Capability Scoring Formula

Used by both HAI-Net and NoSlop for master election and role assignment:
```
score = (RAM_GB × 10 × 0.4) + (has_GPU ? 100 : 0 × 0.3) + (CPU_cores × 5 × 0.2) + (Disk_GB × 0.1)
```

### Model Selection Sweet Spots (from HAI-Net Session 42)

| Preference | Ideal Size | Hard Cap |
|---|---|---|
| Small | 2.5-4.5 GB | 6 GB |
| Medium | 4-6 GB | 8 GB |
| Large | 6-8 GB | 8 GB |
| Any | 3-5 GB | 8 GB |

> Models > 8GB cause timeouts. Models < 2.5GB lack capability. The 3-5GB range is the sweet spot.
