//! HAI-Net Core Daemon
//!
//! The main daemon that coordinates all HAI-Net services including networking,
//! storage, communication with other components, and the web portal UI.
//!
//! This is the single entry point: it serves both the REST API and the static
//! React frontend on one port (default 8080), so users only need one URL.
//!
//! Works in both development mode (cargo run) and as a deployed systemd service.

use tracing::{info, debug, warn};
use anyhow::Result;

pub mod admin_bridge;
pub mod metrics_handler;
pub mod metrics_storage;
pub mod settings_handler;
pub mod settings_storage;
pub mod stt_handler;
pub mod tts_handler;
pub mod video_handler;
pub mod vision_handler;
pub mod api_router;

use std::sync::Arc;
use tokio::sync::RwLock;
use admin_bridge::AdminBridge;
use tts_handler::TTSHandler;
use hainet_persona::agents::metrics::MetricsCollector;
use metrics_storage::MetricsStorage;
use settings_storage::SettingsStorage;

// --- Integration imports: Compute sharing (PPLPWR) & Social mesh (gChat) ---
use hainet_collab::hardware::HardwareProfile;
use hainet_social::gossip::GossipEngine;

// --- Embedded static UI assets (from hainet-portal/dist/) ---
use rust_embed::RustEmbed;

/// Embedded React UI built by `npm run build` inside hainet-portal.
/// The `folder` path is relative to the hainet-core crate root.
#[derive(RustEmbed)]
#[folder = "../hainet-portal/dist/"]
struct PortalAssets;

/// Shared application state passed to every API handler.
#[derive(Clone)]
pub struct SocialDb {
    pub pool: sqlx::SqlitePool,
}
impl SocialDb {
    pub async fn new(db_path: &str) -> anyhow::Result<Self> {
        let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(5).connect(db_path).await?;
        sqlx::query("CREATE TABLE IF NOT EXISTS posts (id TEXT PRIMARY KEY, author TEXT, content TEXT, timestamp TEXT, media_id TEXT, media_type TEXT);").execute(&pool).await?;
        sqlx::query("CREATE TABLE IF NOT EXISTS dms (id TEXT PRIMARY KEY, peer TEXT, sender TEXT, content TEXT, timestamp INTEGER, media_id TEXT, media_type TEXT);").execute(&pool).await?;
        sqlx::query("CREATE TABLE IF NOT EXISTS mesh_peers (public_key TEXT PRIMARY KEY, is_trusted BOOLEAN, handle TEXT, onion_address TEXT);").execute(&pool).await?;
        let _ = sqlx::query("ALTER TABLE mesh_peers ADD COLUMN onion_address TEXT;").execute(&pool).await;
        let _ = sqlx::query("ALTER TABLE mesh_peers ADD COLUMN enc_public_key TEXT;").execute(&pool).await;
        // NOSLOP_HUB_WEBUI_SEND_V1: 1 for a DM the Hub sent itself (typed in the Web UI): the phone keeps a copy
        // but must not send it again.
        let _ = sqlx::query("ALTER TABLE dms ADD COLUMN sent_by_hub INTEGER NOT NULL DEFAULT 0;").execute(&pool).await;
        Ok(Self { pool })
    }
}

pub struct AppState {
    pub admin_bridge: Option<Arc<RwLock<AdminBridge>>>,
    pub tts_handler: Arc<RwLock<TTSHandler>>,
    pub hardware_profile: Arc<RwLock<HardwareProfile>>,
    pub gossip_engine: Arc<RwLock<GossipEngine>>,
    pub social_db: Arc<SocialDb>,
    /// The NoSlop Home HUB node: verified mesh ingress, the phone's durable inbox, creator hosting,
    /// Tor delivery and the owner API (`hainet_noslop::hub`).
    pub noslop: Arc<hainet_noslop::hub::HubNode>,
    /// Nonces of owner-signed API requests already seen (replay guard).
    pub api_nonces: Arc<hainet_noslop::hub::api::NonceCache>,
    /// Portal browser sessions (cookie token -> expiry).
    pub sessions: Sessions,
    /// Admin AI message ids already answered (the phone re-sends until its ACK arrives).
    pub admin_seen: Arc<tokio::sync::Mutex<std::collections::VecDeque<String>>>,
    pub log_dir: std::path::PathBuf,
}

/// A social feed post (temporary in-memory struct until full gossip integration)
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct SocialPost {
    pub id: String,
    pub author: String,
    pub content: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
}

pub type MetricsState = Arc<RwLock<MetricsCollector>>;
pub type MetricsStorageState = Arc<RwLock<MetricsStorage>>;
pub type SettingsState = Arc<RwLock<SettingsStorage>>;

/// The owner's message to the Hub's Admin AI (`admin_<owner key>`), handed over by the phone through
/// `hub_send`. NoSlop 0.7+ seals it as DM v2; the Hub acknowledges it at once (`DM_ACK` v3, so the
/// phone stops re-sending), asks the Admin AI, and puts the v2 reply in the phone's inbox.
pub async fn handle_incoming_dm(app_state: Arc<AppState>, packet: hainet_noslop::packet::Packet) {
    use hainet_noslop::hub::admin;
    let node = app_state.noslop.clone();
    let Some(owner) = node.personal().await else {
        tracing::error!("Admin AI: this Hub has no owner identity; cannot open the message");
        return;
    };
    let (msg_id, text) = match admin::open_owner_message(&owner, &packet) {
        Ok(x) => x,
        Err(e) => { tracing::error!("Admin AI: {}", e); return; }
    };
    match admin::ack_packet(&owner, &msg_id, node.now()) {
        Ok(ack) => node.inbox("packet", &ack, &ack.to_wire()).await,
        Err(e) => tracing::error!("Admin AI: could not build the DM_ACK: {}", e),
    }
    // The phone re-sends a message until its ACK arrives; each message is answered once.
    {
        let mut seen = app_state.admin_seen.lock().await;
        if seen.contains(&msg_id) { return; }
        seen.push_back(msg_id.clone());
        while seen.len() > 512 { seen.pop_front(); }
    }
    tracing::info!("Admin AI: message from the owner opened; asking the Admin AI...");
    let reply_text = match &app_state.admin_bridge {
        Some(bridge_arc) => {
            let bridge = bridge_arc.read().await;
            match bridge.send_message(text, vec![]).await {
                Ok(response) => response.message.content,
                Err(e) => {
                    tracing::error!("Admin AI Error: {}", e);
                    format!("⚠️ Admin AI Error: {}\n\n(Check your Hub logs, MCP servers or persona config might have failed to start)", e)
                }
            }
        }
        None => "⚠️ Admin AI is disabled on this node (not a Master node or failed to init).".to_string(),
    };
    match admin::reply_packet(&owner, &reply_text, node.now()) {
        Ok(reply) => {
            node.inbox("packet", &reply, &reply.to_wire()).await;
            tracing::info!("Admin AI: reply sealed and queued for the phone");
        }
        Err(e) => tracing::error!("Admin AI: could not seal the reply: {}", e),
    }
}
#[tokio::main]
async fn main() -> Result<()> {
    // Step 1: Load configuration (before logging, so we know the log dir)
    let config = hainet_core::config::HainetConfig::load();

    // Step 2: Initialize logging with the configured log directory
    let _guard = hainet_core::logging::initialize_logging_with_dir(
        "hainet-core",
        &config.logs.log_level,
        Some(&config.logs.log_dir),
    )?;

    info!("🌐 HAI-Net Core Daemon starting up...");
    info!("📋 Version: {}", env!("CARGO_PKG_VERSION"));
    info!("🏛️ Constitutional compliance: ENFORCED");
    info!("📄 Role: {}", config.role_display());
    info!("🔌 Configured port: {}", config.network.port);
    if let Some(ref master_ip) = config.network.master_ip {
        info!("🎯 Master node: {}", master_ip);
    }

    // Step 3: Find an available port for the health endpoint
    let health_port = hainet_core::config::find_available_port(config.network.port)
        .unwrap_or(config.network.port);
    
    if health_port != config.network.port {
        warn!("⚠  Port {} in use, health endpoint on port {}", config.network.port, health_port);
    }

    // Initialize database directory from config
    let data_dir = std::path::PathBuf::from(&config.storage.data_dir);
    let _ = std::fs::create_dir_all(&data_dir);
    
    // Determine prompts directory (sibling to data_dir or from repo)
    let mut prompts_dir = data_dir.parent().unwrap_or(&data_dir).join("prompts");
    let repo_prompts = std::path::PathBuf::from("/home/tom/hai/hainet-persona/prompts");
    let cwd_prompts = std::env::current_dir().unwrap_or_default().join("hainet-persona/prompts");
    
    if !prompts_dir.exists() {
        if repo_prompts.exists() {
            prompts_dir = repo_prompts;
        } else if cwd_prompts.exists() {
            prompts_dir = cwd_prompts;
        }
    }
    
    // --- Integration: Detect local hardware (PPLPWR port) ---
    info!("🖥️  Detecting local hardware profile (hainet-collab)...");
    let hardware_profile = HardwareProfile::detect();
    info!(
        "✅ Hardware: {} cores, {:.1} GB RAM, GPU: {}, Score: {:.1}",
        hardware_profile.cpu_cores,
        hardware_profile.ram_total_gb,
        hardware_profile.gpu.as_ref().map_or("None".to_string(), |g| g.name.clone()),
        hardware_profile.capability_score
    );

    // Initialize Admin AI Bridge only on master nodes
    let role_lower = config.network.role.to_lowercase();
    let admin_bridge = if role_lower == "master" || role_lower == "standalone" {
        let max_ctx = hardware_profile.max_safe_context_length();
        match AdminBridge::new(data_dir.clone(), prompts_dir, config.network.role.clone(), max_ctx).await {
            Ok(bridge) => Some(bridge),
            Err(e) => {
                tracing::error!("Failed to initialize Admin AI Bridge: {}", e);
                None
            }
        }
    } else {
        info!("Skipping Admin AI Bridge initialization on non-master node");
        None
    };

    
    // Initialize MetricsCollector with database path
    let metrics_db_path = data_dir.join("metrics.db");
    let metrics_collector = MetricsCollector::new(
        &format!("sqlite://{}?mode=rwc", metrics_db_path.display())
    )
    .await
    .expect("Failed to initialize MetricsCollector");
    
    // Initialize MetricsStorage with database path
    let metrics_storage_path = data_dir.join("metrics_history.db");
    let metrics_storage = MetricsStorage::new(
        &format!("sqlite://{}?mode=rwc", metrics_storage_path.display())
    )
    .await
    .expect("Failed to initialize MetricsStorage");
    
    // Initialize SettingsStorage with database path
    let settings_db_path = data_dir.join("settings.db");
    let settings_storage = SettingsStorage::new(
        &format!("sqlite://{}?mode=rwc", settings_db_path.display())
    )
    .await
    .expect("Failed to initialize SettingsStorage");
    


    // --- Integration: Initialize gossip engine (gChat port) ---
    let pub_key_path = get_hainet_dir().join("identity/ed25519_pub.b64");
    let node_id = std::fs::read_to_string(&pub_key_path)
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| uuid::Uuid::new_v4().to_string());
    info!("🗣️  Initializing gossip engine (hainet-social), node_id={}", &node_id[..std::cmp::min(8, node_id.len())]);
    let gossip_engine = GossipEngine::new(node_id);
    debug!("Gossip engine created with {} max hops", hainet_social::gossip::DEFAULT_MAX_HOPS);
    
    // Wrap states in Arc<RwLock<>> for shared state
    let metrics_state: MetricsState = Arc::new(RwLock::new(metrics_collector));
    let metrics_storage_state: MetricsStorageState = Arc::new(RwLock::new(metrics_storage));
    let settings_state: SettingsState = Arc::new(RwLock::new(settings_storage));
    let qr_sessions = Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new()));
    let tts_handler = TTSHandler::new();
    
    let social_db_path = data_dir.join("social.db");
    let social_db = SocialDb::new(&format!("sqlite://{}?mode=rwc", social_db_path.display()))
        .await
        .expect("Failed to initialize SocialDb");

    // Startup Trust Hydration
    if let Ok(rows) = sqlx::query("SELECT public_key FROM mesh_peers WHERE is_trusted = 1").fetch_all(&social_db.pool).await {
        for row in rows {
            use sqlx::Row;
            if let Ok(pub_key) = row.try_get::<String, _>("public_key") {
                gossip_engine.trust_peer(pub_key).await;
            }
        }
    }

    // Step 3.5: NoSlop Home HUB node (hainet-noslop): verified mesh listeners for the personal and
    // creator onions (loopback; Tor forwards to them), the durable inbox for the phone, creator
    // hosting, and Tor delivery. Replaces the earlier unverified 0.0.0.0:9999 relay.
    let hub_cfg = hainet_noslop::hub::HubConfig::from_env(get_hainet_dir().join("identity"), data_dir.clone());
    info!("🕸️  NoSlop Home HUB: personal mesh on {}, creator mesh on {}, Tor SOCKS {}", hub_cfg.personal_listen, hub_cfg.creator_listen, hub_cfg.socks);
    let noslop = hainet_noslop::hub::HubNode::new(
        hub_cfg,
        hainet_noslop::hub::store::Store::open(social_db.pool.clone()).await.expect("Failed to initialize the NoSlop Hub store"),
        None,
        true,
        hainet_noslop::hub::system_clock(),
    ).await;

    let app_state = Arc::new(AppState {
        admin_bridge: admin_bridge.map(|b| Arc::new(RwLock::new(b))),
        tts_handler: Arc::new(RwLock::new(tts_handler)),
        hardware_profile: Arc::new(RwLock::new(hardware_profile)),
        gossip_engine: Arc::new(RwLock::new(gossip_engine)),
        social_db: Arc::new(social_db),
        noslop: noslop.clone(),
        api_nonces: Arc::new(hainet_noslop::hub::api::NonceCache::default()),
        sessions: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        admin_seen: Arc::new(tokio::sync::Mutex::new(std::collections::VecDeque::new())),
        log_dir: config.effective_log_dir(),
    });
    let hook_state = app_state.clone();
    noslop.set_admin_hook(Arc::new(move |pkt| {
        let st = hook_state.clone();
        Box::pin(async move { handle_incoming_dm(st, pkt).await })
    })).await;
    // NOSLOP_HUB_DM_VIEW_V1: the owner's DMs the Hub can open go straight to the Web UI's chat (`dms`, the
    // table `sync_push_dms` also fills from the phone; same message ids, so the two never duplicate).
    let dm_pool = app_state.social_db.pool.clone();
    noslop.set_dm_hook(Arc::new(move |rec: hainet_noslop::hub::dms::DmRecord| {
        let pool = dm_pool.clone();
        Box::pin(async move {
            if let Err(e) = sqlx::query("INSERT OR REPLACE INTO dms (id, peer, sender, content, timestamp, media_id, media_type) VALUES (?, ?, ?, ?, ?, ?, ?)")
                .bind(&rec.id).bind(&rec.peer).bind(&rec.sender).bind(&rec.content).bind(rec.timestamp)
                .bind(rec.media_id.as_deref()).bind(rec.media_type.as_deref())
                .execute(&pool).await
            {
                warn!("NOSLOP_HUB: could not store DM {} for the Web UI: {}", rec.id, e);
            }
        })
    })).await;
    // NOSLOP_HUB_DM_DELETE_V1: deletes and cleared conversations leave the Web UI's chat too.
    let rm_pool = app_state.social_db.pool.clone();
    noslop.set_dm_remove_hook(Arc::new(move |what: hainet_noslop::hub::dms::DmRemoval| {
        let pool = rm_pool.clone();
        Box::pin(async move {
            use hainet_noslop::hub::dms::DmRemoval;
            let res = match &what {
                DmRemoval::Message { id, sender } => sqlx::query("DELETE FROM dms WHERE id = ? AND sender = ?")
                    .bind(id).bind(sender).execute(&pool).await,
                DmRemoval::Conversation { peer, before } => sqlx::query("DELETE FROM dms WHERE peer = ? AND timestamp <= ?")
                    .bind(peer).bind(before).execute(&pool).await,
            };
            match res {
                Ok(r) => r.rows_affected(),
                Err(e) => { warn!("NOSLOP_HUB: could not apply {:?} to the Web UI chat: {}", what, e); 0 }
            }
        })
    })).await;
    noslop.spawn_all();
    // Step 4: Start minimal TCP-based health/API endpoint
    let role_for_health = config.network.role.clone();
    let health_app_state = app_state.clone();
    let health_metrics_state = metrics_state.clone();
    let health_metrics_storage = metrics_storage_state.clone();
    let health_settings_state = settings_state.clone();
    let health_qr_sessions = qr_sessions.clone();
    let health_handle = tokio::spawn(async move {
        if let Err(e) = run_health_server(
            health_port, 
            &role_for_health, 
            health_app_state, 
            health_metrics_state, 
            health_metrics_storage, 
            health_settings_state,
            health_qr_sessions
        ).await {
            warn!("⚠  Health endpoint failed: {}", e);
        }
    });

    info!("✅ HAI-Net Core initialized successfully");
    info!("🌐 Portal UI + API: http://0.0.0.0:{}", health_port);
    info!("🩺 Health endpoint: http://0.0.0.0:{}/health", health_port);

    // Step 5: Periodic heartbeat + wait for shutdown signal
    let heartbeat = tokio::spawn(async {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            debug!("💓 hainet-core heartbeat — alive");
        }
    });

    // Wait for shutdown signal
    tokio::signal::ctrl_c().await?;
    info!("🛑 HAI-Net Core shutting down gracefully");

    // Cleanup
    health_handle.abort();
    heartbeat.abort();
    
    Ok(())
}

use serde::{Serialize, Deserialize};
use sha3::{Digest, Sha3_256};
use std::path::PathBuf;
use rand::Rng;
use chrono::Utc;

/// Portal browser sessions: `hainet_session` cookie token -> expiry (epoch ms). Each browser gets its
/// own session (HttpOnly, SameSite=Strict); logging in one browser no longer logs in every client.
pub type Sessions = Arc<tokio::sync::Mutex<std::collections::HashMap<String, i64>>>;
const SESSION_COOKIE: &str = "hainet_session";
/// Session lifetime (`HAINET_SESSION_TTL_HOURS`, default 7 days).
fn session_ttl_ms() -> i64 {
    std::env::var("HAINET_SESSION_TTL_HOURS").ok().and_then(|v| v.parse::<i64>().ok()).unwrap_or(168) * 3_600_000
}
/// Request header block ceiling (`HAINET_HTTP_MAX_HEADER_BYTES`).
fn max_header_bytes() -> usize { std::env::var("HAINET_HTTP_MAX_HEADER_BYTES").ok().and_then(|v| v.parse().ok()).unwrap_or(32 * 1024) }
/// Request body ceiling for everything except backup uploads (`HAINET_HTTP_MAX_BODY_BYTES`).
fn max_body_bytes() -> usize { std::env::var("HAINET_HTTP_MAX_BODY_BYTES").ok().and_then(|v| v.parse().ok()).unwrap_or(16 * 1024 * 1024) }
#[derive(Serialize, Deserialize, Clone)]
struct AuthState {
    setup_complete: bool,
    passphrase_hash: String,
    created_at: String,
}

fn get_auth_file_path() -> PathBuf {
    let base = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/root"));
    let dir = base.join(".hainet");
    let _ = std::fs::create_dir_all(&dir);
    dir.join("auth.json")
}

fn is_setup() -> bool {
    get_auth_file_path().exists()
}

fn hash_passphrase(passphrase: &str) -> String {
    let mut hasher = Sha3_256::new();
    hasher.update(passphrase.as_bytes());
    hex::encode(hasher.finalize())
}

const BIP39_WORDS: &[&str] = &[
    "abandon", "ability", "able", "about", "above", "absent", "absorb", "abstract", "absurd", "abuse", "access", "accident",
    "account", "accuse", "achieve", "acid", "acoustic", "acquire", "across", "act", "action", "actor", "actress", "actual",
    "adapt", "add", "addict", "address", "adjust", "admit", "adult", "advance", "advice", "aerobic", "affair", "afford",
    "afraid", "again", "age", "agent", "agree", "ahead", "aim", "air", "airport", "aisle", "alarm", "album",
    "alcohol", "alert", "alien", "all", "alley", "allow", "almost", "alone", "alpha", "already", "also", "alter",
    "always", "amateur", "amazing", "among", "amount", "amused", "analyst", "anchor", "ancient", "anger", "angle", "angry"
];

fn generate_random_seed() -> String {
    let mut rng = rand::thread_rng();
    let mut words = Vec::new();
    for _ in 0..24 {
        let idx = rng.gen_range(0..BIP39_WORDS.len());
        words.push(BIP39_WORDS[idx]);
    }
    words.join(" ")
}

fn now_ms() -> i64 { Utc::now().timestamp_millis() }

async fn new_session(sessions: &Sessions) -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill(&mut bytes);
    let token = hex::encode(bytes);
    let now = now_ms();
    let mut s = sessions.lock().await;
    s.retain(|_, exp| *exp > now);
    s.insert(token.clone(), now + session_ttl_ms());
    token
}

fn session_cookie(token: &str) -> String {
    format!("Set-Cookie: {}={}; Path=/; HttpOnly; SameSite=Strict; Max-Age={}", SESSION_COOKIE, token, session_ttl_ms() / 1000)
}

fn cookie_token(headers: &std::collections::HashMap<String, String>) -> Option<String> {
    headers.get("cookie")?.split(';').find_map(|c| {
        let (k, v) = c.trim().split_once('=')?;
        (k == SESSION_COOKIE && !v.is_empty()).then(|| v.to_string())
    })
}

async fn session_valid(sessions: &Sessions, headers: &std::collections::HashMap<String, String>) -> bool {
    let Some(t) = cookie_token(headers) else { return false };
    let now = now_ms();
    sessions.lock().await.get(&t).map(|exp| *exp > now).unwrap_or(false)
}

/// One parsed HTTP request. `upload` is set for a streamed backup upload: (temp file, SHA-256 hex, size).
struct HttpRequest {
    method: String,
    path: String,
    headers: std::collections::HashMap<String, String>,
    body: Vec<u8>,
    upload: Option<(PathBuf, String, u64)>,
}

/// Reads one request with bounded header and body sizes. A `POST /api/backup/push` body is streamed
/// to a temporary file (hashed on the way) instead of being held in memory.
async fn read_request(stream: &mut tokio::net::TcpStream, backup_tmp_dir: &std::path::Path, backup_max: u64) -> Result<HttpRequest, (&'static str, String)> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let io_timeout = std::time::Duration::from_secs(30);
    let mut head = Vec::with_capacity(4096);
    let mut buf = vec![0u8; 8192];
    let header_end = loop {
        if let Some(pos) = head.windows(4).position(|w| w == b"\r\n\r\n") {
            if pos > max_header_bytes() { return Err(("431 Request Header Fields Too Large", "headers too large".into())); }
            break pos;
        }
        if head.len() > max_header_bytes() { return Err(("431 Request Header Fields Too Large", "headers too large".into())); }
        let n = tokio::time::timeout(io_timeout, stream.read(&mut buf)).await.map_err(|_| ("408 Request Timeout", "timeout".to_string()))?.map_err(|e| ("400 Bad Request", e.to_string()))?;
        if n == 0 { return Err(("400 Bad Request", "connection closed".into())); }
        head.extend_from_slice(&buf[..n]);
    };
    let mut body = head.split_off(header_end + 4);
    let head_str = String::from_utf8_lossy(&head[..header_end]).into_owned();
    let mut lines = head_str.split("\r\n");
    let request_line = lines.next().unwrap_or("");
    let mut parts = request_line.split_whitespace();
    let (method, path) = match (parts.next(), parts.next()) { (Some(m), Some(p)) => (m.to_string(), p.to_string()), _ => return Err(("400 Bad Request", "bad request line".into())) };
    let mut headers = std::collections::HashMap::new();
    for l in lines {
        if let Some((k, v)) = l.split_once(':') { headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string()); }
    }
    let content_length: u64 = match headers.get("content-length") { Some(v) => v.parse().map_err(|_| ("400 Bad Request", "bad content-length".to_string()))?, None => 0 };

    if method == "POST" && path == "/api/backup/push" {
        if content_length == 0 { return Err(("411 Length Required", "Content-Length required".into())); }
        if content_length > backup_max { return Err(("413 Payload Too Large", format!("backup larger than {} bytes", backup_max))); }
        tokio::fs::create_dir_all(backup_tmp_dir).await.map_err(|e| ("500 Internal Server Error", e.to_string()))?;
        let tmp = backup_tmp_dir.join(format!("upload_{}.tmp", uuid::Uuid::new_v4()));
        let mut f = tokio::fs::File::create(&tmp).await.map_err(|e| ("500 Internal Server Error", e.to_string()))?;
        let mut hasher = sha2::Sha256::new();
        let mut received = 0u64;
        if !body.is_empty() {
            let take = (body.len() as u64).min(content_length) as usize;
            sha2::Digest::update(&mut hasher, &body[..take]);
            f.write_all(&body[..take]).await.map_err(|e| ("500 Internal Server Error", e.to_string()))?;
            received = take as u64;
        }
        while received < content_length {
            let want = ((content_length - received) as usize).min(buf.len());
            let n = match tokio::time::timeout(io_timeout, stream.read(&mut buf[..want])).await { Ok(Ok(n)) => n, _ => 0 };
            if n == 0 { let _ = tokio::fs::remove_file(&tmp).await; return Err(("400 Bad Request", "upload interrupted".into())); }
            sha2::Digest::update(&mut hasher, &buf[..n]);
            if let Err(e) = f.write_all(&buf[..n]).await { let _ = tokio::fs::remove_file(&tmp).await; return Err(("500 Internal Server Error", e.to_string())); }
            received += n as u64;
        }
        f.flush().await.map_err(|e| ("500 Internal Server Error", e.to_string()))?;
        let sha = hex::encode(sha2::Digest::finalize(hasher));
        return Ok(HttpRequest { method, path, headers, body: Vec::new(), upload: Some((tmp, sha, received)) });
    }

    if content_length as usize > max_body_bytes() { return Err(("413 Payload Too Large", "body too large".into())); }
    body.truncate(content_length as usize);
    while body.len() < content_length as usize {
        let n = match tokio::time::timeout(io_timeout, stream.read(&mut buf)).await { Ok(Ok(n)) => n, _ => 0 };
        if n == 0 { return Err(("400 Bad Request", "body incomplete".into())); }
        let want = content_length as usize - body.len();
        body.extend_from_slice(&buf[..n.min(want)]);
    }
    Ok(HttpRequest { method, path, headers, body, upload: None })
}

async fn write_response(stream: &mut tokio::net::TcpStream, status: &str, content_type: &str, extra_headers: &[String], body: &[u8]) {
    use tokio::io::AsyncWriteExt;
    let mut head = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nContent-Length: {}\r\nConnection: close\r\n",
        status, content_type, body.len()
    );
    for h in extra_headers { head.push_str(h); head.push_str("\r\n"); }
    head.push_str("\r\n");
    let _ = stream.write_all(head.as_bytes()).await;
    let _ = stream.write_all(body).await;
    let _ = stream.flush().await;
}

fn json_body(v: &str) -> Vec<u8> { v.as_bytes().to_vec() }

/// Run a minimal TCP-based HTTP server for health checks and portal API.
/// Uses raw TCP to avoid hyper version compatibility issues.
///
/// Authentication: the owner's phone signs each request (`X-NoSlop-Hub-*` headers, see
/// `hainet_noslop::hub::api`); the portal uses a per-browser session cookie set at login. Nothing
/// under `/api/invoke` or `/api/backup` is reachable without one of the two.
async fn run_health_server(
    port: u16, 
    role: &str,
    app_state: Arc<AppState>,
    metrics_state: MetricsState,
    metrics_storage: MetricsStorageState,
    settings_state: SettingsState,
    qr_sessions: Arc<tokio::sync::Mutex<std::collections::HashMap<String, bool>>>
) -> Result<()> {
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await
        .map_err(|e| anyhow::anyhow!("Failed to bind health endpoint on port {}: {}", port, e))?;
    
    debug!("Health/API server listening on port {}", port);

    loop {
        let (mut stream, _addr) = match listener.accept().await {
            Ok(res) => res,
            Err(_) => continue,
        };
        let role = role.to_string();
        let app_state = app_state.clone();
        let metrics_state = metrics_state.clone();
        let metrics_storage = metrics_storage.clone();
        let settings_state = settings_state.clone();
        let qr_sessions = qr_sessions.clone();
        
        tokio::spawn(async move {
            let backup_tmp = app_state.noslop.cfg.backup_dir().join("incoming");
            let req = match read_request(&mut stream, &backup_tmp, app_state.noslop.cfg.backup_max).await {
                Ok(r) => r,
                Err((status, msg)) => {
                    write_response(&mut stream, status, "application/json", &[], &json_body(&serde_json::json!({"error": msg}).to_string())).await;
                    return;
                }
            };
            let method = req.method.as_str();
            let path = req.path.as_str();
            debug!("API Request: {} {}", method, path);
            let body_str = String::from_utf8_lossy(&req.body).into_owned();
            let mut extra_headers: Vec<String> = Vec::new();
            let owner_signed = req.headers.contains_key("x-noslop-hub-sig");

            // --- Backups (owner only) ---------------------------------------------------------
            if path == "/api/backup/push" && method == "POST" {
                let Some((tmp, sha, size)) = req.upload else { return };
                let res = hainet_noslop::hub::api::verify_owner_request_hashed(&app_state.noslop, &app_state.api_nonces, method, path, &req.headers, &sha).await;
                let (status, body) = match res {
                    Err(e) => { let _ = tokio::fs::remove_file(&tmp).await; warn!("Backup push refused: {}", e); ("401 Unauthorized", serde_json::json!({"error": e}).to_string()) }
                    Ok(()) => match hainet_noslop::hub::api::backup_store(&app_state.noslop, &tmp, &sha, size).await {
                        Ok(v) => ("200 OK", v.to_string()),
                        Err(e) => { let _ = tokio::fs::remove_file(&tmp).await; ("500 Internal Server Error", serde_json::json!({"error": e}).to_string()) }
                    },
                };
                write_response(&mut stream, status, "application/json", &[], &json_body(&body)).await;
                return;
            }
            if (path == "/api/backup/pull" || path == "/api/backup/list") && method == "GET" {
                if let Err(e) = hainet_noslop::hub::api::verify_owner_request(&app_state.noslop, &app_state.api_nonces, method, path, &req.headers, &req.body).await {
                    write_response(&mut stream, "401 Unauthorized", "application/json", &[], &json_body(&serde_json::json!({"error": e}).to_string())).await;
                    return;
                }
                if path == "/api/backup/list" {
                    let names = hainet_noslop::hub::api::backup_list(&app_state.noslop).await;
                    write_response(&mut stream, "200 OK", "application/json", &[], &json_body(&serde_json::json!({"status": "ok", "backups": names}).to_string())).await;
                    return;
                }
                match hainet_noslop::hub::api::backup_latest(&app_state.noslop).await {
                    None => write_response(&mut stream, "404 Not Found", "application/json", &[], br#"{"error":"no backup on this Hub"}"#).await,
                    Some(file) => {
                        use tokio::io::AsyncWriteExt;
                        let sha = tokio::fs::read_to_string(file.with_extension("sha256")).await.unwrap_or_default();
                        match tokio::fs::File::open(&file).await {
                            Ok(mut f) => {
                                let len = f.metadata().await.map(|m| m.len()).unwrap_or(0);
                                let head = format!("HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nX-Backup-Sha256: {}\r\nConnection: close\r\n\r\n", len, sha.trim());
                                if stream.write_all(head.as_bytes()).await.is_ok() { let _ = tokio::io::copy(&mut f, &mut stream).await; }
                                let _ = stream.flush().await;
                            }
                            Err(e) => write_response(&mut stream, "500 Internal Server Error", "application/json", &[], &json_body(&serde_json::json!({"error": e.to_string()}).to_string())).await,
                        }
                    }
                }
                return;
            }

            let (status, body): (String, Vec<u8>) = match (method, path) {
                ("GET", "/health") => (
                    "200 OK".to_string(),
                    json_body(&format!(r#"{{"status":"ok","service":"hainet-core","role":"{}","version":"{}","noslop_api":{}}}"#, role, env!("CARGO_PKG_VERSION"), hainet_noslop::hub::api::API_PROTOCOL))
                ),
                ("GET", "/api/auth/verify") => {
                    if session_valid(&app_state.sessions, &req.headers).await {
                        ("200 OK".to_string(), json_body(r#"{"status":"ok"}"#))
                    } else {
                        ("401 Unauthorized".to_string(), json_body(r#"{"error":"unauthorized"}"#))
                    }
                },
                ("GET", "/api/auth/status") => {
                    let has_identity = get_hainet_dir().join("identity/ed25519_pub.b64").exists();
                    if is_setup() {
                        ("200 OK".to_string(), json_body(r#"{"status":"login_required"}"#))
                    } else if has_identity {
                        ("200 OK".to_string(), json_body(r#"{"status":"qr_login_only"}"#))
                    } else {
                        ("200 OK".to_string(), json_body(r#"{"status":"setup_required"}"#))
                    }
                },
                ("POST", "/api/auth/qr/init") => {
                    let session_id = uuid::Uuid::new_v4().to_string();
                    let mut sessions = qr_sessions.lock().await;
                    sessions.insert(session_id.clone(), false);
                    ("200 OK".to_string(), json_body(&format!(r#"{{"session_id":"{}"}}"#, session_id)))
                },
                ("POST", "/api/auth/qr/verify") => {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body_str) {
                        let session_id = json.get("session_id").and_then(|v| v.as_str()).unwrap_or("");
                        let public_key = json.get("public_key").and_then(|v| v.as_str()).unwrap_or("");
                        let signature = json.get("signature").and_then(|v| v.as_str()).unwrap_or("");
                        
                        if verify_qr_signature(session_id, public_key, signature) {
                            let mut sessions = qr_sessions.lock().await;
                            if sessions.contains_key(session_id) {
                                sessions.insert(session_id.to_string(), true);
                                ("200 OK".to_string(), json_body(r#"{"status":"verified"}"#))
                            } else {
                                ("404 Not Found".to_string(), json_body(r#"{"error":"Session not found"}"#))
                            }
                        } else {
                            ("401 Unauthorized".to_string(), json_body(r#"{"error":"Invalid signature"}"#))
                        }
                    } else {
                        ("400 Bad Request".to_string(), json_body(r#"{"error":"invalid_json"}"#))
                    }
                },
                ("GET", p) if p.starts_with("/api/auth/qr/status/") => {
                    let session_id = p.trim_start_matches("/api/auth/qr/status/");
                    let is_verified = {
                        let sessions = qr_sessions.lock().await;
                        sessions.get(session_id).copied().unwrap_or(false)
                    };
                    
                    if is_verified {
                        // The browser that showed this QR code (and only it) is now logged in.
                        qr_sessions.lock().await.remove(session_id);
                        let token = new_session(&app_state.sessions).await;
                        extra_headers.push(session_cookie(&token));
                        ("200 OK".to_string(), json_body(r#"{"status":"authenticated"}"#))
                    } else {
                        ("200 OK".to_string(), json_body(r#"{"status":"pending"}"#))
                    }
                },
                ("GET", "/api/auth/generate-seed") => (
                    "200 OK".to_string(),
                    json_body(&format!(r#"{{"seed_phrase":"{}"}}"#, generate_random_seed()))
                ),
                ("POST", "/api/auth/setup") => {
                    if is_setup() {
                        ("400 Bad Request".to_string(), json_body(r#"{"error":"already_setup"}"#))
                    } else if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body_str) {
                        if let Some(passphrase) = json.get("app_passphrase").and_then(|v| v.as_str()) {
                            let auth_state = AuthState {
                                setup_complete: true,
                                passphrase_hash: hash_passphrase(passphrase),
                                created_at: Utc::now().to_rfc3339(),
                            };
                            
                            match std::fs::write(get_auth_file_path(), serde_json::to_string_pretty(&auth_state).unwrap_or_default()) {
                                Ok(_) => {
                                    info!("✅ Setup complete: auth file saved.");
                                    let token = new_session(&app_state.sessions).await;
                                    extra_headers.push(session_cookie(&token));
                                    ("200 OK".to_string(), json_body(r#"{"status":"ok"}"#))
                                },
                                Err(e) => {
                                    warn!("Failed to save auth file: {}", e);
                                    ("500 Internal Server Error".to_string(), json_body(r#"{"error":"disk_error"}"#))
                                }
                            }
                        } else {
                            ("400 Bad Request".to_string(), json_body(r#"{"error":"missing_passphrase"}"#))
                        }
                    } else {
                        ("400 Bad Request".to_string(), json_body(r#"{"error":"invalid_json"}"#))
                    }
                },
                ("POST", "/api/auth/login") => {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body_str) {
                        if let Some(passphrase) = json.get("app_passphrase").and_then(|v| v.as_str()) {
                            if let Ok(auth_content) = std::fs::read_to_string(get_auth_file_path()) {
                                if let Ok(auth_state) = serde_json::from_str::<AuthState>(&auth_content) {
                                    if auth_state.passphrase_hash == hash_passphrase(passphrase) {
                                        info!("✅ Login successful.");
                                        let token = new_session(&app_state.sessions).await;
                                        extra_headers.push(session_cookie(&token));
                                        ("200 OK".to_string(), json_body(r#"{"status":"ok"}"#))
                                    } else {
                                        warn!("❌ Login failed: incorrect passphrase.");
                                        ("401 Unauthorized".to_string(), json_body(r#"{"error":"invalid_credentials"}"#))
                                    }
                                } else {
                                    ("500 Internal Server Error".to_string(), json_body(r#"{"error":"corrupted_auth_file"}"#))
                                }
                            } else {
                                ("400 Bad Request".to_string(), json_body(r#"{"error":"not_setup"}"#))
                            }
                        } else {
                            ("400 Bad Request".to_string(), json_body(r#"{"error":"missing_passphrase"}"#))
                        }
                    } else {
                        ("400 Bad Request".to_string(), json_body(r#"{"error":"invalid_json"}"#))
                    }
                },
                ("POST", "/api/auth/logout") => {
                    if let Some(t) = cookie_token(&req.headers) { app_state.sessions.lock().await.remove(&t); }
                    extra_headers.push(format!("Set-Cookie: {}=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0", SESSION_COOKIE));
                    ("200 OK".to_string(), json_body(r#"{"status":"ok"}"#))
                },
                ("POST", "/api/invoke") => {
                    // The phone signs every request; the portal sends its session cookie. A request with
                    // a signature that does not verify is refused even if it also carries a cookie.
                    let auth = if owner_signed {
                        hainet_noslop::hub::api::verify_owner_request(&app_state.noslop, &app_state.api_nonces, method, path, &req.headers, &req.body).await
                    } else if session_valid(&app_state.sessions, &req.headers).await {
                        Ok(())
                    } else {
                        Err("unauthorized".to_string())
                    };
                    if let Err(e) = auth {
                        warn!("API: refused /api/invoke: {}", e);
                        ("401 Unauthorized".to_string(), json_body(&serde_json::json!({"error": e}).to_string()))
                    } else if let Ok(jv) = hainet_noslop::json::parse(&body_str) {
                        // Hub commands get the request in its wire order (packets are passed on as written).
                        let cmd = jv.get_str("cmd").unwrap_or("").to_string();
                        let empty = hainet_noslop::json::JValue::obj();
                        let jargs = jv.get("args").filter(|a| !a.is_null()).unwrap_or(&empty);
                        // The Web UI feed keeps the posts the phone sends out (as it always did).
                        if cmd == "hub_send" || cmd == "sync_push_packets" {
                            let sv = jargs.to_serde();
                            let packets: Vec<serde_json::Value> = if cmd == "hub_send" {
                                sv.get("items").and_then(|i| i.as_array()).map(|a| a.iter().filter_map(|it| it.get("packet").cloned()).collect()).unwrap_or_default()
                            } else {
                                sv.get("packets").and_then(|p| p.as_array()).cloned().unwrap_or_default()
                            };
                            let posts: Vec<serde_json::Value> = packets.iter()
                                .filter(|p| p.get("type").and_then(|t| t.as_str()) == Some("POST"))
                                .filter_map(|p| p.get("payload").cloned()).collect();
                            if !posts.is_empty() { api_router::store_portal_posts(&app_state, posts.iter()).await; }
                        }
                        match hainet_noslop::hub::api::handle(&app_state.noslop, &cmd, jargs).await {
                            Some(Ok(res)) => ("200 OK".to_string(), json_body(&res.to_string())),
                            Some(Err(e)) => {
                                warn!("API Invoke Error [{}]: {}", cmd, e);
                                ("500 Internal Server Error".to_string(), json_body(&serde_json::json!({"error": e}).to_string()))
                            }
                            None if cmd.is_empty() => ("400 Bad Request".to_string(), json_body(r#"{"error":"missing_cmd"}"#)),
                            None => {
                                let args = jargs.to_serde();
                                match api_router::handle_invoke(
                                    &cmd,
                                    args, 
                                    app_state.clone(), 
                                    &metrics_state, 
                                    &metrics_storage, 
                                    &settings_state
                                ).await {
                                    Ok(res) => ("200 OK".to_string(), json_body(&serde_json::to_string(&res).unwrap_or_default())),
                                    Err(e) => {
                                        warn!("API Invoke Error [{}]: {}", cmd, e);
                                        ("500 Internal Server Error".to_string(), json_body(&serde_json::to_string(&serde_json::json!({"error": e})).unwrap_or_default()))
                                    }
                                }
                            }
                        }
                    } else {
                        ("400 Bad Request".to_string(), json_body(r#"{"error":"invalid_json"}"#))
                    }
                },
                ("OPTIONS", _) => {
                    // Handle CORS preflight
                    ("204 No Content".to_string(), Vec::new())
                },
                // Block API routes from falling through to the SPA fallback
                ("GET", path) if path.starts_with("/api/") => {
                    warn!("API: Not Found: {} {}", method, path);
                    (
                        "404 Not Found".to_string(),
                        json_body(r#"{"error":"not_found"}"#)
                    )
                },
                // --- Static file serving: Portal UI assets ---
                // Any GET that doesn't match an API route serves the React app.
                ("GET", static_path) => {
                    serve_static_asset(static_path)
                },
                _ => {
                    warn!("API: Not Found: {} {}", method, path);
                    (
                        "404 Not Found".to_string(),
                        json_body(r#"{"error":"not_found"}"#)
                    )
                }
            };

            debug!("API Response to {} {}: {}", method, path, status);

            // Determine content type from response — JSON for API, mime type for static
            let content_type = if status.contains("STATIC:") {
                // Extract the mime type from our marker (see serve_static_asset)
                status.split("STATIC:").nth(1).unwrap_or("application/octet-stream").to_string()
            } else {
                "application/json".to_string()
            };

            // Clean the status line for the HTTP response
            let clean_status = if status.contains("STATIC:") {
                "200 OK"
            } else {
                &status
            };

            write_response(&mut stream, clean_status, &content_type, &extra_headers, &body).await;
        });
    }
}

/// Serve a static file from the embedded Portal UI assets (as bytes: images and fonts are binary).
/// Falls back to index.html for SPA routing (React Router).
fn serve_static_asset(path: &str) -> (String, Vec<u8>) {
    let mut asset_path = path.trim_start_matches('/').split('?').next().unwrap_or("").to_string();
    
    // Root path → index.html
    if asset_path.is_empty() {
        asset_path = "index.html".to_string();
    }

    if asset_path.starts_with("api/") {
        warn!("API route not found, preventing SPA fallback: /{}", asset_path);
        return (
            "404 Not Found".to_string(),
            json_body(r#"{"error":"not_found"}"#)
        );
    }

    debug!("Static asset request: {}", asset_path);

    match PortalAssets::get(&asset_path) {
        Some(content) => {
            let mime = mime_guess::from_path(&asset_path)
                .first_or_octet_stream()
                .to_string();
            // Use a "STATIC:" prefix in the status to signal the content type to the writer
            (format!("STATIC:{}", mime), content.data.to_vec())
        }
        None => {
            // SPA fallback: return index.html for unknown routes (React Router)
            debug!("Asset '{}' not found, serving index.html (SPA fallback)", asset_path);
            match PortalAssets::get("index.html") {
                Some(index) => {
                    ("STATIC:text/html".to_string(), index.data.to_vec())
                }
                None => {
                    warn!("Portal UI not built! Run 'cd hainet-portal && npm run build'");
                    (
                        "404 Not Found".to_string(),
                        json_body(r#"{"error":"Portal UI not built. Run: cd hainet-portal && npm run build"}"#)
                    )
                }
            }
        }
    }
}

fn verify_qr_signature(session_id: &str, public_key_b64: &str, signature_b64: &str) -> bool {
    use base64::{Engine as _, engine::general_purpose::STANDARD as b64};
    use ed25519_dalek::{VerifyingKey, Signature, Verifier};
    
    let owner_pub = std::fs::read_to_string(get_hainet_dir().join("identity/ed25519_pub.b64")).unwrap_or_default();
    if owner_pub.trim() != public_key_b64.trim() {
        return false;
    }
    
    let pub_bytes = match b64.decode(public_key_b64) {
        Ok(b) => b,
        Err(_) => return false,
    };
    
    let raw_pub = if pub_bytes.len() == 44 {
        &pub_bytes[12..44]
    } else if pub_bytes.len() == 32 {
        &pub_bytes[..]
    } else {
        return false;
    };
    
    let sig_bytes = match b64.decode(signature_b64) {
        Ok(b) => b,
        Err(_) => return false,
    };
    
    if sig_bytes.len() != 64 {
        return false;
    }
    
    let public_key = match VerifyingKey::try_from(raw_pub) {
        Ok(k) => k,
        Err(_) => return false,
    };
    
    let signature = match Signature::try_from(sig_bytes.as_slice()) {
        Ok(s) => s,
        Err(_) => return false,
    };
    
    public_key.verify(session_id.as_bytes(), &signature).is_ok()
}

fn get_hainet_dir() -> std::path::PathBuf {
    dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/root")).join(".hainet")
}


pub async fn decrypt_and_store_dm(app_state: Arc<AppState>, packet_json: serde_json::Value) {
    tracing::info!("Intercepted DM for owner! Hub is decrypting and storing it...");
    use base64::{Engine as _, engine::general_purpose::STANDARD as b64};
    use x25519_dalek::{StaticSecret, PublicKey};
    use hainet_social::crypto::decrypt_from_sender;

    let payload = match packet_json.get("payload") {
        Some(p) => p,
        None => { tracing::error!("DM Decrypt FATAL: Packet missing payload!"); return; },
    };
    
    let ciphertext_b64 = payload.get("ciphertext").and_then(|v| v.as_str()).unwrap_or_default();
    let nonce_b64 = payload.get("nonce").and_then(|v| v.as_str()).unwrap_or_default();
    
    let ident_dir = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/root")).join(".hainet/identity");
    let priv_key_path = ident_dir.join("x25519_priv.b64");
    if !priv_key_path.exists() {
        tracing::error!("DM Decrypt FATAL: x25519_priv.b64 is MISSING!");
        return;
    }
    let priv_key_str = std::fs::read_to_string(priv_key_path).unwrap_or_default().replace("\n", "").replace("\r", "");
    
    let priv_bytes = match b64.decode(priv_key_str.trim()) {
        Ok(b) => b,
        Err(e) => {
            tracing::error!("DM Decrypt: Failed to decode private key: {}", e);
            return;
        }
    };
    
    let secret_bytes = if priv_bytes.len() == 32 {
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&priv_bytes);
        arr
    } else {
        let mut found = None;
        for i in 0..priv_bytes.len().saturating_sub(33) {
            if priv_bytes[i] == 0x04 && priv_bytes[i+1] == 0x20 {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&priv_bytes[i+2..i+34]);
                found = Some(arr);
                break;
            }
        }
        if let Some(arr) = found {
            arr
        } else {
            tracing::error!("DM Decrypt FATAL: Invalid x25519_priv length: {}", priv_bytes.len());
            return;
        }
    };
    
    let recipient_secret = StaticSecret::from(secret_bytes);
    
    let sender_id = packet_json.get("sender_id").or_else(|| packet_json.get("senderId")).and_then(|v| v.as_str()).unwrap_or_default().to_string();
    
    // Look up X25519 public key from mesh_peers (or fallback to Ed25519 key)
    let sender_enc_pub = match sqlx::query_scalar::<_, Option<String>>("SELECT enc_public_key FROM mesh_peers WHERE public_key = ?")
        .bind(&sender_id)
        .fetch_one(&app_state.social_db.pool).await 
    {
        Ok(Some(key)) if !key.trim().is_empty() => key,
        _ => sender_id.clone() 
    };

    let pub_bytes = match b64.decode(sender_enc_pub.trim()) {
        Ok(b) => b,
        Err(e) => {
            tracing::error!("DM Decrypt: Failed to decode sender public key: {}", e);
            return;
        }
    };
    
    let sender_raw = if pub_bytes.len() == 32 {
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&pub_bytes);
        arr
    } else {
        let mut found = None;
        for i in 0..pub_bytes.len().saturating_sub(33) {
            if pub_bytes[i] == 0x03 && pub_bytes[i+1] == 0x21 && pub_bytes[i+2] == 0x00 {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&pub_bytes[i+3..i+35]);
                found = Some(arr);
                break;
            }
        }
        if let Some(arr) = found {
            arr
        } else {
            tracing::error!("DM Decrypt FATAL: Invalid x25519_pub length: {}", pub_bytes.len());
            return;
        }
    };
    let sender_public = PublicKey::from(sender_raw);
    
    let ciphertext = b64.decode(ciphertext_b64).unwrap_or_default();
    let nonce_vec = match b64.decode(nonce_b64) {
        Ok(b) => b,
        Err(e) => {
            tracing::error!("DM Decrypt: Failed to decode nonce: {}", e);
            return;
        }
    };
    if nonce_vec.len() != 12 {
        tracing::error!("DM Decrypt: Invalid nonce length: {}", nonce_vec.len());
        return;
    }
    let mut nonce_bytes = [0u8; 12];
    nonce_bytes.copy_from_slice(&nonce_vec);
    
    let decrypt_result = decrypt_from_sender(&ciphertext, &nonce_bytes, &recipient_secret, &sender_public);
    if let Ok(decrypted) = decrypt_result {
        tracing::info!("DM Decrypted successfully! Storing in SQLite for mobile sync...");
        if let Ok(plaintext) = String::from_utf8(decrypted) {
            let id = payload.get("id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let timestamp = payload.get("timestamp").and_then(|v| v.as_i64()).unwrap_or_else(|| chrono::Utc::now().timestamp_millis());
            
            let mut content = plaintext.clone();
            let mut media_id = None;
            let mut media_type = None;

            if let Ok(json_obj) = serde_json::from_str::<serde_json::Value>(&plaintext) {
                if let Some(c) = json_obj.get("content").and_then(|v| v.as_str()) {
                    content = c.to_string();
                }
                if let Some(media) = json_obj.get("media") {
                    media_id = media.get("id").and_then(|v| v.as_str()).map(|s| s.to_string());
                    media_type = media.get("type").and_then(|v| v.as_str()).map(|s| s.to_string());
                }
            }

            let _ = sqlx::query("INSERT OR IGNORE INTO dms (id, peer, sender, content, timestamp, media_id, media_type) VALUES (?, ?, ?, ?, ?, ?, ?)")
                .bind(&id).bind(&sender_id).bind(&sender_id).bind(&content).bind(timestamp).bind(&media_id).bind(&media_type)
                .execute(&app_state.social_db.pool).await;
        }
    } else {
        tracing::error!("DM Decryption failed.");
    }
}
