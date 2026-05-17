# 🏗️ Architecture & Technology Stack

## Architecture Overview

HAI-Net consists of several interconnected Rust crates:

```
hainet/
├── hainet-core/          # Multimodal features, networking, and orchestration
├── hainet-persona/       # 🤖 Multi-agent AI system
├── hainet-chain/         # Blockchain & governance
├── hainet-seed/          # 🚀 Smart installer & mesh deployer
├── hainet-portal/        # Web UI (stub — Tauri integration planned)
└── hainet-bridge/        # External API gateway
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

**UI:**
- Tauri (desktop app framework — planned)
- React (frontend library — planned)

**AI Integration:**
- Ollama (local LLM hosting)
- Whisper.cpp (speech-to-text)
- Piper (text-to-speech)
- MCP (Model Context Protocol for AI tools)

**Deployment:**
- Nmap (network device discovery)
- Systemd (service management on Linux)
- Ed25519 SSH keys (mesh authentication)
