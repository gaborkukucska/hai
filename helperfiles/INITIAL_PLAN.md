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
# Multimodal Architecture

## Overview

This section describes HAI-Net's multimodal capabilities architecture, focusing on Speech-to-Text (STT) integration as the first multimodal feature.

**Status**: Phase 0 - Foundation Complete (Cycle 2.2)  
**Last Updated**: 2025-10-23

## Architecture Principles

### 1. **Portal-Centric User Interface**
- All user interactions happen through `hainet-portal` (Tauri desktop app)
- Portal captures multimodal inputs (audio, video, images, etc.)
- Portal displays multimodal outputs (text, audio, visualizations)

### 2. **Admin AI Orchestration**
- Admin AI receives all user requests (text or multimodal)
- Admin AI decides which specialized Worker to spawn/reuse
- Workers handle specific modality processing (STT, TTS, image analysis, etc.)

### 3. **Provider Discovery via AI Providers System**
- Workers use `hainet-persona::ai_providers` to discover capabilities
- Providers registered via MCP servers (e.g., `hainet-stt` for Whisper)
- Dynamic selection based on availability, performance, and cost

### 4. **MCP Tools for External Services**
- MCP servers expose external AI services as tools
- Example: `hainet-stt` server provides `transcribe_audio` tool
- Standardized interface across different provider backends

## Speech-to-Text (STT) Integration

1. **Frontend Voice Input Component** (`hainet-portal/src/components/VoiceInput.tsx`)
   - MediaRecorder API for audio capture
   - Voice Activity Detection (VAD) with configurable threshold
   - Real-time audio level visualization
   - Base64 encoding for audio transmission

2. **Backend STT Handler** (`hainet-portal/src-tauri/src/stt_handler.rs`)
   - Placeholder for STT processing logic
   - Structured types for audio data and transcription results
   - Prepared for Admin AI integration

3. **Admin AI Bridge Integration** (`hainet-portal/src-tauri/src/admin_bridge.rs`)
   - Added `transcribe_audio()` method
   - Routes audio through STT handler
   - Ready for Worker spawning logic

4. **Tauri IPC Commands** (`hainet-portal/src-tauri/src/lib.rs`)
   - Added `transcribe_audio` command
   - Frontend can invoke via `@tauri-apps/api/core`

🚧 **Pending** (Next Cycles):
- Admin AI intent detection for STT requests
- Worker spawning for STT processing
- Provider discovery via `ai_providers` system
- MCP `hainet-stt` server implementation
- Integration with Whisper (Ollama or external API)

### Data Flow (Current Implementation)

```
┌─────────────────┐
│  User speaks    │
│  into mic       │
└────────┬────────┘
         │
         ▼
┌─────────────────────────────────────────┐
│  VoiceInput.tsx                         │
│  • Captures audio via MediaRecorder     │
│  • Detects voice activity (VAD)         │
│  • Encodes to Base64                    │
└────────┬────────────────────────────────┘
         │ invoke('transcribe_audio', audio)
         ▼
┌─────────────────────────────────────────┐
│  lib.rs (Tauri IPC)                     │
│  • transcribe_audio command             │
└────────┬────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────┐
│  admin_bridge.rs                        │
│  • AdminBridge::transcribe_audio()      │
└────────┬────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────┐
│  stt_handler.rs                         │
│  • STTHandler::transcribe()             │
│  • Returns placeholder error for now    │
└─────────────────────────────────────────┘
```

### Data Flow (Target Architecture)

```
┌─────────────────┐
│  User speaks    │
│  into mic       │
└────────┬────────┘
         │
         ▼
┌─────────────────────────────────────────┐
│  VoiceInput.tsx                         │
│  • Audio capture + VAD                   │
└────────┬────────────────────────────────┘
         │ IPC: transcribe_audio
         ▼
┌─────────────────────────────────────────┐
│  AdminBridge                            │
│  • Routes to Admin AI                   │
└────────┬────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────┐
│  Admin AI Agent                         │
│  • Detects STT intent                   │
│  • Spawns/reuses STT Worker             │
└────────┬────────────────────────────────┘
         │ Message bus
         ▼
┌─────────────────────────────────────────┐
│  STT Worker                             │
│  • Discovers STT provider               │
│  • Calls MCP hainet-stt tool            │
└────────┬────────────────────────────────┘
         │ MCP protocol
         ▼
┌─────────────────────────────────────────┐
│  hainet-stt MCP Server                  │
│  • Whisper integration                  │
│  • Returns transcription                │
└────────┬────────────────────────────────┘
         │
         ▼ (results flow back up)
┌─────────────────────────────────────────┐
│  VoiceInput.tsx                         │
│  • Displays transcription               │
│  • Inserts into chat input              │
└─────────────────────────────────────────┘
```

## Key Components

### 1. VoiceInput Component

**File**: `hainet-portal/src/components/VoiceInput.tsx`

**Features**:
- 🎤 One-click recording with microphone access
- 📊 Real-time audio level visualization
- 🟢 Voice Activity Detection (VAD) with adjustable threshold
- ⚙️ Audio settings optimized for Whisper (16kHz, mono, Opus codec)
- 🎛️ User controls for VAD enable/disable and threshold adjustment

**Props**:
```typescript
interface VoiceInputProps {
  onTranscription: (text: string) => void;
  onError?: (error: string) => void;
}
```

**Audio Format**:
- Sample rate: 16,000 Hz (optimal for Whisper)
- Channels: Mono (1 channel)
- Format: WebM with Opus codec
- Encoding: Base64 for IPC transmission

### 2. STTHandler

**File**: `hainet-portal/src-tauri/src/stt_handler.rs`

**Types**:
```rust
pub struct AudioData {
    pub data: String,        // Base64-encoded audio
    pub sample_rate: u32,    // Sample rate in Hz
    pub channels: u16,       // Number of channels
    pub format: String,      // Audio format (e.g., "webm")
}

pub struct TranscriptionResult {
    pub text: String,           // Transcribed text
    pub confidence: f32,        // Confidence score (0.0-1.0)
    pub language: String,       // Detected language code
    pub processing_time_ms: u64, // Processing time
}
```

**Current Implementation**:
- Placeholder that returns an error
- TODO: Integrate with Admin AI for Worker spawning

### 3. AdminBridge STT Method

**File**: `hainet-portal/src-tauri/src/admin_bridge.rs`

```rust
pub async fn transcribe_audio(&self, audio: AudioData) -> Result<TranscriptionResult>
```

**TODO (Next Cycles)**:
1. Detect STT intent from audio input
2. Spawn/reuse STT Worker via Admin AI
3. Worker discovers STT provider via `ai_providers`
4. Worker calls MCP `hainet-stt` tool
5. Return transcription result to Portal

## Future Multimodal Capabilities

### Planned Features

1. **Text-to-Speech (TTS)**
   - Admin AI speaks responses aloud
   - User preference for voice/accent
   - Streaming audio playback

2. **Image Analysis**
   - Upload images to Admin AI
   - Vision models analyze content
   - Multimodal context in conversations

3. **Screen Sharing**
   - Admin AI sees user's screen
   - Context-aware assistance
   - Visual debugging support

4. **Video Processing**
   - Upload video clips
   - Frame-by-frame analysis
   - Action recognition

### MCP Servers Roadmap

| Server | Purpose | Status |
|--------|---------|--------|
| `hainet-stt` | Whisper STT integration | 🚧 Planned (Cycle 2.3+) |
| `hainet-tts` | Text-to-speech synthesis | 🔮 Future |
| `hainet-vision` | Image/video analysis | 🔮 Future |
| `hainet-ocr` | Text extraction from images | 🔮 Future |

## Integration with Existing Systems

### AI Providers System

The `hainet-persona::ai_providers` module will be extended to support multimodal providers:

```rust
pub enum ProviderCapability {
    TextGeneration,
    CodeGeneration,
    SpeechToText,      // ← NEW
    TextToSpeech,      // ← NEW
    ImageAnalysis,     // ← NEW
    VideoProcessing,   // ← NEW
}
```

**Provider Discovery**:
1. Worker queries `ProviderCatalog` for STT-capable providers
2. Catalog returns available providers (e.g., Whisper via MCP)
3. Worker selects best provider based on ranking
4. Worker invokes provider via MCP tool

### Worker Agents

New specialized Workers for multimodal processing:

- **STT Worker**: Transcribes audio using Whisper
- **TTS Worker**: Synthesizes speech from text
- **Vision Worker**: Analyzes images/video
- **Multimodal Worker**: Combines multiple modalities

Workers follow the same lifecycle as existing Workers (Planning → Executing → Completed).

## Technical Notes

### Audio Capture Challenges

1. **Browser Compatibility**: MediaRecorder API support varies
   - Chrome/Edge: Excellent (WebM + Opus)
   - Firefox: Good (WebM + Opus)
   - Safari: Limited (may need fallback to WAV)

2. **Sample Rate**: 16kHz is optimal for Whisper
   - Reduces file size while maintaining quality
   - Must be explicitly set in getUserMedia constraints

3. **VAD Threshold**: Default 0.5 works for most cases
   - Lower threshold = more sensitive (picks up more background noise)
   - Higher threshold = less sensitive (may miss quiet speech)

### Performance Considerations

1. **Audio Encoding**: Base64 increases size by ~33%
   - Consider binary WebSocket for large files in future
   - For short voice commands (<10s), overhead is acceptable

2. **Transcription Latency**: Depends on provider
   - Local Whisper (via Ollama): 1-5s for short clips
   - Cloud APIs (OpenAI, AssemblyAI): 2-10s with network latency
   - Streaming transcription: Possible future enhancement

## Testing Strategy

### Unit Tests
- ✅ STTHandler placeholder logic (compiles correctly)
- 🚧 Audio data serialization/deserialization
- 🚧 VAD threshold calculations

### Integration Tests
- 🚧 End-to-end STT flow (Portal → Admin AI → Worker → MCP → Portal)
- 🚧 Provider discovery and selection
- 🚧 Error handling for missing providers

### Manual Testing
- 🚧 Record audio in VoiceInput component
- 🚧 Verify audio level visualization
- 🚧 Test VAD with different threshold values
- 🚧 Validate transcription accuracy

## Dependencies

### New Crates (Added in Cycle 2.2)

In `hainet-portal/src-tauri/Cargo.toml`:
```toml
base64 = "0.21"            # Audio data encoding
reqwest = "0.11"           # HTTP client for future API calls
tracing = "0.1"            # Structured logging
```

### Frontend Dependencies

In `hainet-portal/package.json` (existing):
```json
"@tauri-apps/api": "^2.0.0"  // IPC communication
```

## References

- **Model Context Protocol**: See `helperfiles/external/MCP_SPECIFICATIONS.md`
- **AI Providers System**: See `hainet-persona/src/ai_providers/` (implementation in progress)
- **Whisper.cpp Documentation**: https://github.com/ggml-org/whisper.cpp
