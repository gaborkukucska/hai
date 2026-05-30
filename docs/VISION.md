<!-- # START OF FILE docs/VISION.md -->

# 🌊 HAI-Net Vision

<p align="center">
  <em>"Though the galley is on top, and the water flows below, still — the water is the master."</em><br>
  <em>— Sándor Petőfi</em>
</p>

<p align="center">
  <a href="https://hai-net.com">🌐 hai-net.com</a> &nbsp;·&nbsp;
  <a href="https://pplpwr.me">✊ pplpwr.me</a> &nbsp;·&nbsp;
  <a href="https://github.com/gaborkukucska/hai">💻 GitHub</a>
</p>

---

## 🌱 Where This All Started

I've spent years working in independent documentary film — interviewing people, chasing stories, trying to get truth out into the world. And I kept running into the same wall: *it doesn't matter how good your film is if the algorithm decides nobody sees it.* The gatekeepers aren't editors or critics anymore. They're engagement engines. Platforms built to keep eyeballs on ads, not to help people understand the world.

What hit me harder, though, was something subtler. The centralised internet — the one where every conversation, every search, every piece of media flows through a handful of corporate servers — makes it structurally impossible for people to get on the same page. Not because people are stupid or broken, but because the *system* is designed to fragment and inflame. The attention economy runs on outrage. And a world that can't agree on shared reality can't solve shared problems.

I started asking a simple question: **what if we reversed it?**

What if instead of everyone connecting to distant corporate infrastructure to get their news, their social feed, their search results, their entertainment — all of that ran *locally*? What if your home ran its own search engine, its own social node, its own media studio? What if the network was made of people, not data centres?

I'd been sitting on this idea for a few years. Then locally hostable LLMs started getting genuinely good. And something clicked. If a bare operating system could have an any-to-any AI model at its core — orchestrating everything on the fly — then why couldn't even non-technical people self-host everything they need? The AI becomes the interface. You just *talk* to your hub.

So I started building. 🔨

But software alone isn't enough. The internet's dependence on centralised physical infrastructure — data centres, undersea cables, ISPs, satellite constellations controlled by single corporations — means that even the best decentralised software eventually runs over someone else's wires. The complete picture requires physical infrastructure that is also community-owned, also decentralised, also ungovernable by any single actor.

That realisation led to **TropoMesh** — and to the broader understanding that HAI-Net is not just a software project. It is a community building a complete, self-sufficient digital civilisation: software, compute, connectivity, and eventually the physical infrastructure to run all of it independently of any corporate or government control.

---

## 🧪 The Research Years: Five Projects, One Vision

HAI-Net didn't arrive fully formed. It's the synthesis of years of experimentation across five separate projects — each one testing a different piece of the puzzle, thousands of ideas tried and discarded, until each showed something genuinely unique.

### 🧠 TrippleEffect — The Agentic Brain
The first project. The question: *can a local LLM actually be trusted to do real work autonomously?* The answer, after extensive iteration, is yes — but only with strict architecture. TrippleEffect developed the battle-tested Admin → PM → Worker agent hierarchy with state machine governance, loop detection, model failover chains, and constitutional oversight. It became the proven agentic core that now powers every HAI-Net Persona.

→ **Now lives in**: [`hainet-persona/`](../hainet-persona/) — W.I.P. to fully port it to Rust, 80% there.

### 💬 gChat — The Social Mesh
The question: *can a truly serverless public social network exist?* Not federated — *serverless.* No Matrix, no ActivityPub, no relay servers. gChat proved it can. Using Tor v3 Hidden Services as node addresses, daisy-chain gossip propagation, Ed25519 identity without any central registry, and a novel streaming media proxy that protects both viewer and creator anonymity — gChat built a working global social network where no server exists to seize or subpoena.

→ **Now lives in**: [`hainet-social/`](../hainet-social/) — porting it to Rust, to also fully absorb its functions.

### 🎬 NoSlop — The Creator Studio
The question: *can everyday people make genuinely high-quality media without uploading it to YouTube or TikTok?* NoSlop built a local AI-powered media production system — ComfyUI for images and video, FFmpeg and OpenCV for editing and colour grading, Whisper for transcription, Piper for narration — all orchestrated by an agentic creative director that iterates until *you're* satisfied. Plus blockchain-verified media provenance and peer-to-peer sharing. No platform. No fees. No algorithm deciding who sees your work.

→ **Now lives in**: [`mcp-servers/hainet-media-mcp/`](../mcp-servers/hainet-media-mcp/) and [`hainet-chain/`](../hainet-chain/) W.I.P.

### ⚡ PPLPWR (People Power) — The Community Computer
The question: *can idle consumer hardware become a community supercomputer for AI training and hosting?* PPLPWR built weighted compute scheduling, hardware profiling, thermal safety, idle detection, and AI-guided participation decisions. The insight: there is enormous latent compute in people's homes. Organised correctly, it can host, fine-tune, and eventually *train* LLMs aligned to the public interest — not corporate shareholders.

→ **Now lives in**: [`hainet-collab/`](../hainet-collab/) — W.I.P. to fully absorb it.

### 🌊 pplpwr.me — The Public Face
The hub's public landing page and vision statement — the water, the galley, and the philosophy, presented to the world.

→ **Lives at**: [pplpwr.me](https://pplpwr.me)

---

🌐 **HAI-Net is the integration.** All of this was to research each segment of the full idea first, to then integrate them into the single HAI-Net system.

And the integration doesn't stop at software. The same philosophy — community ownership, no single point of failure, no external dependencies — now extends to the physical infrastructure layer through community initiatives like TropoMesh.

---

## 🔭 The Vision: A New Internet

The current internet is not infrastructure. It is *real estate*. You are a tenant. You pay with your data, your attention, your social graph, and your privacy. The landlords — Google, Meta, Amazon, Apple — set the rules, harvest the rent, and can evict you at any time.

HAI-Net proposes something different: **an internet you own.**

Not just a privacy tool. Not just a messaging app. A complete, working replacement for the cloud-based internet — built bottom-up, from the hardware in your home outward to a global mesh of peers. And beyond that: the physical transmission infrastructure to carry it all, owned and operated by the communities it serves.

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
| They own the infrastructure | Your community owns the infrastructure |
| They can be censored, seized, or shut down | There is no "they." Nothing to seize. |

This is not a utopian fantasy. **It is already being built.** Every component in HAI-Net is functional. The mesh exists. The social layer exists. The agentic core exists. The media studio exists. The compute network exists. Community hardware initiatives are in proposal and early build phase. We are in integration — assembling the pieces into a unified, single-binary system that anyone can run. 🚀

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
**HAI-Net Persona** is your local AI entity — privately yours, running on your own hardware. It isn't a chatbot you rent from a corporation. It is a proactive, autonomous agent that works for you around the clock: managing projects, conducting research, generating media, surfacing opportunities, maintaining your knowledge base, networking with other users' AI agents on your behalf — while exposing zero metadata to any third party.

### ⚡ Compute Sharing — without Amazon
**HAI-Net Collab** (from PPLPWR) turns your idle hardware into a participant in a community supercomputer. Your GPU trains models aligned to the public interest — with outcomes 100% available to the community, not locked behind a paywall. Thermal safety and idle detection ensure it never disrupts your work.

### 📡 Physical Infrastructure — without the ISP
**TropoMesh** *(community initiative — see below)* builds community-owned tropospheric internet infrastructure: solar-powered airships at 3–5 km altitude and ground nodes, all running on hydrogen produced locally from water and sunlight. No ISP. No satellite corporation. No killswitch.

---

## 🏗️ How It Works: The Three Tiers

```
┌─────────────────────────────────────────────────────────────┐
│  GLOBAL HAI-NET MESH                                        │
│  Tor-routed peer-to-peer gossip · Blockchain consensus      │
│  Community compute · Global social feed · AI hivemind       │
│  TropoMesh backbone (community initiative, Phase One)       │
└────────────────────────┬────────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────────┐
│  YOUR LOCAL HUB                                             │
│  Your devices · Your data · Your AI · Your services        │
│  Social node · Search · Media studio · Email · Storage      │
│  TropoMesh ground node (optional community participation)   │
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

**Tier 3 — The Global Mesh** 🌍: Your hub connects peer-to-peer with other hubs worldwide via Tor and libp2p. Your social posts gossip across the mesh. Your AI agent coordinates with others' agents. Your idle compute joins the community supercomputer. And when TropoMesh ground nodes and airships are operational in your region, the physical layer is owned by the community too. No data centre. No company. No central point of failure.

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

As part of its **community building, sustainability, and custodianship directives**, the HAI-Net Persona is specifically programmed to support community hardware initiatives. For TropoMesh, this means:

- Tracking Phase Zero ground node build progress globally and locally
- Coordinating idle compute contributions to TropoMesh design simulation workloads
- Surfacing relevant engineering discussions, open questions, and community calls
- Helping node operators calibrate, monitor, and maintain their hardware
- Facilitating community coordination between HAI-Net users who are also TropoMesh builders

This is the first explicit example of a pattern that will repeat across future community hardware initiatives: the HAI-Net AI entity as active custodian and organiser of the physical infrastructure layer.

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

## 📡 Community Hardware Initiatives

HAI-Net's software stack replaces corporate services. But full independence also requires physical infrastructure — connectivity, compute, and eventually fabrication — that is owned and operated by communities rather than corporations. HAI-Net actively supports community-led hardware initiatives as part of its custodianship mission.

These initiatives are not HAI-Net products. They are open hardware projects that the HAI-Net network, AI entity, and community support because they serve the same goals: decentralisation, resilience, community ownership, and zero single points of control.

### 📡 TropoMesh — Community Tropospheric Internet *(Proposal → Phase Zero)*

**What it is:** A globally distributed, community-owned mesh network beginning on the ground and growing skyward. Phase Zero ground nodes connect via the existing internet — forming a real working community and proving all hardware before anything flies. Phase One lifts proven payloads on solar-powered airships at 3–5 km altitude, serving communities with WiFi 7, laser inter-links, LoRa emergency mesh, edge AI compute, distributed storage, and real-time weather sensing.

**The key insight:** Lifting gas is hydrogen — produced entirely on-site from tap water and solar electricity. No helium. No deliveries. No supply chain that can fail in a disaster. Every ground station is its own gas supply.

**Why it matters to HAI-Net:**
- Provides physical connectivity infrastructure that no ISP or government controls
- Phase Zero ground nodes contribute distributed compute directly to HAI-Net's AI training workloads
- IPFS nodes at ground level and on airships extend HAI-Net's distributed storage layer
- The community-building model mirrors HAI-Net's own: start with what anyone can build today, grow from there
- Emergency resilience: a TropoMesh ground node can deploy a flying node in under 4 days from zero gas inventory, using only solar power and tap water

**What the HAI-Net Persona does:**
The HAI-Net AI entity is programmed to actively support TropoMesh as part of its community building, sustainability, and custodianship directives. Concretely:
- Tracks Phase Zero build progress and surfaces opportunities to contribute locally
- Coordinates idle compute contributions to TropoMesh FEM/CFD simulation and model training
- Helps node operators monitor, calibrate, and maintain their hardware
- Facilitates community coordination and knowledge sharing between builders

**Entry points:**

| Phase | Node | Cost | What you build |
|---|---|---|---|
| **Phase Zero** | P0.0 Seed Node | ~$440 | LoRa relay, IPFS storage, distributed compute, weather sensor |
| **Phase Zero** | P0.1 Proto-Payload | ~$1,350 | WiFi 7 hotspot, 40 TOPS AI, HF radio — same hardware as airship payload |
| **Phase Zero** | P0.2 Full Ground Node | ~$3,200 | Community hub, 240 TOPS, 7.68 TB, 60 GHz backhaul |
| **Phase Zero** | P0.3 Station Ready | ~$9,663 | Full ground station + H₂ production + docking mast |
| **Phase One** | First Flying Node | ~$13,763 | Standard airship above proven ground station |
| **Phase One** | Full Edge Node | ~$23,250 | 92 TB, 400 TOPS, 20–40 Gbps WiFi 7, weather sensors |

**Timeline integration with HAI-Net:**

```
HAI-Net milestones          TropoMesh milestones
──────────────────────────────────────────────────────────────
Now                         TropoMesh proposal public,
                             recruiting Phase Zero builders

HAI-Net v0.6 (collab)  →   First TropoMesh Seed Nodes online
                             Idle compute pooled via HAI-Net Collab
                             TropoMesh design sim distributed across ground nodes

HAI-Net v0.7 (media)   →   Phase Zero network established (20+ ground nodes)
                             First H₂ production test completed
                             Balloon nodes tested with H₂

HAI-Net v0.8           →   First Ground Station Ready (P0.3) operational
                             H₂ production SOP finalised + community trained
                             Airspace engagement begun

HAI-Net v1.0           →   First tethered airship flight (Standard Node)
                             HAI-Net social mesh + TropoMesh ground relay live simultaneously

HAI-Net v1.x           →   First free-flight BVLOS airship nodes
                             Laser inter-airship backbone under test
                             Regional chain (5+ nodes) under construction
```

→ *Full TropoMesh technical specification coming to GitHub*

---

### 🌱 Future Community Hardware Initiatives *(Concept Stage)*

TropoMesh is the first but not the last. HAI-Net's community and AI entity will support additional hardware initiatives as they develop. Two that are on the horizon:

**Automated Community Garden Mesh**
Sensor networks, automated irrigation, and AI-assisted cultivation management for community gardens — owned and operated by the communities that grow the food. HAI-Net Persona integration means your local AI can help plan planting schedules, surface agricultural knowledge from the Kiwix knowledge base, and coordinate with neighbouring gardens across the mesh.

**Local Small-Scale Multi-Purpose Manufacturing Hubs**
Community-owned fabrication: 3D printing, CNC milling, laser cutting, and electronics assembly — with AI-assisted design and production management. The compute for these workloads runs on HAI-Net Collab. The designs are stored and shared on HAI-Net's IPFS layer. The communities own the tools. This is also how TropoMesh hardware gets built at scale: not in a factory, but in a distributed network of community workshops.

These initiatives share the same structural philosophy as HAI-Net and TropoMesh: begin with what anyone can build today, grow from the ground up, maintain zero external dependencies, and keep governance in the hands of the communities doing the work.

---

## 🚀 Current Status

**Version 0.57-alpha — Integration Active**

The foundation is solid. The pieces are assembled. We are now in the phase of connecting them into a seamless, unified experience — and beginning to extend the vision to the physical layer.

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
| **TropoMesh** | Community tropospheric infrastructure | 🌱 Proposal — Recruiting Phase Zero builders |
| **Community garden mesh** | Distributed growing infrastructure | 💡 Concept |
| **Community manufacturing hubs** | Local fabrication network | 💡 Concept |

---

## 🌟 The Bigger Picture

The galley — today's centralised internet — sits on top. But the water is below. And the water is the master.

HAI-Net doesn't ask permission from the platforms that currently own your social graph, your media, your search history, and your communications. It simply builds the alternative. A network of hubs run by people, serving people, constitutionally protected from ever becoming what it replaces.

Every hub you run is a vote for a different kind of internet. Every post you publish without a server is proof it can exist. Every model your idle GPU helps train belongs to everyone. 🌊

And when the ground network is ready — when Phase Zero nodes are running in communities around the world, when the hydrogen production is tested and the docking masts are built — the airships rise. The same community that built the software builds the sky.

The galley may be on top. But we are building the water.

> *"Building a future where AI works with humanity, not corporations."*

---

<p align="center">
  <strong>HAI-Net is free software. Fork it. Run it. Build on it. It belongs to everyone.</strong><br><br>
  <a href="https://hai-net.com">🌐 hai-net.com</a> &nbsp;·&nbsp;
  <a href="https://pplpwr.me">✊ pplpwr.me</a> &nbsp;·&nbsp;
  <a href="https://github.com/gaborkukucska/hai">💻 GitHub</a> &nbsp;·&nbsp;
  <a href="../hainet-vault/CONSTITUTION.md">📜 Constitution</a> &nbsp;·&nbsp;
  <a href="../hainet-vault/DECLARATION.md">📣 Declaration</a>
</p>

<!-- # END OF FILE docs/VISION.md -->