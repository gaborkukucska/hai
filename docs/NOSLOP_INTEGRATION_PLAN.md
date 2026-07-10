# HAI-Net Home Hub <-> NoSlop Integration Plan

## Context & Architecture
HAI-Net is adapting its architecture to act as the `MASTER` Home Hub for the NoSlop mobile application.
- **The Hub (Rust)** handles 24/7 Tor connectivity, AI media processing (Whisper, Ollama, FFmpeg), and master data storage.
- **NoSlop (Android)** is the client, holding the same Ed25519 private key (Identity Clone model).
- The Hub must expose a secure API for the mobile app to sync data, push backups, and approve AI-processed media.

**LLM INSTRUCTION:** When implementing these phases, do NOT leave placeholder, mock, or simulated code. All implementations must be fully functional. Ensure standard Rust testing practices are followed and `docs/PROJECT_STATUS.toml` is updated upon completion.

---

### Phase 1: Headless Seed Deployment & Verification ✅ FULLY OPERATIONAL

**Goal:** Allow `hainet-seed` to be deployed silently via NoSlop's mobile SSH connection.

**Status:** Fully operational. NoSlop deploys HAI-Net by cloning the repo and running `cargo run --package hainet-seed --bin hainet-seed install -- --config hub_config.json`.

0. **Smart Auth State**: `hainet-core` now detects 'Identity Clone' deployments and provides a `qr_login_only` status to the frontend, bypassing the need for manual seed entry.
1. **CLI Argument Parsing (`hainet-seed/src/main.rs`)** ✅
   - `--config <file_path>` argument implemented via `clap` on the `Install` subcommand.
2. **JSON Config Ingestion (`hainet-seed/src/lib.rs`)** ✅
   - `HubConfig` struct with Serde deserialization accepting:
     - `shared_folder: Option<String>` — user-specified media path
     - `identity: Option<HubIdentity>` — full Identity Clone payload
   - `HubIdentity` struct with 6 fields: `public_key`, `private_key`, `enc_public_key`, `enc_private_key`, `onion_address`, `display_name`
   - When `--config` flag is present, all `dialoguer` terminal prompts are completely bypassed.
3. **Identity Clone Import (`hainet-seed/src/lib.rs`)** ✅
   - Receives Ed25519 and X25519 keypairs from NoSlop via the `identity` JSON block.
   - Writes each key component to `~/.hainet/identity/`:
     ```
     ~/.hainet/identity/         (drwx------)
     ├── ed25519_pub.b64         (-rw-------)
     ├── ed25519_priv.b64        (-rw-------)
     ├── x25519_pub.b64          (-rw-------)
     ├── x25519_priv.b64         (-rw-------)
     ├── onion_address           (-rw-------)
     └── display_name            (-rw-------)
     ```
   - File permissions hardened to `chmod 600` per file, `chmod 700` on directory (Unix only).
   - The Hub derives the same `.onion` address and `Handle.Tripcode` as the mobile app, enabling a true identity mirror.
4. **Config Persistence** ✅
   - Full `hub_config.json` is written to `/etc/hainet/hub_config.json` for `hainet-core` to read on startup.

### Phase 2: Dual Hidden Services & Remote API ✅ FULLY OPERATIONAL
**Status:** Mobile uses LAN IP locally, and falls back to SOCKS5 over Tor to hit the Hub's persistent `.onion` address on port 8080 globally.

1. **Dual Tor Services** ✅
   - Hub dynamically generates `hs_ed25519_secret_key` via SHA-512 expansion of the PKCS#8 mobile identity.
   - Configures `/etc/tor/torrc` to expose both Port 9999 (Mesh Gossip) and Port 8080 (REST API) on the same persistent `.onion`.
2. **Smart Firewall Integration** ✅
   - Hub listens on TCP 9999.
   - Mobile pushes its Contacts list (`sync_push_peers`) to populate the Hub's `TrustFirewall`.
   - Hub drops spam/untrusted packets at the edge and buffers valid packets in memory for mobile to pull (`sync_pull_packets`).
**Goal:** Allow NoSlop to securely communicate with the Hub over Tor.

1. **Dual Tor Services (`hainet-core/src/networking/tor.rs`)**
   - Register the primary public `.onion` for standard `hainet-social` gossip mesh (Port 9999).
   - Register a secondary private `.onion` for the REST API (Port 8080).
   - Protect the private API onion using Tor v3 Client Authentication (only NoSlop holds the auth cookie).
2. **API Endpoints (`hainet-core/src/api_router.rs`)**
   - Implement `/api/backup/push` (Receive AES-encrypted ZIP from NoSlop, write to `/media/hai-drive/backups`).
   - Implement `/api/backup/pull` (Serve the latest ZIP to NoSlop for mnemonic restoration).
   - Implement `/api/sync/clearnet` (Merge liked/saved RSS items).

### Phase 3: Deep Data Sync & Portal Population (NEXT)
**Goal:** Transition the Hub from an in-memory relay into a persistent database to populate the Web UI and offload mobile processing.

1. **Rust SQLite Persistence**
   - Replicate NoSlop's Room database schema (Posts, Comments, Reactions, DMs) in `hainet-social`.
   - When the Hub's firewall accepts a packet, persist it to disk *before* making it available for mobile sync.
   - Wire the React `hainet-portal` to this database so the Web UI displays the user's social feed and chats.
2. **Heavy Media Offloading**
   - Port NoSlop's AIMD chunk-downloading algorithm (`MediaManager.kt`) to Rust.
   - Allow the Hub to download 50MB video files over Tor 24/7. Mobile app will request the fully assembled MP4 over the LAN/Tor API instead of handling chunks itself.
3. **Creator Media Ingestion Pipeline**
   - Use `notify` to monitor `/media/hai-drive/uploads`. Trigger Whisper/Ollama metadata generation and queue for NoSlop approval.

1. **Directory Watcher (`hainet-media-mcp` or `hainet-core`)**
   - Use the `notify` crate to monitor `/media/hai-drive/uploads` for new video/audio/image files.
2. **AI Processing Pipeline**
   - **Transcription:** Trigger `Whisper.cpp` to transcribe audio.
   - **Metadata Generation:** Pass the transcription to the local `Ollama` service with a prompt to generate a Title, Description, and Hashtags.
   - **Thumbnail Extraction:** Execute `FFmpeg` to pull a high-quality frame.
3. **Database Insertion**
   - Store the generated metadata, file paths, and a `status = "PENDING"` flag in an SQLite `approval_queue` table.
4. **API Endpoints**
   - Implement `/api/studio/queue` (Return PENDING items to NoSlop).

### Phase 4: Channel Onion & Blockchain Linking
**Goal:** Separate a Creator's high-volume media broadcasts from their personal DMs/identity.

1. **Publish Endpoint (`hainet-core/src/api_router.rs`)**
   - Implement `/api/studio/publish` (Accept metadata edits from NoSlop, change status to `PUBLISHED`).
2. **Channel Identity Generation (`hainet-social/src/identity.rs`)**
   - Generate a *secondary* Ed25519 keypair for the Creator's "Channel".
   - Register a dedicated Channel `.onion` address.
3. **Blockchain Provenance (`hainet-chain`)**
   - When the first item is published, create a transaction on `hainet-chain` linking the Primary Identity (`Alice.x7z9`) to the Channel Identity (`Alice_Channel.q2p4`). 
   - This cryptographic link proves ownership and grants the "Creator Badge" on the mesh.
4. **Mesh Broadcast (`hainet-social/src/gossip.rs`)**
   - Hash the media file for CAS storage.
   - Broadcast the `POST` packet over the mesh originating from the *Channel* identity, not the personal identity.

### Phase 5: Active-Passive Mesh Reconciliation
**Goal:** Handle the scenario where NoSlop fell back to its own Tor daemon and has new data to merge.

1. **Merge Logic (`hainet-social/src/sync.rs`)**
   - When NoSlop reconnects and sends an `INVENTORY_SYNC_REQUEST`, compare local Post IDs, Reaction hashes, and DM nonces.
   - Accept any signed packets generated by NoSlop while offline.
   - Broadcast any newly merged packets to the broader HAI-Net mesh so the rest of the network catches up with what the user did while their Hub was offline.
