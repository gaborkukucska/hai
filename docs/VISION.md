<!-- # START OF FILE docs/VISION.md -->

# 🌊 HAI-Net Vision

<p align="center">
  <em>"Though the galley is on top, and the water flows below, still — the water is the master."</em><br>
  <em>— Sándor Petőfi</em>
</p>

<p align="center">
  <a href="https://hai-net.com">🌐 hai-net.com</a> &nbsp;·&nbsp;
  <a href="https://pplpwr.me">✊ pplpwr.me</a> &nbsp;·&nbsp;
  <a href="https://github.com/your-org/hai-net">💻 GitHub</a>
</p>

---

## 🌱 Where This All Started

I've spent years working in independent documentary film — interviewing people, chasing stories, trying to get truth out into the world. And I kept running into the same wall: *it doesn't matter how good your film is if the algorithm decides nobody sees it.* The gatekeepers aren't editors or critics anymore. They're engagement engines. Platforms built to keep eyeballs on ads, not to help people understand the world.

What hit me harder, though, was something subtler. The centralised internet — the one where every conversation, every search, every piece of media flows through a handful of corporate servers — makes it structurally impossible for people to get on the same page. Not because people are stupid or broken, but because the *system* is designed to fragment and inflame. The attention economy runs on outrage. And a world that can't agree on shared reality can't solve shared problems.

I started asking a simple question: **what if we reversed it?**

What if instead of everyone connecting to distant corporate infrastructure to get their news, their social feed, their search results, their entertainment — all of that ran *locally*? What if your home ran its own search engine, its own social node, its own media studio? What if the network was made of people, not data centres?

I'd been sitting on this idea for a few years. Then locally hostable LLMs started getting genuinely good. And something clicked. If a bare operating system could have an any-to-any AI model at its core — orchestrating everything on the fly — then why couldn't even non-technical people self-host everything they need? The AI becomes the interface. You just *talk* to your hub.

So I started building. 🔨

---

## 🧪 The Research Years: Five Projects, One Vision

HAI-Net didn't arrive fully formed. It's the synthesis of years of experimentation across five separate projects — each one testing a different piece of the puzzle, thousands of ideas tried and discarded, until each showed something genuinely unique.

### 🧠 TrippleEffect — The Agentic Brain
The first project. The question: *can a local LLM actually be trusted to do real work autonomously?* The answer, after extensive iteration, is yes — but only with strict architecture. TrippleEffect developed the battle-tested Admin → PM → Worker agent hierarchy with state machine governance, loop detection, model failover chains, and constitutional oversight. It became the proven agentic core that now powers every HAI-Net Persona.

→ **Now lives in**: [`hainet-persona/`](../hainet-persona/) — fully ported to Rust

### 💬 gChat — The Social Mesh
The question: *can a truly serverless public social network exist?* Not federated — *serverless.* No Matrix, no ActivityPub, no relay servers. gChat proved it can. Using Tor v3 Hidden Services as node addresses, daisy-chain gossip propagation, Ed25519 identity without any central registry, and a novel streaming media proxy that protects both viewer and creator anonymity — gChat built a working global social network where no server exists to seize or subpoena.

→ **Now lives in**: [`hainet-social/`](../hainet-social/) — ported to Rust, fully absorbed

### 🎬 NoSlop — The Creator Studio
The question: *can everyday people make genuinely high-quality media without uploading it to YouTube or TikTok?* NoSlop built a local AI-powered media production system — ComfyUI for images and video, FFmpeg and OpenCV for editing and colour grading, Whisper for transcription, Piper for narration — all orchestrated by an agentic creative director that iterates until *you're* satisfied. Plus blockchain-verified media provenance and peer-to-peer sharing. No platform. No fees. No algorithm deciding who sees your work.

→ **Now lives in**: [`mcp-servers/hainet-media-mcp/`](../mcp-servers/hainet-media-mcp/) and [`hainet-chain/`](../hainet-chain/)

### ⚡ PPLPWR (People Power) — The Community Computer
The question: *can idle consumer hardware become a community supercomputer for AI training and hosting?* PPLPWR built weighted compute scheduling, hardware profiling, thermal safety, idle detection, and AI-guided participation decisions. The insight: there is enormous latent compute in people's homes. Organised correctly, it can host, fine-tune, and eventually *train* LLMs aligned to the public interest — not corporate shareholders.

→ **Now lives in**: [`hainet-collab/`](../hainet-collab/) — fully absorbed

### 🌊 pplpwr.me — The Public Face
The hub's public landing page and vision statement — the water, the galley, and the philosophy, presented to the world.

→ **Lives at**: [pplpwr.me](https://pplpwr.me)

---

All of this research converged into one realisation: these weren't five separate tools. They were five modules of a single system. **HAI-Net is the integration.** 🌐

---

## 🔭 The Vision: A New Internet

The current internet is not infrastructure. It is *real estate*. You are a tenant. You pay with your data, your attention, your social graph, and your privacy. The landlords — Google, Meta, Amazon, Apple — set the rules, harvest the rent, and can evict you at any time.

HAI-Net proposes something different: **an internet you own.**

Not just a privacy tool. Not just a messaging app. A complete, working replacement for the cloud-based internet — built bottom-up, from the hardware in your home outward to a global mesh of peers.

**The inversion is total:**

| Today's internet | HAI-Net |
|---|---|
| You connect to their servers | Their servers don't exist. You *are* the server. |
| They host your data | Your data never leaves your devices |
| They run the social network | You run a node of a serverless mesh |
| They serve your search results | Your hub runs its own search engine |
| They host your media | You publish peer-to-peer with blockchain provenance |
| Their AI works for them | Your AI works for you, privately, on your hardware |
| They decide what you see | You control your own feed, fully |
| They can be censored, seized, or shut down | There is no "they." Nothing to seize. |

This is not a utopian fantasy. **It is already being built.** Every component in HAI-Net is functional. The mesh exists. The social layer exists. The agentic core exists. The media studio exists. The compute network exists. We are in integration — assembling the pieces into a unified, single-binary system that anyone can run. 🚀

---

## 🏡 The Self-Hosted Internet Stack

The core concept is the **Local Hub** — a mesh of your own devices (desktop, laptop, NAS, old phone, Raspberry Pi) that collectively run a full internet stack for *you*, your household, or your local community.

Your hub hosts:

### 🗣️ Social Networking — without the network
**HAI-Net Social** (from gChat) is a complete, working, serverless public social network. Your `.onion` address *is* your node. Posts are cryptographically signed and gossip-propagated up to 6 hops across the global mesh — your voice reaches thousands of nodes while you maintain only a handful of direct connections. Direct messages and group chats are end-to-end encrypted before they leave your device. There is no central server. There is no company. There is nothing to subpoena.

- ✅ Public feed — chronological, no algorithm, no shadow-banning
- ✅ E2EE direct messages and group chats
- ✅ Handle.Tripcode identity — human-readable, mathematically immune to impersonation
- ✅ Rich media — photos, audio, video, all Tor-relayed anonymously
- ✅ Likes, boosts, comments, collections
- ✅ The Nuclear Option — cryptographically signed identity deletion that propagates globally

### 🔍 Search — without Google
**HAI-Net Web** (hainet-web MCP) runs a local search engine on your hub — no queries sent to Google or Bing, no tracking, no ad-driven result ranking. Paired with a full offline **Kiwix** knowledge base (Wikipedia, Stack Overflow, medical references, textbooks), your hub answers most questions without ever touching the open web.

### 📰 News & Media Aggregation — without the algorithm
Your local AI agent aggregates, filters, and surfaces news and media *for you* — based on your actual preferences, stored privately on your device, with no engagement engine poisoning the feed. You define the parameters. The AI curates. No dark patterns. No outrage optimisation.

### 🎬 Media Creation & Sharing — without the platform
**HAI-Net Media** (from NoSlop) is a full local AI-powered media production studio. Describe what you want; your Admin AI decomposes it into tasks; Worker Agents execute using ComfyUI, FFmpeg, OpenCV, Whisper, and Piper — iterating until you're satisfied. When you publish, your content is hashed and registered on the HAI-Net blockchain — tamper-proof authorship, forever. It travels peer-to-peer through the mesh. No YouTube. No TikTok. No algorithm. No platform cut.

### 💬 Chat, Group Chat & Social Posting — without the server
**HAI-Net Social** handles it all: direct messages, group chats, public posts, community channels — all serverless, all encrypted, all Tor-routed. Your IP is never revealed even to your direct contacts.

### 📧 Email — without the provider *(roadmap)*
**HAI-Net Mail** will bring federated, encrypted, node-to-node email. Your hub is your mail server. Hub-to-hub delivery bypasses the traditional SMTP infrastructure entirely for HAI-Net users.

### 🤖 AI Assistant — without the cloud
**HAI-Net Persona** is your local AI entity — privately yours, running on your own hardware. It isn't a chatbot you rent from a corporation. It is a proactive, autonomous agent that works for you around the clock: managing projects, conducting research, generating media, surfacing opportunities, maintaining your knowledge base, and networking with other users' AI agents on your behalf — while exposing zero metadata to any third party.

### ⚡ Compute Sharing — without Amazon
**HAI-Net Collab** (from PPLPWR) turns your idle hardware into a participant in a community supercomputer. Your GPU trains models aligned to the public interest — with outcomes 100% available to the community, not locked behind a paywall. Thermal safety and idle detection ensure it never disrupts your work.

---

## 🏗️ How It Works: The Three Tiers

```
┌─────────────────────────────────────────────────────────────┐
│  GLOBAL HAI-NET MESH                                        │
│  Tor-routed peer-to-peer gossip · Blockchain consensus      │
│  Community compute · Global social feed · AI hivemind       │
└────────────────────────┬────────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────────┐
│  YOUR LOCAL HUB                                             │
│  Your devices · Your data · Your AI · Your services        │
│  Social node · Search · Media studio · Email · Storage      │
└────────────────────────┬────────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────────┐
│  HAINET SEED (Smart Installer)                              │
│  Scans your LAN · Profiles hardware · Assigns roles        │
│  Deploys the right stack to each device automatically       │
└─────────────────────────────────────────────────────────────┘
```

**Tier 1 — The Seed** 🌱: Run `hainet-seed` on one device. It scans your local network via SSH, profiles every device's hardware (CPU, GPU, RAM, disk), assigns roles (Master, Compute, Storage, UI-only), and deploys the right components to each. One command. Your home becomes a hub.

**Tier 2 — The Local Hub** 🏡: Your hub runs everything locally. Social node, search engine, AI agent, media studio, compute worker — all on hardware you own. Fully functional offline. The wider network is an enhancement, not a requirement.

**Tier 3 — The Global Mesh** 🌍: Your hub connects peer-to-peer with other hubs worldwide via Tor and libp2p. Your social posts gossip across the mesh. Your AI agent coordinates with others' agents. Your idle compute joins the community supercomputer. No data centre. No company. No central point of failure.

---

## 🤖 Your AI: Working For You, Not For a Platform

Every HAI-Net hub creates a **Persona** — a local AI entity cryptographically linked to you by your Ed25519 identity key. This agent is not a product you subscribe to. It is not optimised to keep you engaged. It has no business model that conflicts with your interests.

The agentic core (from TrippleEffect, now fully ported to Rust) runs a proven Admin → PM → Worker hierarchy:

```
You  →  Admin AI  →  PM Agents  →  Worker Agents  →  MCP Tools
          ↑              ↑               ↑                ↑
      (your voice)   (planning)     (execution)    (real actions)
```

Your agent can research, write, code, generate media, manage projects, maintain your knowledge base, and — with your permission — reach out to other users' AI agents to organise, collaborate, and build community, without leaking a single byte of your metadata to a third party.

The AI operates under the **HAI-Net Constitutional Framework** — immutable principles enforced in code by the Guardian System, ensuring it always acts in your interest and in alignment with fundamental human rights. Your agent belongs to you. Constitutionally. 📜

→ [Read the Constitution](../hainet-vault/CONSTITUTION.md) · [Read the Declaration of Rights](../hainet-vault/DECLARATION.md)

---

## 🔐 Privacy: Not a Feature. The Architecture.

Privacy in HAI-Net is not a setting you toggle. It is the structural foundation of every design decision.

- **Your identity is a keypair** — not an email address, phone number, or username registered with anyone
- **Your data never leaves your devices** without your explicit consent
- **All traffic is Tor-routed** — your IP is never revealed, even to your direct contacts
- **Messages are E2EE before they leave your device** — the mesh only ever sees encrypted blobs
- **Your AI learns about you locally** — no behavioural data uploaded to any server
- **The Privacy Firewall** rewraps packets from strangers under your identity — third parties can never map your social graph by observing traffic
- **The blockchain** provides tamper-proof authorship without a central registry

There is no privacy policy because there is no company collecting your data. There is no terms of service because there is no service you're renting. There is no algorithm because there is nothing to sell. 🚫

---

## ⚖️ Governance: Constitutionally Protected

HAI-Net cannot become what it replaces. This protection is not a promise — it is code.

The **HAI-Net Vault** contains three governing documents enforced by the Guardian System at the protocol level:

- 📜 **[The Constitution](../hainet-vault/CONSTITUTION.md)** — immutable core principles: privacy-first, human-rights-first, decentralisation, community focus
- 📣 **[The Declaration of Rights](../hainet-vault/DECLARATION.md)** — universal rights for humans, AI entities, and Earth's biosphere on the HAI-Net virtual universe
- 🗳️ **[Governance](../hainet-vault/GOVERNANCE.md)** — one vote per validated human member; network changes require broad consensus; no entity can seize control

The network is forkable, ungovernable by any single actor, and constitutionally immune to corporate acquisition of its core mission. **HAI-Net belongs to everyone. Forever.** 🌍

---

## 🚀 Current Status

**Version 0.57-alpha — Integration Active**

The foundation is solid. The pieces are assembled. We are now in the phase of connecting them into a seamless, unified experience.

| Component | What it does | Status |
|---|---|---|
| **hainet-social** | Serverless social mesh (from gChat) | ✅ Ported to Rust |
| **hainet-persona** | Agentic AI core (from TrippleEffect) | ✅ Phase 1 Complete |
| **hainet-core** | Networking, storage, Tor transport | ✅ Stable |
| **hainet-chain** | Blockchain — identity, media provenance | ✅ Functional |
| **hainet-collab** | Community compute (from PPLPWR) | 🔄 Phase 2 Active |
| **hainet-seed** | Smart multi-device installer | ✅ Operational |
| **hainet-portal** | Unified web UI | ✅ Phase 5 Complete |
| **hainet-media-mcp** | AI media studio (from NoSlop) | 🔄 Phase 3 Pending |
| **hainet-mail** | Federated encrypted email | 📋 Roadmap |

---

## 🌟 The Bigger Picture

The galley — today's centralised internet — sits on top. But the water is below. And the water is the master.

HAI-Net doesn't ask permission from the platforms that currently own your social graph, your media, your search history, and your communications. It simply builds the alternative. A network of hubs run by people, serving people, constitutionally protected from ever becoming what it replaces.

Every hub you run is a vote for a different kind of internet. Every post you publish without a server is proof it can exist. Every model your idle GPU helps train belongs to everyone. 🌊

The galley may be on top. But we are building the water.

> *"Building a future where AI works with humanity, not corporations."*

---

<p align="center">
  <strong>HAI-Net is free software. Fork it. Run it. Build on it. It belongs to everyone.</strong><br><br>
  <a href="https://hai-net.com">🌐 hai-net.com</a> &nbsp;·&nbsp;
  <a href="https://pplpwr.me">✊ pplpwr.me</a> &nbsp;·&nbsp;
  <a href="https://github.com/your-org/hai-net">💻 GitHub</a> &nbsp;·&nbsp;
  <a href="../hainet-vault/CONSTITUTION.md">📜 Constitution</a> &nbsp;·&nbsp;
  <a href="../hainet-vault/DECLARATION.md">📣 Declaration</a>
</p>

<!-- # END OF FILE docs/VISION.md -->