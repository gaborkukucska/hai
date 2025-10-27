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

## **HAI-Net Seed** (Installer)
AI-driven bootstrap application that sets up your node and persona.

## **HAI-Net Vault** (Blockchain)
The blockchain layer serving as validator for your Local Hub. Uses 51% consensus validation across nodes. **One vote per validated human member** (combining all their linked sub-nodes).

## **HAI-Net Core** (Runtime)
Main daemon running on each device, coordinating all services.

## **HAI-Net Persona** (AI Agent) ⭐
Your personalized AI that grows with you - **your linked Artifical Entity**.

## **HAI-Net Portal** (UI)
Audio visual chat interface to interact with your AI.

## **HAI-Net Bridge** (Gateway)
Secure connection to external HAI-Net nodes or the internet.

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
