# 🏗️ Architecture & Technology Stack

## Architecture Overview

HAI-Net consists of several interconnected Rust crates:

```
hainet/
├── hainet-core/          # Core daemon, orchestration, and backend API provider for the UI
├── hainet-persona/       # 🤖 Multi-agent AI system
├── hainet-chain/         # Blockchain & governance
├── hainet-seed/          # 🚀 Smart installer & mesh deployer
├── hainet-portal/        # Headless Web UI (React/Vite app embedded in core via rust-embed)
├── hainet-bridge/        # External API gateway
├── hainet-collab/        # Compute sharing & hardware profiling
├── hainet-social/        # Privacy-first decentralized social (gossip, E2E encryption)
└── mcp-servers/          # External tools for AI (e.g., hainet-media-mcp for ComfyUI/FFmpeg)
```

## 🛠️ Technology Stack

**Core:**
- Rust 🦀 (async/await with Tokio)
- TOML (configuration & templates)
- Handlebars (template rendering)

**Networking:**
- Libp2p (P2P mesh)
- mDNS (local device discovery)
- SSH2 (secure mesh deployment)

**UI & Frontend:**
- React & Vite (Headless Web Portal)
- Unified Port 8080 (Single binary serves both REST API and React UI)
- `rust-embed` (UI assets baked directly into the daemon binary)

**AI Integration:**
- Ollama (local LLM hosting)
- Whisper.cpp (speech-to-text)
- Piper (text-to-speech)
- ComfyUI & FFmpeg (media generation and processing via MCP)
- MCP (Model Context Protocol for AI tools)

**Deployment:**
- Nmap (network device discovery)
- Systemd (service management on Linux)
- Ed25519 SSH keys (mesh authentication)
