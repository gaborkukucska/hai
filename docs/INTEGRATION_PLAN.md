<!-- # START OF FILE docs/INTEGRATION_PLAN.md -->
# HAI-Net Grand Integration Plan v2

> **Created**: 2026-05-17
> **Status**: Approved — Ready for Phase 1 execution
> **Strategy**: Hybrid Progressive (Option C) — Python/JS sidecars first, progressive Rust port

Merging **TrippleEffect**, **PPLPWR**, **NoSlop**, **TropoMesh**, and **gChat** into a unified HAI-Net framework.

---

## 1. Architectural Decisions (Locked)

| Decision | Resolution | Rationale |
|---|---|---|
| Language Strategy | **Option C: Hybrid Progressive** | TrippleEffect Python sidecar first, progressive Rust port |
| TropoMesh Priority | **Last (Phase 6)** | Software must stabilize before hardware testing |
| gChat Independence | **Full absorption** — no standalone build | HAI-Net Portal is the only UI; multi-node deployment is HAI-Net Seed's job |
| Social Layer | **gChat wins entirely** — NoSlop social features retired | gChat's daisy-chain gossip, privacy firewall, and auto-sync are superior |
| Matrix Protocol | **Removed** — gChat's gossip mesh replaces Matrix | HAI-Net nodes gossip directly via libp2p+Tor; no Matrix dependency |
| Compute Coordination | **PPLPWR fully absorbed** — no standalone | Announcements travel via HAI-Net gossip, not Matrix rooms |

---

## 2. Codebase Summary

| Project | Language | LoC | Maturity | Key Capability |
|---|---|---|---|---|
| **HAI-Net** (this repo) | Rust | ~40K | Medium | Core infra, networking, agents, MCP, blockchain |
| **TrippleEffect** | Python | ~29K | High | Battle-tested agentic orchestration (state machines, failover, loop detection) |
| **PPLPWR** | Node.js/TS | ~50K | Medium | Compute sharing (GPU profiling, idle detection, network adapters) |
| **NoSlop** | Python + Next.js | ~24K | Medium | Media creation (ComfyUI, FFmpeg), mature multi-device installer |
| **TropoMesh** | Documentation | 81K README | Spec only | Hardware backbone specification |
| **gChat** | Node.js + React/TS | ~45K+ | High | Privacy-first social (Tor, gossip, E2E crypto, media relay) |

---

## 3. Post-Integration Crate Structure

```
hainet/
├── hainet-core/          # Networking (libp2p), storage (CAS+CRDT), multimodal, Tor transport
├── hainet-persona/       # Advanced agentic system (TrippleEffect patterns ported)
├── hainet-chain/         # Blockchain, identity, media provenance
├── hainet-collab/        # [NEW] Compute sharing (from PPLPWR)
├── hainet-social/        # [NEW] Decentralized social (from gChat)
├── hainet-seed/          # Enhanced installer (+ NoSlop Seed capabilities)
├── hainet-portal/        # Unified Tauri UI (all views merged)
├── hainet-bridge/        # External API gateway
├── hainet-vault/         # Constitution, governance
├── mcp-servers/          # Extended MCP tools (+ TrippleEffect tool ports)
│   ├── hainet-web/
│   ├── hainet-files/
│   ├── hainet-system/
│   ├── hainet-dev/
│   ├── hainet-mcp-server/
│   ├── hainet-media-mcp/    # [NEW] ComfyUI, FFmpeg, OpenCV
│   └── hainet-collab-mcp/   # [NEW] Compute network tools
├── services/             # [NEW] Sidecar services (temporary, Phase C hybrid)
│   └── agent-svc/        # TrippleEffect Python wrapper
└── tests/
```

---

## 4. Phased Roadmap

### Phase 1: Agentic Core (Weeks 1-6)

**Goal**: Get TrippleEffect's advanced agentic capabilities running as the brain of HAI-Net.

#### 1A. TrippleEffect → HAI-Net Agentic Service

| Task | Detail |
|---|---|
| Create `services/agent-svc/` | Python package wrapping TrippleEffect as a managed subprocess |
| Define gRPC/IPC contract | Protobuf schema for HAI-Net ↔ TrippleEffect communication |
| Create `hainet-persona/src/bridge/` | Rust gRPC client + sidecar lifecycle management |
| Port TE state machine patterns | Implement in `hainet-persona` Rust: state graphs, transition validation |
| Port TE cycle handler patterns | AgentCycleHandler logic → Rust async tasks |
| Port TE failover handler | Model failover chain → enhance `ai_providers` |
| Port TE context management | Bounded workspace trees, auto-summarization |
| Merge TE tool ecosystem | Map TE's 21 tools → HAI-Net MCP server equivalents |
| Port TE Constitutional Guardian | Merge with existing guardian module |
| Unify prompt systems | TE's YAML prompts + HAI-Net's Handlebars templates |

**Key mapping — TrippleEffect Python → HAI-Net Rust**:

| TrippleEffect (Python) | HAI-Net (Rust) | Strategy |
|---|---|---|
| `AgentManager` | `PersonaService` in lib.rs | Sidecar first, then port |
| `Agent.process_message()` async generator | `Agent::process_message()` trait | Events → tokio mpsc channel |
| `AgentCycleHandler.run_cycle()` | New `CycleEngine` module | Port cycle logic to async tasks |
| `WorkflowManager` state transitions | `AgentStateMachine` (exists!) | Merge TE's transition rules |
| `ToolExecutor` | MCP servers (exist!) | Map TE tools → MCP tool calls |
| `ConstitutionalGuardian` | `GuardianAgent` (exists!) | Merge governance.yaml enforcement |
| `FailoverHandler` | New `failover.rs` | Port model failover chain |
| `ContextSummarizer` | New `context.rs` | Port token-bounded summarization |
| `detect_autoregressive_loop()` | New `loop_detector.rs` | Port pattern detection |

#### 1B. Authentication & Sessions

| Task | Detail |
|---|---|
| Port TE JWT+bcrypt auth | Into hainet-portal Tauri commands |
| Session management | TE's SessionManager → Rust with serde state snapshots |
| User identity | Merge with hainet-chain identity system |

#### 1C. Database Unification

| Task | Detail |
|---|---|
| Define unified schema | Projects, tasks, agents, metrics, chat history, user settings |
| Migration layer | SQLite for standalone, PostgreSQL option for clusters |
| Port TE database_manager | Into hainet-persona/projects |

---

### Phase 2: Compute Sharing — PPLPWR Absorption (Weeks 4-8)

**Goal**: Integrate PPLPWR's compute contribution system as `hainet-collab`.

#### 2A. New `hainet-collab` Crate

| PPLPWR (TypeScript) | `hainet-collab` (Rust) | Dependency |
|---|---|---|
| `HardwareProfiler` | `hardware.rs` | `sysinfo` + `nvidia-ml-sys` |
| `IdleDetector` | `idle.rs` | `tokio` timer + OS idle APIs |
| `Scheduler` (weighted, thermal) | `scheduler.rs` | Pure Rust async |
| `PetalsAdapter` / `PrimeIntellectAdapter` | `adapters/` module | Trait-based, extensible |
| `UserPolicy` | `policy.rs` | `serde` + `toml` config |
| `ContributionRepository` | `contributions.rs` | `sqlx` |

#### 2B. Matrix → Gossip Migration

| Matrix Pattern | HAI-Net Gossip Replacement |
|---|---|
| `MatrixConnector.connect()` | hainet-social gossip mesh |
| `AnnouncementParser` | New `ComputeAnnouncement` packet type |
| Room-based broadcast | libp2p gossipsub topic: `/hainet/collab/announcements` |

#### 2C. Installer Enhancement

| Task | Detail |
|---|---|
| Merge NoSlop Seed capabilities | Port service discovery into hainet-seed |
| Port role assignment logic | NoSlop's hardware-based role assignment → Rust |
| Port SSH deployment | NoSlop's mature SSH manager → enhance hainet-seed |
| TropoMesh compatibility | Ensure hainet-seed runs on RPi 5, Jetson Orin Nano |

---

### Phase 3: Media Creation — NoSlop Selective Absorption (Weeks 6-10)

**Goal**: Absorb NoSlop's media production capabilities. Social features are **retired** (gChat's are superior).

#### What We Keep

| Component | Target |
|---|---|
| ComfyUI `WorkflowGenerator` | `mcp-servers/hainet-media-mcp/` |
| FFmpeg/OpenCV processing | `mcp-servers/hainet-media-mcp/` |
| `WorkerAgent` specializations | hainet-persona worker templates |
| `ResourceRequirements` / `WorkerCapabilities` | hainet-collab capabilities |
| Seed deployer (most mature) | Already merged into hainet-seed (Phase 2C) |
| Admin AI personality system | Config in hainet.toml |

#### What We Retire

| Component | Reason |
|---|---|
| NoSlop FastAPI backend | Replaced by hainet-bridge |
| NoSlop Next.js frontend | Replaced by hainet-portal |
| NoSlop social features | gChat's gossip is superior |
| NoSlop auth/database | HAI-Net has its own |
| NoSlop `admin_ai.py` | TrippleEffect's Admin AI is far more advanced |

---

### Phase 4: Privacy & Social Mesh — gChat Full Port (Weeks 8-12)

**Goal**: Integrate gChat's privacy-first social networking as `hainet-social`.

#### 4A. Gossip Protocol → Rust

| gChat (TypeScript) | `hainet-social` (Rust) |
|---|---|
| `NetworkPacketSchema` (32 packet types) | `packets.rs` — Rust enum (serde) |
| Daisy-chain broadcast (TTL hops) | `gossip.rs` — libp2p gossipsub + TTL |
| `_processedPacketIds` dedup | `dedup.rs` — time-windowed HashSet |
| `_trustedPeers` firewall | `firewall.rs` — whitelist filter |
| Link identity rewrapping | `privacy.rs` — strip originNode |
| `INVENTORY_SYNC_*` | `sync.rs` — post reconciliation |

#### 4B. E2E Encryption → Rust

| gChat | `hainet-social` | Crate |
|---|---|---|
| Ed25519 keypairs | `identity.rs` | `ed25519-dalek` |
| X25519+XSalsa20 (nacl.box) | `crypto.rs` | `x25519-dalek` + `chacha20poly1305` |
| Tripcode (SHA3→Base32) | `tripcode.rs` | `sha3` |
| AES-GCM backup | `backup.rs` | `aes-gcm` |

#### 4C. Media Transport → Rust

| gChat | `hainet-social` |
|---|---|
| `ActiveDownload` chunk manager | `downloads.rs` — async chunk assembly |
| AIMD congestion control | `congestion.rs` |
| Pure streaming proxy (`_relayState`) | `relay.rs` — zero-copy forwarding |
| Mesh recovery broadcast | `recovery.rs` |

#### 4D. Social Features → Rust

| Feature | Module |
|---|---|
| Posts (public/friends/private) | `feed.rs` |
| Votes, Comments, Reactions | `interactions.rs` |
| Groups (invite, sync, admin) | `groups.rs` |
| Handle.Tripcode identity | `identity.rs` |
| DMs + group chat | `messaging.rs` |

#### 4E. Tor as Optional Transport

| Component | Implementation |
|---|---|
| Tor daemon management | `hainet-core/src/networking/tor.rs` |
| Dual hidden services | Public mesh + Private admin |
| Config toggle | `hainet.toml` → `[networking.tor]` |

---

### Phase 5: Unified Portal (Weeks 10-14)

| Module | Source | Key Views |
|---|---|---|
| **Agent Workspace** | TrippleEffect | Chat, project boards, agent tree, task tracking |
| **Compute Dashboard** | PPLPWR | Hardware stats, contributions, scheduling |
| **Media Studio** | NoSlop | ComfyUI workflows, media library, rendering |
| **Social Hub** | gChat | Feed, DMs, groups, contacts, media sharing |
| **System** | HAI-Net | Node settings, mesh peers, guardian, Tor status |
| **Congress** | HAI-Net (new) | Governance proposals, voting |

---

### Phase 6: TropoMesh Hardware Compatibility (Weeks 12-16+)

| Task | Detail |
|---|---|
| ARM64 cross-compilation | `cargo build --target aarch64-unknown-linux-gnu` |
| RPi 5 / Jetson Orin Nano testing | HAI-Net on Phase Zero hardware |
| LoRa mesh integration | Meshtastic protocol via serial |
| Resource-constrained mode | Disable heavy features on low-spec nodes |

---

## 5. Dependency Graph

```
Phase 1A (Agentic Core) ──→ Phase 2A (Compute Sharing)
                        └──→ Phase 3A (Media Tools)
Phase 1B (Auth) ───────────→ Phase 4A (Social Crate)
Phase 1C (Database) ───────→ Phase 2A, Phase 3A
Phase 2A ──→ Phase 2B (Matrix→Gossip) ──→ Phase 2C (Installer)
Phase 3A ──→ Phase 3B (Media Social) ──→ Phase 3C (Retire NoSlop)
Phase 4A ──→ Phase 4B (Tor) + Phase 4C (Social Features) ──→ Phase 4D (Retire gChat)
Phase 3C + Phase 4D + Phase 2C ──→ Phase 5 (Portal)
Phase 5 ──→ Phase 6 (TropoMesh)
```

---

## 6. What NOT to Duplicate

| Capability | Keep From | Do NOT Duplicate |
|---|---|---|
| Agent orchestration | TrippleEffect → HAI-Net | NoSlop's simpler agent system |
| Multi-device deployment | NoSlop Seed → hainet-seed | gChat's deployer |
| AI provider management | HAI-Net + TE failover | PPLPWR's basic agent |
| P2P networking | HAI-Net libp2p | gChat's raw Tor HTTP |
| Cryptographic identity | gChat Ed25519 + HAI-Net Chain | NoSlop's basic blockchain |
| Constitutional oversight | TE Guardian + HAI-Net Guardian | (merge both) |
| Social features | gChat | NoSlop social |
| Project management | TrippleEffect | NoSlop's simpler PM |

---

## 7. Verification Plan

### Automated
- `cargo test --workspace` / `cargo clippy --workspace`
- Python sidecar tests via `pytest`
- Integration: Agent creates project → PM decomposes → Workers execute

### End-to-End Smoke Tests
1. `hainet-seed install` → single device bootstrap
2. User → Admin AI → project → PM → Workers → output
3. Idle detection → compute adapter → stops on activity
4. Post → gossip propagates → peer receives via daisy-chain
5. Media upload → chunked transfer → relay through intermediary
6. Tor enabled → onion address reachable → E2E encrypted DM

---

## 8. Risk Assessment

| Risk | Severity | Mitigation |
|---|---|---|
| Python sidecar deployment complexity | Medium | `hainet-seed install` handles it |
| gChat gossip port complexity (1,400 LoC) | High | Port subsystem-by-subsystem with tests |
| TrippleEffect 29K LoC bridge via gRPC | Medium | Start with critical paths, port progressively |
| Multiple async runtimes (tokio + asyncio) | Medium | gRPC bridge isolates cleanly |
| Tor integration reliability | Medium | Make optional; `arti` crate fallback |
