# 🏗️ Architecture & Technology Stack

## Architecture Overview

HAI-Net consists of several interconnected Rust crates:

```
hainet/
├── hainet-core/          # Core daemon, orchestration, and backend API provider for the UI
├── hainet-persona/       # 🤖 Multi-agent AI system
├── hainet-chain/         # Blockchain & governance
├── hainet-seed/          # 🚀 Smart installer & mesh deployer
├── hainet-portal/        # Headless Web UI (React/Vite app served by hainet-core or Axum)
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
- React & Vite (Headless Web Portal)
- Standard HTTP/REST APIs (decoupled from Tauri for network accessibility)
- Axum (static asset serving)

**AI Integration:**
- Ollama (local LLM hosting)
- Whisper.cpp (speech-to-text)
- Piper (text-to-speech)
- MCP (Model Context Protocol for AI tools)

**Deployment:**
- Nmap (network device discovery)
- Systemd (service management on Linux)
- Ed25519 SSH keys (mesh authentication)
