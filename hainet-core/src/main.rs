//! HAI-Net Core Daemon
//! 
//! The main daemon that coordinates all HAI-Net services including networking,
//! storage, and communication with other components.
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

pub struct AppState {
    pub admin_bridge: Arc<RwLock<AdminBridge>>,
    pub tts_handler: Arc<RwLock<TTSHandler>>,
}

pub type MetricsState = Arc<RwLock<MetricsCollector>>;
pub type MetricsStorageState = Arc<RwLock<MetricsStorage>>;
pub type SettingsState = Arc<RwLock<SettingsStorage>>;

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

    // Initialize Admin AI Bridge and other backend states
    let admin_bridge = AdminBridge::new().await
        .expect("Failed to initialize Admin AI Bridge");
    
    // Initialize database directory
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/root/.local/share"))
        .join("hainet-portal");
    let _ = std::fs::create_dir_all(&data_dir);
    
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
    
    // Wrap states in Arc<RwLock<>> for shared state
    let metrics_state: MetricsState = Arc::new(RwLock::new(metrics_collector));
    let metrics_storage_state: MetricsStorageState = Arc::new(RwLock::new(metrics_storage));
    let settings_state: SettingsState = Arc::new(RwLock::new(settings_storage));
    let tts_handler = TTSHandler::new();
    
    let app_state = Arc::new(AppState {
        admin_bridge: Arc::new(RwLock::new(admin_bridge)),
        tts_handler: Arc::new(RwLock::new(tts_handler)),
    });

    // Step 4: Start minimal TCP-based health/API endpoint
    let role_for_health = config.network.role.clone();
    let health_app_state = app_state.clone();
    let health_metrics_state = metrics_state.clone();
    let health_metrics_storage = metrics_storage_state.clone();
    let health_settings_state = settings_state.clone();
    let health_handle = tokio::spawn(async move {
        if let Err(e) = run_health_server(
            health_port, 
            &role_for_health, 
            health_app_state, 
            health_metrics_state, 
            health_metrics_storage, 
            health_settings_state
        ).await {
            warn!("⚠  Health endpoint failed: {}", e);
        }
    });

    info!("✅ HAI-Net Core initialized successfully");
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
    settings_state: SettingsState
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
                    if is_setup() {
                        ("200 OK".to_string(), r#"{"status":"login_required"}"#.to_string())
                    } else {
                        ("200 OK".to_string(), r#"{"status":"setup_required"}"#.to_string())
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
                _ => {
                    warn!("API: Not Found: {} {}", method, path);
                    (
                        "404 Not Found".to_string(),
                        r#"{"error":"not_found"}"#.to_string()
                    )
                }
            };

            debug!("API Response to {} {}: {}", method, path, status);

            let response = format!(
                "HTTP/1.1 {}\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                status,
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes()).await;
        });
    }
}
