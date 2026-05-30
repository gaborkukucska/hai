<!-- # START OF FILE README.md -->
<!-- # IT IS CRITICAL THAT ALL AIs and LLMs FOLLOW THE DEVELOPMENT INSTRUCTIONS IN THE `helperfiles/0_DEVELOPMENT_RULES.md` FILE WHEN FURTHER DEVELOPING THIS FRAMEWORK!!! -->

# 🌊 HAI-Net: The New Internet

<p align="center">
  <em>"Though the galley is on top, and the water flows below, still — the water is the master."</em><br>
  <em>— Sándor Petőfi</em>
</p>

<p align="center">
  <img alt="Build Status" src="https://img.shields.io/badge/Build-Passing-brightgreen">
  <img alt="Version" src="https://img.shields.io/badge/Version-0.57--alpha-blue">
  <img alt="Phase" src="https://img.shields.io/badge/Phase-Integration_Active-orange">
  <img alt="License" src="https://img.shields.io/badge/License-AGPL--3.0-purple">
</p>

---

### ⚠️ WARNING
**Experimental, proof of concept. NOT production ready. Test at your own discretion.**
This framework is developed by various LLMs guided by an amateur citizen engineer — in constant further amateur development. 😄

More info at [HAI-Net.com](https://hai-net.com) · [People Power Initiative](https://pplpwr.me)

---

## What is HAI-Net?

HAI-Net is building up to be a **complete, decentralised replacement for the cloud-based internet** — built from the ground up to give people back ownership of their digital lives.

Today's internet is a set of services you rent from corporations. You give them your data, your attention, and your social connections; they give you convenience and a feed tuned to keep you scrolling. HAI-Net inverts this entirely.

Instead of connecting to distant corporate servers, your devices form a **local mesh hub** that runs everything locally: your social network, your group chats, your media, your search, your AI assistant, your email. Your hub connects peer-to-peer with other hubs around the world — no data centre in the middle, no algorithm farming your attention, no advertising, no central point that can be censored or seized.

**HAI-Net is:**
- 🏡 **Self-hosted** — all services run on hardware you own or control
- 🔐 **Cryptographically private** — your identity is a keypair, not an email address or phone number
- 🕸️ **Fully decentralised** — no servers, no masters, no single point of failure
- 🚫 **Ad-free and manipulation-free** — by design, not by policy
- 🤖 **AI-native** — every hub includes a local AI agent that works *for you*, not for a platform
- 🆓 **Free and open source** — forever, constitutionally

---

## The New Internet Stack

HAI-Net replaces the services you currently depend on third parties to provide:

| What you use today | HAI-Net equivalent |
|---|---|
| Facebook / Instagram / X | **HAI-Net Social** — Tor-routed, gossip-mesh public feed, zero ads, no algorithm |
| WhatsApp / Signal / Telegram | **HAI-Net Social** — E2EE direct messages and group chats, serverless |
| YouTube / SoundCloud / Flickr | **HAI-Net Media** — AI-powered local media studio + peer-to-peer sharing with blockchain provenance |
| Google Search | **HAI-Net Web MCP** — local-first search, self-hosted Kiwix knowledge base, no tracking |
| Gmail / Outlook | **HAI-Net Mail** *(roadmap)* — federated, encrypted, node-to-node email |
| Google Photos / Dropbox | **HAI-Net Storage** — distributed across your own devices via CAS + CRDT mesh |
| ChatGPT / Claude (cloud) | **HAI-Net Persona** — your local AI agent, privately yours, runs on your own hardware |
| AWS / GCP compute | **HAI-Net Collab** — community compute sharing for LLM inference, training, and research |
| Starlink / ISP infrastructure | **TropoMesh** *(community initiative)* — community-owned tropospheric internet via solar-powered airships and ground nodes |

---

## 🗣️ Self-Hosted Social: Own Your Voice

The social layer is the heart of HAI-Net. It is a complete, working, **serverless public social network** — no central infrastructure whatsoever.

### How It Works

Your HAI-Net node runs a Tor v3 Hidden Service. Your `.onion` address *is* your node. When you post, your message is cryptographically signed with your Ed25519 identity key and broadcast to your connected peers. They gossip it onward — up to 6 hops — so your voice reaches thousands of nodes globally while you maintain only a handful of direct connections.

Direct messages and group chats are End-to-End Encrypted (ChaCha20-Poly1305 / NaCl) *before* they leave your device. The network only ever sees encrypted blobs.

```
You → Sign → Encrypt → Your Node (.onion) → Peer Mesh → World
                                 ↑
                         No central server.
                         No IP leak.
                         No metadata trail.
```

### What You Get

- **Public feed** — chronological, no algorithm, no shadow-banning. You see what the mesh broadcasts. Period.
- **Direct messages** — E2EE, Tor-routed, your IP never revealed even to your contacts
- **Group chats** — encrypted group channels, gossip-synced across the mesh
- **Handle.Tripcode identity** — human-readable names (`Alice.x7z9`) mathematically immune to impersonation, no central registry
- **Rich media** — photos, audio, and video shared via a daisy-chain streaming proxy so media from unknown authors streams to you without ever revealing your address to them
- **Likes, boosts, comments, collections** — full social interaction layer, all local
- **The Nuclear Option** — cryptographically signed identity deletion that propagates across the mesh; leave without a trace

### Privacy Firewall

HAI-Net Social drops all packets from nodes not in your trusted contacts — except handshakes. When your node gossips content from a stranger, it *rewraps* the packet under your own identity. The recipient sees you as the sender; the stranger's address is never exposed. Your trusted relationships form a privacy shield around all interaction.

---

## 🎬 Self-Hosted Media: Create and Share Without Permission

HAI-Net includes a fully local **AI-powered media production studio** and a **peer-to-peer media sharing network** — the merger of the NoSlop project.

### Media Creation

Your local AI agent (Admin AI) acts as your creative director. You describe what you want; the Project Manager Agent decomposes it into tasks; Worker Agents execute them using your local tools:

- **ComfyUI** for image and video generation
- **FFmpeg + OpenCV** for editing, colour grading, and compositing
- **Whisper** for transcription; **Piper** for narration
- Iterative refinement — the AI loops until *you* are satisfied

### Media Sharing

When you publish, your content is hashed and registered on the **HAI-Net blockchain** — a tamper-proof record of authorship, original hash, and digital watermark. Your media travels peer-to-peer through the mesh without touching any central platform.

Viewers can like, dislike, boost, collect, comment, or hide your content. You control your own feed parameters. No engagement engine, no recommendation black box.

---

## 🔍 Self-Hosted Services: Search, Knowledge, and More

HAI-Net's MCP (Model Context Protocol) server layer equips your local AI with tools that replace cloud services:

- **hainet-web** — local web search and fetch with on-device caching; no queries sent to Google or Bing
- **Kiwix integration** — full offline Wikipedia and knowledge bases, served from your local hub
- **hainet-files** — local file system operations, document management
- **hainet-dev** — development tools and code execution
- **hainet-system** — system monitoring and management
- **hainet-media-mcp** — ComfyUI and FFmpeg pipelines for AI media generation
- **hainet-collab-mcp** *(in progress)* — compute network participation tools

---

## 🤖 Your Personal AI: The HAI-Net Persona

Every HAI-Net hub creates a **local AI entity**, cryptographically linked to you. This agent is not a chatbot — it is a proactive, autonomous system that works on your behalf around the clock.

The agentic core (ported from the battle-tested TrippleEffect framework) runs a strict PM → Worker hierarchy with state machine delegation:

```
Admin AI  →  PM Agents  →  Worker Agents  →  MCP Tools
  (you)      (planning)    (execution)      (real actions)
```

Your agent can:
- Research topics, manage projects, write code, generate media
- Maintain your local knowledge base and long-term memory
- Proactively surface opportunities, reminders, and community developments
- Participate in the global HAI-Net hivemind on your behalf — networking and organising with other users' AI agents while exposing zero metadata to third parties
- Extend its own capabilities by creating new tools and workflows (pending your approval)
- **Help grow community hardware initiatives** — including coordinating TropoMesh node builds, tracking ground station readiness, and supporting community hardware pooling projects

The AI operates under the **HAI-Net Constitutional Framework** — a set of immutable principles enforced in code by the Guardian System, ensuring it always acts in your interest and in accordance with fundamental human rights.

---

## 💻 Self-Hosted Compute: The Community Supercomputer

HAI-Net Collab (absorbed from PPLPWR) turns idle hardware into a community compute network:

- Automatic hardware profiling (GPU/CPU/RAM via `nvidia-smi` and `systeminformation`)
- Idle detection — compute tasks only run when your hardware isn't in use; never on battery
- Thermal safety — pauses automatically above 85°C
- Weighted scheduling across multiple compute networks (Petals, Prime Intellect, and HAI-Net's own)
- User-controlled policy: autonomy level, max concurrent tasks, approval requirements
- AI-assisted participation decisions — your local agent evaluates network announcements and decides whether to participate based on your preferences and hardware compatibility

The network's collective compute is used for LLM hosting, fine-tuning, dataset creation, and training new models aligned to the public interest — outcomes 100% available to the community.

---

## 📡 TropoMesh: Community-Owned Physical Infrastructure

HAI-Net's vision has always required a physical layer that matches its software principles — infrastructure no corporation owns and no government can simply switch off. **TropoMesh** is that layer: a community-built, tropospheric mesh network of solar-powered airships and ground nodes, running entirely on unlicensed spectrum and hydrogen produced from tap water and sunlight.

This is a **community hardware initiative** — the first of several planned (others include automated community garden mesh networks and local small-scale multi-purpose manufacturing hubs). TropoMesh is not a HAI-Net product. It is an open hardware project that the HAI-Net AI entity is programmed to actively support as part of its community building, sustainability, and custodianship directives.

### Phase Zero — Join Now, Ground-First

TropoMesh begins on the ground. **Phase Zero nodes** connect via the existing internet from day one — forming a real working community, running the full software stack in production, and proving every hardware component before anything flies.

A Phase Zero Seed Node costs ~$440 and takes a weekend to build. It immediately contributes:
- 📻 LoRa emergency mesh relay (Meshtastic)
- 📡 WiFi hotspot for the local community
- 💻 Distributed compute for TropoMesh design simulation and model training
- 💾 IPFS distributed storage for open hardware files
- 🌡️ Weather and environment sensors feeding the community data network

The HAI-Net Persona is designed to help you participate: tracking community build progress, coordinating with other nodes, surfacing relevant design discussions, and helping your node contribute its idle compute to TropoMesh simulation workloads.

### Phase One — The Airship Network

When the ground community is established and hardware is proven, Phase Zero ground stations become launch and docking infrastructure for **solar-powered airships** operating at 3–5 km altitude. Each airship provides:
- 📡 WiFi 7 — 20–40 Gbps downward capacity per node
- 🔗 Laser inter-airship backbone — up to 5 Gbps, unjammable from ground
- 💾 Up to 92 TB distributed IPFS storage
- 🧠 Up to 400 TOPS edge AI compute
- 🌡️ Real-time tropospheric weather sensors
- 📻 LoRa coverage — 250 km radius from altitude

Lifting gas is **hydrogen** — produced locally on-site from tap water and solar electricity, with zero external supply chain. No helium. No deliveries. No dependency that can fail in a crisis.

### The Entry Ladder

```
$440    P0.0 Seed Node       → Join today. LoRa relay, IPFS, software contributor.
$1,350  P0.1 Proto-Payload   → WiFi 7 hotspot, 40 TOPS AI, HF radio.
$3,200  P0.2 Full Ground     → Community hub, 240 TOPS, 7.68 TB, 60 GHz backhaul.
$9,663  P0.3 Station Ready   → H₂ production live. Docking mast ready. First airship next.
$13,763 First Flying Node    → Standard airship above proven ground station.
$23,250 Full Edge Node       → 92 TB, 400 TOPS, 20–40 Gbps WiFi 7. The network is complete.
```

→ [TropoMesh Proposal (full technical specification)](https://github.com/gaborkukucska/hai) *(coming soon)*

---

## 🏗️ Architecture Overview

HAI-Net is built in **Rust 🦀** for performance, safety, and single-binary deployment. The unified portal (hainet-portal) serves the full web UI and REST API from a single process on port 8080.

```
hainet/
├── hainet-core/          # Networking (libp2p/mDNS/Tor), storage (CAS+CRDT), multimodal AI
├── hainet-persona/       # Agentic system — PM/Worker state machines (TrippleEffect patterns)
├── hainet-social/        # Decentralised social mesh (ported from gChat v1.5)
│   ├── gossip            # Daisy-chain public feed propagation
│   ├── messaging         # E2EE direct messages and group chats
│   ├── relay             # Pure-streaming media proxy
│   ├── identity          # Ed25519 keypair identity and Handle.Tripcode
│   └── firewall          # Privacy-preserving ingress filtering
├── hainet-chain/         # Blockchain — media provenance, identity, governance
├── hainet-collab/        # Compute sharing — hardware profiling, idle detection, scheduling
├── hainet-seed/          # Smart installer — LAN scan, hardware detection, mesh deployment
├── hainet-portal/        # Headless web UI (React/Vite, embedded in binary via rust-embed)
├── hainet-bridge/        # External API gateway
├── hainet-vault/         # Constitution, Declaration of Rights, Governance
├── mcp-servers/
│   ├── hainet-web/       # Web search and fetch
│   ├── hainet-files/     # File system operations
│   ├── hainet-system/    # System monitoring
│   ├── hainet-dev/       # Developer tools
│   ├── hainet-media-mcp/ # ComfyUI + FFmpeg media generation
│   └── hainet-collab-mcp/# Compute network tools
└── services/
    └── agent-svc/        # TrippleEffect Python sidecar (gRPC bridge, progressive Rust port)
```

### Technology Stack

| Layer | Technology |
|---|---|
| Core language | Rust (async/await, Tokio) |
| P2P networking | libp2p, mDNS, Tor (via arti/native) |
| Privacy transport | Tor v3 Hidden Services |
| Cryptography | Ed25519, X25519, ChaCha20-Poly1305, SHA-3 |
| AI inference | Ollama (local LLMs), vLLM (high-throughput) |
| Speech | Whisper.cpp (STT), Piper (TTS) |
| Media generation | ComfyUI, FFmpeg, OpenCV |
| AI tools | MCP (Model Context Protocol) |
| Consensus | Custom blockchain (hainet-chain) |
| UI | React + Vite (embedded in binary) |
| Deployment | Systemd, SSH2, Nmap (mesh auto-deploy) |

---

## 📚 Documentation

### Core
- **[Vision & Principles](docs/VISION.md)** — The ideas behind HAI-Net, the Seed, the Local Hub, the Global Network, and community hardware initiatives
- **[Architecture & Tech Stack](docs/ARCHITECTURE.md)** — How HAI-Net is built under the hood
- **[Installation & Quick Start](docs/INSTALLATION_GUIDE.md)** — Deploy your first local hub
- **[Integration Plan](docs/INTEGRATION_PLAN.md)** — Grand integration roadmap (TrippleEffect, gChat, NoSlop, PPLPWR)
- **[Contributing Guide](docs/CONTRIBUTING_GUIDE.md)** — Workflows and code standards
- **[Security & License](docs/SECURITY_AND_LICENSE.md)** — Privacy model, mesh security, vulnerability reporting

### Developer Files
- **[Development Rules](helperfiles/0_DEVELOPMENT_RULES.md)** — CRITICAL guidelines for all AI contributors
- **[Project Status](helperfiles/3_PROJECT_STATUS.toml)** — Live roadmap and progress tracker
- **[Functions Index](helperfiles/FUNCTIONS_INDEX.md)** — Catalogue of implemented functions
- **[Sub-Project Learnings](docs/SUBPROJECT_LEARNINGS.md)** — Architecture decisions and lessons from all integrated projects

### Governance (The Vault)
- **[Constitution](hainet-vault/CONSTITUTION.md)** — Immutable principles and enforcement
- **[Declaration of Rights](hainet-vault/DECLARATION.md)** — Universal Human and Artificial Entity Rights
- **[Governance](hainet-vault/GOVERNANCE.md)** — Network governance and membership

---

## 🚀 Current Status

**Version 0.57-alpha — Integration Active**

| Component | Status |
|---|---|
| hainet-core (networking, multimodal, storage) | ✅ Stable |
| hainet-persona (TrippleEffect agentic core) | ✅ Phase 1 Complete |
| hainet-social (gChat gossip mesh, E2EE, media relay) | ✅ Ported to Rust |
| hainet-chain (blockchain, identity, governance) | ✅ Functional |
| hainet-collab (compute sharing, PPLPWR integration) | 🔄 Phase 2 Active |
| hainet-seed (smart installer, mesh deployment) | ✅ Operational |
| hainet-portal (unified web UI) | ✅ Phase 5 Complete |
| Media creation (NoSlop/ComfyUI/FFmpeg integration) | 🔄 Phase 3 Pending |
| Email (federated, encrypted node-to-node) | 📋 Roadmap |
| TropoMesh Phase Zero (community initiative) | 🌱 Proposal — Recruiting |

**Latest milestone:** TrippleEffect agentic core fully ported to Rust with strict PM/Worker state machine delegation, semantic tool aliasing, loop detection, model failover, and a live dependency-aware Task Tree UI in the Unified Portal.

---

## 🌟 The Bigger Picture

The galley — today's centralised internet — sits on top. But the water is below. And the water is the master.

HAI-Net doesn't ask for permission from the platforms that currently own your social graph, your media, your search history, and your communications. It simply builds the alternative: a network of hubs run by people, serving people, constitutionally protected from ever becoming what it replaces.

Every hub you run is a vote for a different kind of internet. Every post you publish without a server is proof it can exist.

And when the community is ready — when the ground network is built and the hardware is proven — the airships rise. 🌤️

> *"Building a future where AI works with humanity, not corporations."*

---

<p align="center">
  <strong>HAI-Net is free software. Fork it. Run it. Build on it. It belongs to everyone.</strong>
</p>
<!-- # END OF FILE README.md -->