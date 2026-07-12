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
pub struct AppState {
    /// Buffered mesh packets for mobile to pull
    pub incoming_mesh_packets: Arc<RwLock<Vec<serde_json::Value>>>,
    /// Bridge to the Admin AI agent system
    pub admin_bridge: Option<Arc<RwLock<AdminBridge>>>,
    /// Text-to-speech handler
    pub tts_handler: Arc<RwLock<TTSHandler>>,
    /// Live hardware profile from hainet-collab (PPLPWR port)
    pub hardware_profile: Arc<RwLock<HardwareProfile>>,
    /// Gossip engine from hainet-social (gChat port)
    pub gossip_engine: Arc<RwLock<GossipEngine>>,
    /// In-memory social feed posts (bridged to gossip later)
    pub social_posts: Arc<RwLock<Vec<SocialPost>>>,
    /// Synchronized peers from mobile
    pub mesh_peers: Arc<RwLock<Vec<serde_json::Value>>>,
    /// Configured log directory
    pub log_dir: std::path::PathBuf,
}

/// A social feed post (temporary in-memory struct until full gossip integration)
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct SocialPost {
    pub id: String,
    pub author: String,
    pub content: String,
    pub timestamp: String,
}

pub type MetricsState = Arc<RwLock<MetricsCollector>>;
pub type MetricsStorageState = Arc<RwLock<MetricsStorage>>;
pub type SettingsState = Arc<RwLock<SettingsStorage>>;

async fn handle_incoming_dm(app_state: Arc<AppState>, packet_json: serde_json::Value) {
    use base64::{Engine as _, engine::general_purpose::STANDARD as b64};
    use x25519_dalek::{StaticSecret, PublicKey};
    use hainet_social::crypto::{decrypt_from_sender, encrypt_for_recipient};
    use serde_json::json;

    let payload = match packet_json.get("payload") {
        Some(p) => p,
        None => return,
    };
    
    let ciphertext_b64 = payload.get("ciphertext").and_then(|v| v.as_str()).unwrap_or_default();
    let nonce_b64 = payload.get("nonce").and_then(|v| v.as_str()).unwrap_or_default();
    
    let ident_dir = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/root")).join(".hainet/identity");
    let priv_key_str = std::fs::read_to_string(ident_dir.join("x25519_priv.b64")).unwrap_or_default();
    
    let priv_bytes = match b64.decode(priv_key_str.trim()) {
        Ok(b) => b,
        Err(_) => return,
    };
    
    let secret_bytes = if priv_bytes.len() == 48 {
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&priv_bytes[16..48]);
        arr
    } else if priv_bytes.len() == 32 {
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&priv_bytes);
        arr
    } else {
        return;
    };
    
    let recipient_secret = StaticSecret::from(secret_bytes);
    
    let pub_key_str = std::fs::read_to_string(ident_dir.join("x25519_pub.b64")).unwrap_or_default();
    let pub_bytes = match b64.decode(pub_key_str.trim()) {
        Ok(b) => b,
        Err(_) => return,
    };
    
    let sender_raw = if pub_bytes.len() == 44 {
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&pub_bytes[12..44]);
        arr
    } else if pub_bytes.len() == 32 {
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&pub_bytes);
        arr
    } else {
        return;
    };
    let sender_public = PublicKey::from(sender_raw);
    
    let ciphertext = b64.decode(ciphertext_b64).unwrap_or_default();
    let nonce_vec = b64.decode(nonce_b64).unwrap_or_default();
    if nonce_vec.len() != 12 { return; }
    let mut nonce_bytes = [0u8; 12];
    nonce_bytes.copy_from_slice(&nonce_vec);
    
    if let Ok(decrypted) = decrypt_from_sender(&ciphertext, &nonce_bytes, &recipient_secret, &sender_public) {
        if let Ok(plaintext) = String::from_utf8(decrypted) {
            let mut message_content = plaintext.clone();
            if let Ok(json_obj) = serde_json::from_str::<serde_json::Value>(&plaintext) {
                if let Some(content) = json_obj.get("content").and_then(|v| v.as_str()) {
                    message_content = content.to_string();
                }
            }
            
            if let Some(bridge_arc) = &app_state.admin_bridge {
                let bridge = bridge_arc.read().await;
                if let Ok(response) = bridge.send_message(message_content, vec![]).await {
                    let response_json = json!({"content": response.message.content}).to_string();
                    if let Ok((resp_ciphertext, resp_nonce)) = encrypt_for_recipient(response_json.as_bytes(), &recipient_secret, &sender_public) {
                        
                        let msg_id = uuid::Uuid::new_v4().to_string();
                        let my_node_id = std::fs::read_to_string(ident_dir.join("ed25519_pub.b64")).unwrap_or_default().trim().to_string();
                        
                        let reply_packet = json!({
                            "id": uuid::Uuid::new_v4().to_string(),
                            "hops": 1,
                            "sender_id": my_node_id,
                            "target_user_id": my_node_id,
                            "type": "MESSAGE",
                            "payload": {
                                "id": msg_id,
                                "nonce": b64.encode(resp_nonce),
                                "ciphertext": b64.encode(resp_ciphertext),
                                "timestamp": chrono::Utc::now().timestamp_millis()
                            }
                        });
                        
                        let mut buffer = app_state.incoming_mesh_packets.write().await;
                        buffer.push(reply_packet);
                    }
                }
            }
        }
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
    
    // Determine prompts directory (sibling to data_dir)
    let prompts_dir = data_dir.parent().unwrap_or(&data_dir).join("prompts");
    
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
        Some(AdminBridge::new(data_dir.clone(), prompts_dir, config.network.role.clone(), max_ctx).await
            .expect("Failed to initialize Admin AI Bridge"))
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
    
    let app_state = Arc::new(AppState {
        admin_bridge: admin_bridge.map(|b| Arc::new(RwLock::new(b))),
        tts_handler: Arc::new(RwLock::new(tts_handler)),
        hardware_profile: Arc::new(RwLock::new(hardware_profile)),
        gossip_engine: Arc::new(RwLock::new(gossip_engine)),
        social_posts: Arc::new(RwLock::new(vec![])),
        mesh_peers: Arc::new(RwLock::new(vec![])),
        incoming_mesh_packets: Arc::new(RwLock::new(vec![])),
        log_dir: config.effective_log_dir(),
    });

    // Step 3.5: Start Smart Mesh Gossip Listener on Port 9999
    let mesh_app_state = app_state.clone();
    tokio::spawn(async move {
        match tokio::net::TcpListener::bind("0.0.0.0:9999").await {
            Ok(listener) => {
                info!("🕸️  Smart Mesh gossip listener running on port 9999");
                loop {
                    if let Ok((mut stream, _)) = listener.accept().await {
                        let state = mesh_app_state.clone();
                        tokio::spawn(async move {
                            use tokio::io::{AsyncBufReadExt, BufReader};
                            let mut reader = BufReader::new(stream);
                            let mut line = String::new();
                            while let Ok(n) = reader.read_line(&mut line).await {
                                if n == 0 { break; }
                                if let Ok(packet_json) = serde_json::from_str::<serde_json::Value>(&line) {
                                    let ptype = packet_json.get("type").and_then(|v| v.as_str()).unwrap_or_default();
                                    let target_id = packet_json.get("target_user_id").and_then(|v| v.as_str()).unwrap_or_default();
                                    let sender_id = packet_json.get("sender_id").and_then(|v| v.as_str()).unwrap_or_default();
                                    let ident_dir = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/root")).join(".hainet/identity");
                                    let my_node_id = std::fs::read_to_string(ident_dir.join("ed25519_pub.b64")).unwrap_or_default().trim().to_string();
                                    
                                    // Intercept DMs sent to ourselves (AI Persona Chat)
                                    if ptype == "MESSAGE" && target_id == my_node_id && sender_id == my_node_id {
                                        let state_clone = state.clone();
                                        let pjson_clone = packet_json.clone();
                                        tokio::spawn(async move {
                                            handle_incoming_dm(state_clone, pjson_clone).await;
                                        });
                                    }

                                    // Parse into native Rust NetworkPacket
                                    if let Ok(packet) = serde_json::from_value::<hainet_social::packets::NetworkPacket>(packet_json.clone()) {
                                        let engine = state.gossip_engine.read().await;
                                        // The Hub's native firewall rejects untrusted/spam packets!
                                        if let Ok(_) = engine.process_incoming(&packet).await {
                                            debug!("Hub Firewall passed packet from: {}", packet.header.sender_id);
                                            
                                            let mut buffer = state.incoming_mesh_packets.write().await;
                                            buffer.push(packet_json);
                                        } else {
                                            warn!("Hub Firewall rejected packet from: {}", packet.header.sender_id);
                                        }
                                    }
                                }
                                line.clear();
                            }
                        });
                    }
                }
            },
            Err(e) => warn!("Failed to bind mesh port 9999: {}", e),
        }
    });

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

use std::sync::atomic::{AtomicBool, Ordering};
use serde::{Serialize, Deserialize};
use sha3::{Digest, Sha3_256};
use std::path::PathBuf;
use rand::Rng;
use chrono::Utc;

static IS_LOGGED_IN: AtomicBool = AtomicBool::new(false);

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

/// Run a minimal TCP-based HTTP server for health checks and portal API.
/// Uses raw TCP to avoid hyper version compatibility issues.
async fn run_health_server(
    port: u16, 
    role: &str,
    app_state: Arc<AppState>,
    metrics_state: MetricsState,
    metrics_storage: MetricsStorageState,
    settings_state: SettingsState,
    qr_sessions: Arc<tokio::sync::Mutex<std::collections::HashMap<String, bool>>>
) -> Result<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

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
            let mut buf = [0u8; 8192];
            let n = stream.read(&mut buf).await.unwrap_or(0);
            if n == 0 { return; }
            
            let request = String::from_utf8_lossy(&buf[..n]);
            
            // Parse headers and body
            let body_str = if let Some(idx) = request.find("\r\n\r\n") {
                &request[idx + 4..]
            } else {
                ""
            };

            let first_line = request.lines().next().unwrap_or("");
            let parts: Vec<&str> = first_line.split_whitespace().collect();
            
            if parts.len() < 2 { return; }
            let method = parts[0];
            let path = parts[1];
            
            debug!("API Request: {} {}", method, path);
            
            let (status, body) = match (method, path) {
                ("GET", "/health") => (
                    "200 OK".to_string(),
                    format!(r#"{{"status":"ok","service":"hainet-core","role":"{}","version":"{}"}}"#, role, env!("CARGO_PKG_VERSION"))
                ),
                ("GET", "/api/auth/verify") => {
                    if IS_LOGGED_IN.load(Ordering::SeqCst) {
                        ("200 OK".to_string(), r#"{"status":"ok"}"#.to_string())
                    } else {
                        ("401 Unauthorized".to_string(), r#"{"error":"unauthorized"}"#.to_string())
                    }
                },
                ("GET", "/api/auth/status") => {
                    let has_identity = get_hainet_dir().join("identity/ed25519_pub.b64").exists();
                    if is_setup() {
                        ("200 OK".to_string(), r#"{"status":"login_required"}"#.to_string())
                    } else if has_identity {
                        ("200 OK".to_string(), r#"{"status":"qr_login_only"}"#.to_string())
                    } else {
                        ("200 OK".to_string(), r#"{"status":"setup_required"}"#.to_string())
                    }
                },
                ("POST", "/api/auth/qr/init") => {
                    let session_id = uuid::Uuid::new_v4().to_string();
                    let mut sessions = qr_sessions.lock().await;
                    sessions.insert(session_id.clone(), false);
                    ("200 OK".to_string(), format!(r#"{{"session_id":"{}"}}"#, session_id))
                },
                ("POST", "/api/auth/qr/verify") => {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body_str) {
                        let session_id = json.get("session_id").and_then(|v| v.as_str()).unwrap_or("");
                        let public_key = json.get("public_key").and_then(|v| v.as_str()).unwrap_or("");
                        let signature = json.get("signature").and_then(|v| v.as_str()).unwrap_or("");
                        
                        if verify_qr_signature(session_id, public_key, signature) {
                            let mut sessions = qr_sessions.lock().await;
                            if sessions.contains_key(session_id) {
                                sessions.insert(session_id.to_string(), true);
                                ("200 OK".to_string(), r#"{"status":"verified"}"#.to_string())
                            } else {
                                ("404 Not Found".to_string(), r#"{"error":"Session not found"}"#.to_string())
                            }
                        } else {
                            ("401 Unauthorized".to_string(), r#"{"error":"Invalid signature"}"#.to_string())
                        }
                    } else {
                        ("400 Bad Request".to_string(), r#"{"error":"invalid_json"}"#.to_string())
                    }
                },
                ("GET", p) if p.starts_with("/api/auth/qr/status/") => {
                    let session_id = p.trim_start_matches("/api/auth/qr/status/");
                    let is_verified = {
                        let sessions = qr_sessions.lock().await;
                        sessions.get(session_id).copied().unwrap_or(false)
                    };
                    
                    if is_verified {
                        IS_LOGGED_IN.store(true, Ordering::SeqCst);
                        qr_sessions.lock().await.remove(session_id);
                        ("200 OK".to_string(), r#"{"status":"authenticated"}"#.to_string())
                    } else {
                        ("200 OK".to_string(), r#"{"status":"pending"}"#.to_string())
                    }
                },
                ("GET", "/api/auth/generate-seed") => (
                    "200 OK".to_string(),
                    format!(r#"{{"seed_phrase":"{}"}}"#, generate_random_seed())
                ),
                ("POST", "/api/auth/setup") => {
                    if is_setup() {
                        ("400 Bad Request".to_string(), r#"{"error":"already_setup"}"#.to_string())
                    } else if let Ok(json) = serde_json::from_str::<serde_json::Value>(body_str) {
                        if let Some(passphrase) = json.get("app_passphrase").and_then(|v| v.as_str()) {
                            let auth_state = AuthState {
                                setup_complete: true,
                                passphrase_hash: hash_passphrase(passphrase),
                                created_at: Utc::now().to_rfc3339(),
                            };
                            
                            match std::fs::write(get_auth_file_path(), serde_json::to_string_pretty(&auth_state).unwrap_or_default()) {
                                Ok(_) => {
                                    info!("✅ Setup complete: auth file saved.");
                                    IS_LOGGED_IN.store(true, Ordering::SeqCst);
                                    ("200 OK".to_string(), r#"{"status":"ok"}"#.to_string())
                                },
                                Err(e) => {
                                    warn!("Failed to save auth file: {}", e);
                                    ("500 Internal Server Error".to_string(), r#"{"error":"disk_error"}"#.to_string())
                                }
                            }
                        } else {
                            ("400 Bad Request".to_string(), r#"{"error":"missing_passphrase"}"#.to_string())
                        }
                    } else {
                        ("400 Bad Request".to_string(), r#"{"error":"invalid_json"}"#.to_string())
                    }
                },
                ("POST", "/api/auth/login") => {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body_str) {
                        if let Some(passphrase) = json.get("app_passphrase").and_then(|v| v.as_str()) {
                            if let Ok(auth_content) = std::fs::read_to_string(get_auth_file_path()) {
                                if let Ok(auth_state) = serde_json::from_str::<AuthState>(&auth_content) {
                                    if auth_state.passphrase_hash == hash_passphrase(passphrase) {
                                        info!("✅ Login successful.");
                                        IS_LOGGED_IN.store(true, Ordering::SeqCst);
                                        ("200 OK".to_string(), r#"{"status":"ok"}"#.to_string())
                                    } else {
                                        warn!("❌ Login failed: incorrect passphrase.");
                                        ("401 Unauthorized".to_string(), r#"{"error":"invalid_credentials"}"#.to_string())
                                    }
                                } else {
                                    ("500 Internal Server Error".to_string(), r#"{"error":"corrupted_auth_file"}"#.to_string())
                                }
                            } else {
                                ("400 Bad Request".to_string(), r#"{"error":"not_setup"}"#.to_string())
                            }
                        } else {
                            ("400 Bad Request".to_string(), r#"{"error":"missing_passphrase"}"#.to_string())
                        }
                    } else {
                        ("400 Bad Request".to_string(), r#"{"error":"invalid_json"}"#.to_string())
                    }
                },
                ("POST", "/api/invoke") => {
                    if !IS_LOGGED_IN.load(Ordering::SeqCst) {
                        ("401 Unauthorized".to_string(), r#"{"error":"unauthorized"}"#.to_string())
                    } else {
                        // Parse invoke JSON body: { "cmd": "foo", "args": { ... } }
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(body_str) {
                            if let Some(cmd) = json.get("cmd").and_then(|v| v.as_str()) {
                                let args = json.get("args").cloned().unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
                                match api_router::handle_invoke(
                                    cmd, 
                                    args, 
                                    &app_state, 
                                    &metrics_state, 
                                    &metrics_storage, 
                                    &settings_state
                                ).await {
                                    Ok(res) => ("200 OK".to_string(), serde_json::to_string(&res).unwrap_or_default()),
                                    Err(e) => {
                                        warn!("API Invoke Error [{}]: {}", cmd, e);
                                        ("500 Internal Server Error".to_string(), serde_json::to_string(&serde_json::json!({"error": e})).unwrap_or_default())
                                    }
                                }
                            } else {
                                ("400 Bad Request".to_string(), r#"{"error":"missing_cmd"}"#.to_string())
                            }
                        } else {
                            ("400 Bad Request".to_string(), r#"{"error":"invalid_json"}"#.to_string())
                        }
                    }
                },
                ("OPTIONS", _) => {
                    // Handle CORS preflight
                    ("204 No Content".to_string(), "".to_string())
                },
                // Block API routes from falling through to the SPA fallback
                ("GET", path) if path.starts_with("/api/") => {
                    warn!("API: Not Found: {} {}", method, path);
                    (
                        "404 Not Found".to_string(),
                        r#"{"error":"not_found"}"#.to_string()
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
                        r#"{"error":"not_found"}"#.to_string()
                    )
                }
            };

            debug!("API Response to {} {}: {}", method, path, status);

            // Determine content type from response — JSON for API, mime type for static
            let content_type = if body.starts_with('{') || body.starts_with('[') {
                "application/json"
            } else if status.contains("STATIC:") {
                // Extract the mime type from our marker (see serve_static_asset)
                status.split("STATIC:").nth(1).unwrap_or("application/octet-stream")
            } else {
                "application/json"
            };

            // Clean the status line for the HTTP response
            let clean_status = if status.contains("STATIC:") {
                "200 OK"
            } else {
                &status
            };

            let response = format!(
                "HTTP/1.1 {}\r\nContent-Type: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                clean_status,
                content_type,
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes()).await;
        });
    }
}

/// Serve a static file from the embedded Portal UI assets.
/// Falls back to index.html for SPA routing (React Router).
fn serve_static_asset(path: &str) -> (String, String) {
    let mut asset_path = path.trim_start_matches('/').to_string();
    
    // Root path → index.html
    if asset_path.is_empty() {
        asset_path = "index.html".to_string();
    }

    if asset_path.starts_with("api/") {
        warn!("API route not found, preventing SPA fallback: /{}", asset_path);
        return (
            "404 Not Found".to_string(),
            r#"{"error":"not_found"}"#.to_string()
        );
    }

    debug!("Static asset request: {}", asset_path);

    match PortalAssets::get(&asset_path) {
        Some(content) => {
            let mime = mime_guess::from_path(&asset_path)
                .first_or_octet_stream()
                .to_string();
            let body = String::from_utf8_lossy(content.data.as_ref()).to_string();
            // Use a "STATIC:" prefix in the status to signal the content type to the writer
            (format!("STATIC:{}", mime), body)
        }
        None => {
            // SPA fallback: return index.html for unknown routes (React Router)
            debug!("Asset '{}' not found, serving index.html (SPA fallback)", asset_path);
            match PortalAssets::get("index.html") {
                Some(index) => {
                    let body = String::from_utf8_lossy(index.data.as_ref()).to_string();
                    ("STATIC:text/html".to_string(), body)
                }
                None => {
                    warn!("Portal UI not built! Run 'cd hainet-portal && npm run build'");
                    (
                        "404 Not Found".to_string(),
                        r#"{"error":"Portal UI not built. Run: cd hainet-portal && npm run build"}"#.to_string()
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
