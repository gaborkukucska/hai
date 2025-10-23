# HAI-Net Multimodal Architecture

## Overview

This document describes the multimodal capabilities architecture for HAI-Net, focusing on Speech-to-Text (STT) integration as the first multimodal feature.

**Status**: Phase 0 - Foundation Complete (Cycle 2.2)  
**Last Updated**: 2025-10-23

---

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

---

## Speech-to-Text (STT) Integration

### Implementation Status (Cycle 2.2)

✅ **Completed**:
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

---

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

---

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

---

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

---

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

---

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

---

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

---

## Next Steps (Cycle 2.3+)

### Priority 1: Complete STT Flow
1. Implement Admin AI intent detection for audio input
2. Create STT Worker agent in `hainet-persona`
3. Extend `ai_providers` with STT capability types
4. Build `hainet-stt` MCP server with Whisper integration

### Priority 2: User Experience
1. Add "listening..." animation to VoiceInput
2. Display transcription in progress (streaming)
3. Allow editing transcription before sending
4. Save voice notes with timestamps

### Priority 3: Advanced Features
1. Multi-language support (auto-detect or manual select)
2. Speaker diarization (identify multiple speakers)
3. Punctuation and formatting improvements
4. Custom vocabulary for domain-specific terms

---

## References

- **Model Context Protocol**: See `helperfiles/external/MCP_SPECIFICATIONS.md`
- **AI Providers System**: See `hainet-persona/src/ai_providers/README.md` (TODO)
- **Agent Architecture**: See `helperfiles/PROJECT_BASED_AGENTIC_SYSTEM.md`
- **Whisper Documentation**: https://github.com/openai/whisper

---

## Changelog

### 2025-10-23 (Cycle 2.2)
- ✅ Created VoiceInput.tsx component with VAD
- ✅ Created STTHandler with placeholder logic
- ✅ Integrated STT into AdminBridge
- ✅ Added Tauri IPC command for transcription
- ✅ Added required dependencies
- ✅ Fixed compilation errors (Agent trait, Tauri v2 API)
- ✅ Successful compilation with only warnings
- 📝 Documented architecture and data flow
