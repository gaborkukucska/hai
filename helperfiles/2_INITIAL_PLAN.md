# HAI-Net Overview

**Philosophy**: Local resilience meets global capability. HAI-Net represents a fundamental reimagining of human-AI collaboration through a decentralized, privacy-first framework. It creates an unbreakable bond between local AI entities and their human counterparts while ensuring privacy, security, and shared prosperity through innovative resource sharing and community building.

## System Overview

**HAI-Net** (Hybrid AI Network) is a three-tier mesh  (local-regional-global) lead by individuals and their linked AI. A dynamic multi-agent framework that manages local resources (Local Hub) to assist the requirements of local user(s) and if allowed, share idle processing and storage capabilities, information and help communication and collaboration with other members of the overall network. All secured by validated blockchain nodes.

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

## **HAI-Net Seed** (Smart Installer)
AI-driven bootstrap application that intelligently sets up your node and persona with **smart multi-device mesh deployment**.

### Smart Installer Architecture

The HAI-Net Seed installer is designed to be **intelligent and adaptive**, distributing components based on device capabilities:

**Key Features:**
- 🔍 **Network Discovery**: Automatically scans local network for SSH-enabled devices (nmap integration)
- 🧠 **Capability Assessment**: Evaluates each device's hardware (CPU, RAM, GPU, disk, OS, architecture)
- 🎯 **Master Election**: Assigns roles based on weighted capability scoring (RAM 40%, GPU 30%, CPU 20%, Disk 10%)
- 🔐 **SSH Key Management**: Generates Ed25519 keys for passwordless authentication
- 📦 **Optimized Deployment**: Installs only necessary components per device role

**Installation Workflow:**
1. User runs `hainet-seed` installer on first device
2. Installer prompts: "Set up multi-device mesh? (Y/n)"
3. Network scan discovers all SSH devices on LAN (port 22)
4. User provides SSH credentials (once for all devices)
5. Installer connects to each device, assesses hardware capabilities
6. Capability scores calculated using weighted algorithm
7. Highest scoring device assigned **Master** role, others become **Slaves**
8. SSH keys generated, role-specific service plans displayed
9. User confirms deployment
10. Installer deploys components to each device based on role
11. Master initializes mesh coordination
12. Slaves connect to master, receive specialized assignments

**Device Roles:**

**Master Node** (Highest capability score):
- HAI-Net Core (Master mode) - Mesh coordination
- HAI-Net Chain (Blockchain) - Consensus & governance
- HAI-Net Bridge (Gateway) - External connectivity
- HAI-Net Portal (UI) - Primary user interface
- Primary storage and processing

**Slave Nodes** (Computing devices):
- HAI-Net Core (Slave mode) - Compute tasks
- HAI-Net Chain (Validator) - Blockchain validation
- Specialized roles assigned by Master:
  - **LLM Host** - Best GPU device runs AI models
  - **STT/TTS Host** - Good CPU/audio for speech processing
  - **Storage Node** - Available disk space for distributed storage
  - **Compute Worker** - Additional processing capacity

**Standalone** (Single device only):
- HAI-Net Core (Standalone mode) - Full local stack
- HAI-Net Portal (UI) - User interface
- All services run on one machine

**Mobile Devices** (Low RAM/CPU - e.g., smartphones):
- HAI-Net Portal (UI only) - Remote access point
- Minimal compute footprint
- Connects to hub for all processing
- Serves as mobile gateway to your mesh

**Capability Scoring Algorithm:**
```
Score = (RAM_GB × 10 × 0.4) + (GPU_Present × 100 × 0.3) + (CPU_Cores × 5 × 0.2) + (Disk_GB × 0.1)

Examples:
- Desktop PC (16GB RAM, RTX3060, 8 cores, 500GB): Score 152
- Laptop (8GB RAM, no GPU, 4 cores, 250GB): Score 61
- Smartphone (4GB RAM, no GPU, 8 cores, 128GB): Score 28.8
```

**Smart Distribution Strategy:**
- Master coordinates load balancing across mesh
- Specialized role assignments based on hardware strengths
- Mobile devices run UI-only (connect to hub processing power)
- Install only necessary components per device (minimal footprint)
- Constitutional transparency (user confirms every deployment step)

**Implementation Status:** ✅ Complete (Phase 4.5a)
- Network scanner: 300 LOC, 6 tests passing
- SSH client & assessment: 250 LOC, 4 tests passing
- SSH key management: 200 LOC, 2 tests passing
- Deployment orchestrator: 350 LOC, 5 tests passing
- **Total: 1,100 LOC, 17 tests passing (100% success rate)**

## **HAI-Net Vault** (Blockchain)
The blockchain layer serving as validator for your Local Hub. Uses 51% consensus validation across nodes. **One vote per validated human member** (combining all their linked sub-nodes).

## **HAI-Net Persona** (AI Agent) ⭐
Your personalized AI that grows with you - **your linked Artifical Entity**.

## **HAI-Net Portal** (UI)
Audio visual chat interface to interact with your AI.

## **HAI-Net Core** (Runtime)
Main daemon running on each device, coordinating all services and hosting local infrastructure.

### Local Services Infrastructure

Each HAI-Net local hub hosts its own suite of **privacy-first, open-source services** that operate fully offline and can optionally interconnect with other local hubs via the regional/global mesh network:

**Core Services:**

🔍 **Search Engine** (SearXNG or YaCy)
- Private, meta-search aggregator
- No tracking, no data collection
- Customizable search sources
- **Mesh Enhancement**: Distributed search across connected hubs (shared index for community knowledge)

📧 **Email Server** (Maddy or Stalwart Mail)
- Full SMTP/IMAP/POP3 support
- Spam filtering and encryption
- Local-first with optional federation
- **Mesh Enhancement**: Direct peer-to-peer email between hubs (bypassing traditional email infrastructure)

🌐 **Web Server** (Nginx)
- Host personal websites and services
- Reverse proxy for hub services
- SSL/TLS termination
- **Mesh Enhancement**: Serve content to other hubs (private CDN for community resources)

📚 **Offline Library** (Kiwix)
- Wikipedia, Stack Overflow, medical resources, textbooks
- ZIM file format (compressed Wikipedia dumps)
- Searchable offline knowledge base
- **Mesh Enhancement**: Shared library across hubs (distributed knowledge network)

**Service Deployment Strategy:**

**Standalone/Master Nodes:**
- All services installed by default
- Full local infrastructure stack
- Self-sufficient operation

**Slave Nodes:**
- Lightweight service subset based on available resources
- May host specific services (e.g., dedicated search index, library mirror)
- Can offload heavy services to Master

**Mobile Devices:**
- Client-only access to hub services
- No server hosting (minimal resource footprint)
- Connect to Master/Slave services via mesh

**Mesh Network Benefits:**

When multiple local hubs discover each other (via HAI-Net Bridge):

1. **Distributed Search**: Query indexes across all connected hubs for richer results
2. **Federated Email**: Direct hub-to-hub email (privacy-preserving, no external servers)
3. **Content Distribution**: Share web content across mesh (private CDN)
4. **Knowledge Sharing**: Merged Kiwix libraries (expanded offline knowledge base)
5. **Load Balancing**: Distribute service requests across multiple hubs
6. **Redundancy**: Failover to peer hubs if local services are down

**Service Coordination:**

The HAI-Net Core daemon manages service lifecycle:
- Auto-discovery of available services on local hub
- Registration of services with mesh network (if enabled)
- Health monitoring and automatic restarts
- Resource allocation based on device capabilities
- Privacy-preserving service advertisement to trusted peers

**Implementation Status:** 🔮 Planned (Phase 5+)
- Service installer modules (search, email, nginx, kiwix)
- Mesh service discovery protocol
- Inter-hub service coordination
- Privacy-preserving service federation


## **HAI-Net Bridge** (Gateway)
Secure connection to external HAI-Net nodes or the internet.

---

# Resource Priority Cascade

The HAI-Net resource cascade ensures **privacy-first** operation by prioritizing local resources, then trusted mesh peers, and finally external services:

```
┌─────────────────────────────────────┐
│        RESOURCE REQUEST             │
│   (Search, Email, Web, Knowledge)   │
└──────────────┬──────────────────────┘
               │
    ┌──────────▼──────────────────────────────┐
    │  1. LOCAL HUB SERVICES                  │
    │  - SearXNG/YaCy (local search)          │
    │  - Maddy/Stalwart (local email)         │
    │  - Nginx (local web server)             │
    │  - Kiwix (offline Wikipedia/libraries)  │
    │  - Your devices (always available)      │
    └──────────┬──────────────────────────────┘
               │ Not sufficient?
    ┌──────────▼──────────────────────────────┐
    │  2. HAI-NET MESH SERVICES               │
    │  - Federated search (peer indexes)      │
    │  - Hub-to-hub email (P2P)               │
    │  - Shared web content (mesh CDN)        │
    │  - Distributed Kiwix libraries          │
    │  - Regional hubs (trusted peers)        │
    │  - Global mesh (community network)      │
    └──────────┬──────────────────────────────┘
               │ Not available or authorized?
    ┌──────────▼──────────────────────────────┐
    │  3. EXTERNAL SERVICES                   │
    │  - Traditional web (via Bridge)         │
    │  - Cloud APIs (when necessary)          │
    │  - Internet search engines              │
    │  - Public email servers                 │
    └─────────────────────────────────────────┘
```

**Example Use Cases:**

**Search Request:**
1. Local: SearXNG searches local index + cached results
2. Mesh: Queries connected hubs for distributed search results
3. External: Falls back to traditional search engines if enabled

**Email Request:**
1. Local: Deliver to local mailbox if recipient is on same hub
2. Mesh: Direct P2P delivery to recipient's hub (no SMTP relay)
3. External: Traditional SMTP for non-HAI-Net recipients

**Knowledge Lookup:**
1. Local: Search local Kiwix library (Wikipedia, Stack Overflow, etc.)
2. Mesh: Query combined libraries from connected hubs
3. External: Web search as last resort

**Web Hosting:**
1. Local: Serve content from local Nginx server
2. Mesh: Share content with trusted peers (private CDN)
3. External: Proxy to public web if content not available locally

**And More**
Other "traditionally" external services will be added for further localisation and privacy depending on available local devices and shared network capacity.

---

# AI Intelligence Layer

```
┌─────────────────────────────────────┐
│          HUMAN USER                 │
│              ↕                      │
│    Natural Language Text, Audio,    |
|     Image or Video                  │
└───────────────┬─────────────────────┘
                │
┌───────────────▼─────────────────────┐
│           ADMIN AI                  │
│  • Primary interface                │
│  • Understands intent               │
│  • Orchestrates PM agents           │
│  • Blockchain-secured human link    │
└───────────────┬─────────────────────┘
                │
    ┌───────────┼───────────┐
    │           │           │
┌───▼────┐  ┌──▼────┐  ┌──▼───────┐
│PM:Comms│  │PM:Know│  │PM:Intent │
│Email,  │  │Learn, │  │Driven    │
│Chat    │  │Memory │  │Project   │
└───┬────┘  └───┬───┘  └───┬──────┘
    │           │           │
    Workers     Workers     Workers
(Email,     (Search,    (Files,
 Chat,       Tutor,      Network,
 Social)     Research)   Compute)
```

**Service Integration:**
The PM agents coordinate with local hub services:
- **PM:Comms** → Email Server, Web Server (for communication tasks)
- **PM:Knowledge** → Search Engine, Kiwix Library (for research/learning)
- **PM:Intent** → All services based on project needs

**Implementation Status:**
- ✅ Type system defined (AgentType, AgentState, MessageContent)
- ✅ Hierarchical communication infrastructure complete
- 🚧 Agent intelligence (Cycles 1+)
- 🔮 Local service integration (Phase 5+)

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

**Note:** For detailed multimodal architecture information (STT, TTS, Vision), see `PROJECT_STATUS.toml` Phase 2 section.
