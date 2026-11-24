use serde_json::{Value, json};
use tracing::{error, debug, info};

use crate::{
    AppState, MetricsState, MetricsStorageState, SettingsState,
    admin_bridge, metrics_handler, settings_handler, tts_handler
};

fn derive_onion_address(pub_key_b64: &str) -> Option<String> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use sha3::{Digest, Sha3_256};
    
    let decoded = STANDARD.decode(pub_key_b64.trim()).ok()?;
    if decoded.len() < 32 {
        return None;
    }
    let pubkey = &decoded[decoded.len() - 32..];
    
    let mut hasher = Sha3_256::new();
    hasher.update(b".onion checksum");
    hasher.update(pubkey);
    hasher.update(&[0x03]);
    let hash = hasher.finalize();
    
    let mut onion_data = Vec::with_capacity(35);
    onion_data.extend_from_slice(pubkey);
    onion_data.extend_from_slice(&hash[0..2]);
    onion_data.push(0x03);
    
    let onion_str = base32::encode(base32::Alphabet::Rfc4648 { padding: false }, &onion_data).to_lowercase();
    Some(format!("{}.onion", onion_str))
}

pub async fn handle_invoke(
    cmd: &str,
    args: Value,
    app_state: std::sync::Arc<AppState>,
    metrics_state: &MetricsState,
    metrics_storage: &MetricsStorageState,
    settings_state: &SettingsState,
) -> Result<Value, String> {
    debug!("Routing API invoke: {}", cmd);
    
    match cmd {
        // --- Admin Bridge ---
        "send_message" => {
            let content = args.get("content").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let attachments_val = args.get("attachments").cloned().unwrap_or(Value::Array(vec![]));
            let attachments: Vec<admin_bridge::FileAttachment> = serde_json::from_value(attachments_val).unwrap_or_default();
            
            let bridge_arc = app_state.admin_bridge.as_ref().ok_or_else(|| "Admin Bridge not available on this node".to_string())?;
            let bridge = bridge_arc.read().await;
            let res = bridge.send_message(content, attachments).await.map_err(|e| e.to_string())?;
            Ok(serde_json::to_value(res).unwrap())
        },
        "get_history" => {
            let bridge_arc = app_state.admin_bridge.as_ref().ok_or_else(|| "Admin Bridge not available on this node".to_string())?;
            let bridge = bridge_arc.read().await;
            let res = bridge.get_history().await.map_err(|e| e.to_string())?;
            Ok(serde_json::to_value(res).unwrap())
        },
        "clear_history" => {
            let bridge_arc = app_state.admin_bridge.as_ref().ok_or_else(|| "Admin Bridge not available on this node".to_string())?;
            let bridge = bridge_arc.read().await;
            bridge.clear_history().await.map_err(|e| e.to_string())?;
            Ok(json!({}))
        },
        "new_session" => {
            let bridge_arc = app_state.admin_bridge.as_ref().ok_or_else(|| "Admin Bridge not available on this node".to_string())?;
            let bridge = bridge_arc.read().await;
            let id = bridge.new_session().await.map_err(|e| e.to_string())?;
            Ok(json!({ "session_id": id }))
        },
        "list_sessions" => {
            let bridge_arc = app_state.admin_bridge.as_ref().ok_or_else(|| "Admin Bridge not available on this node".to_string())?;
            let bridge = bridge_arc.read().await;
            let res = bridge.list_sessions().await.map_err(|e| e.to_string())?;
            Ok(serde_json::to_value(res).unwrap())
        },
        "load_session" => {
            let id = args.get("session_id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let bridge_arc = app_state.admin_bridge.as_ref().ok_or_else(|| "Admin Bridge not available on this node".to_string())?;
            let bridge = bridge_arc.read().await;
            bridge.load_session(id).await.map_err(|e| e.to_string())?;
            Ok(json!({}))
        },
        "get_agent_state" => {
            let bridge_arc = app_state.admin_bridge.as_ref().ok_or_else(|| "Admin Bridge not available on this node".to_string())?;
            let bridge = bridge_arc.read().await;
            let res = bridge.get_state().await.map_err(|e| e.to_string())?;
            Ok(json!(res))
        },
        "get_active_agents" => {
            let bridge_arc = app_state.admin_bridge.as_ref().ok_or_else(|| "Admin Bridge not available on this node".to_string())?;
            let bridge = bridge_arc.read().await;
            let res = bridge.get_active_agents().await.map_err(|e| e.to_string())?;
            Ok(serde_json::to_value(res).unwrap())
        },
        "get_active_projects" => {
            let bridge_arc = app_state.admin_bridge.as_ref().ok_or_else(|| "Admin Bridge not available on this node".to_string())?;
            let bridge = bridge_arc.read().await;
            let res = bridge.get_active_projects().await.map_err(|e| e.to_string())?;
            Ok(serde_json::to_value(res).unwrap())
        },
        "pause_project" => {
            let id = args.get("project_id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let bridge_arc = app_state.admin_bridge.as_ref().ok_or_else(|| "Admin Bridge not available on this node".to_string())?;
            let bridge = bridge_arc.read().await;
            bridge.pause_project(id).await.map_err(|e| e.to_string())?;
            Ok(json!({}))
        },
        "resume_project" => {
            let id = args.get("project_id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let bridge_arc = app_state.admin_bridge.as_ref().ok_or_else(|| "Admin Bridge not available on this node".to_string())?;
            let bridge = bridge_arc.read().await;
            bridge.resume_project(id).await.map_err(|e| e.to_string())?;
            Ok(json!({}))
        },
        "stop_project" => {
            let id = args.get("project_id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let bridge_arc = app_state.admin_bridge.as_ref().ok_or_else(|| "Admin Bridge not available on this node".to_string())?;
            let bridge = bridge_arc.read().await;
            bridge.stop_project(id).await.map_err(|e| e.to_string())?;
            Ok(json!({}))
        },
        "rename_project" => {
            let id = args.get("project_id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let name = args.get("new_title").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let bridge_arc = app_state.admin_bridge.as_ref().ok_or_else(|| "Admin Bridge not available on this node".to_string())?;
            let bridge = bridge_arc.read().await;
            bridge.rename_project(id, name).await.map_err(|e| e.to_string())?;
            Ok(json!({}))
        },
        "delete_project" => {
            let id = args.get("project_id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let bridge_arc = app_state.admin_bridge.as_ref().ok_or_else(|| "Admin Bridge not available on this node".to_string())?;
            let bridge = bridge_arc.read().await;
            bridge.delete_project(id).await.map_err(|e| e.to_string())?;
            Ok(json!({}))
        },
        
        // --- Settings ---
        "get_settings" => {
            let res = settings_handler::get_settings(settings_state).await?;
            Ok(serde_json::to_value(res).unwrap())
        },
        "update_settings" => {
            let settings: settings_handler::Settings = serde_json::from_value(args.get("settings").cloned().unwrap_or_default()).map_err(|e| e.to_string())?;
            settings_handler::update_settings(settings, settings_state).await?;
            Ok(json!({}))
        },
        "get_model_preferences" => {
            let res = settings_handler::get_model_preferences(settings_state).await?;
            Ok(serde_json::to_value(res).unwrap())
        },
        "save_model_preference" => {
            let agent = args.get("agent_type").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let family = args.get("family").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let fallback = args.get("allow_fallback").and_then(|v| v.as_bool()).unwrap_or(true);
            settings_handler::save_model_preference(agent, family, fallback, settings_state).await?;
            Ok(json!({}))
        },
        "get_system_status" => {
            // Because we removed the system_info state, we'll just implement it locally here to avoid passing a mutex
            use sysinfo::{System, Disks};
            let mut sys = System::new_all();
            sys.refresh_cpu();
            sys.refresh_memory();
            let cpu_usage = sys.global_cpu_info().cpu_usage();
            let total_memory = sys.total_memory();
            let memory_usage = sys.used_memory();
            let disks = Disks::new_with_refreshed_list();
            let (disk_usage, total_disk) = disks.iter().fold((0, 0), |(used, total), disk| {
                (used + (disk.total_space() - disk.available_space()), total + disk.total_space())
            });
            let status = settings_handler::SystemStatus {
                cpu_usage, memory_usage, total_memory, disk_usage, total_disk,
            };
            Ok(serde_json::to_value(status).unwrap())
        },

        // --- Metrics ---
        "get_metrics_summary" => {
            let res = metrics_handler::get_metrics_summary(metrics_state).await?;
            Ok(serde_json::to_value(res).unwrap())
        },
        "get_agent_metrics" => {
            let id = args.get("agent_id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let res = metrics_handler::get_agent_metrics_by_type(id, metrics_state).await?;
            Ok(serde_json::to_value(res).unwrap())
        },
        "get_metrics_trend" => {
            let agent = args.get("agent_type").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let interval = args.get("interval").and_then(|v| v.as_str()).unwrap_or("3600").to_string();
            let res = metrics_handler::get_metrics_trend(agent, interval, None, metrics_storage).await?;
            Ok(serde_json::to_value(res).unwrap())
        },
        
        // --- Logs ---
        "get_system_logs" => {
            let lines = args.get("lines").and_then(|v| v.as_u64()).unwrap_or(500) as usize;
            
            // Try journalctl first
            let journalctl_logs = std::process::Command::new("journalctl")
                .arg("-u").arg("hainet-core")
                .arg("-n").arg(lines.to_string())
                .arg("--no-pager")
                .output()
                .ok()
                .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
                .unwrap_or_default();
            
            if !journalctl_logs.trim().is_empty() {
                return Ok(json!({ "logs": journalctl_logs }));
            }
            
            // Fallback: read the most recent .log file from configured log directory
            let log_dirs: Vec<std::path::PathBuf> = vec![
                app_state.log_dir.clone(),
                std::path::PathBuf::from("/media/hai-drive/logs"),      // Slave node fallback
                std::env::current_exe()
                    .ok()
                    .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                    .unwrap_or_default()
                    .join("../../logs"),
                std::path::PathBuf::from("logs"),
            ];
            
            let mut newest_log: Option<(std::path::PathBuf, std::time::SystemTime)> = None;
            for dir in &log_dirs {
                if let Ok(entries) = std::fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().map(|e| e == "log").unwrap_or(false) {
                            if let Ok(meta) = path.metadata() {
                                if let Ok(modified) = meta.modified() {
                                    if newest_log.as_ref().map(|(_, t)| modified > *t).unwrap_or(true) {
                                        newest_log = Some((path, modified));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            
            if let Some((log_path, _)) = newest_log {
                match std::fs::read_to_string(&log_path) {
                    Ok(content) => {
                        // Return only the last N lines
                        let all_lines: Vec<&str> = content.lines().collect();
                        let start = if all_lines.len() > lines { all_lines.len() - lines } else { 0 };
                        let tail = all_lines[start..].join("\n");
                        Ok(json!({ "logs": tail, "source": log_path.display().to_string() }))
                    }
                    Err(e) => Ok(json!({ "logs": format!("Failed to read log file {}: {}", log_path.display(), e) })),
                }
            } else {
                Ok(json!({ "logs": "No logs available. journalctl returned empty and no log files found." }))
            }
        },
        
        // --- STT/TTS ---
        "tts_is_ready" => {
            let tts = app_state.tts_handler.read().await;
            Ok(json!(tts.is_ready()))
        },
        "list_tts_voices" => {
            let tts = app_state.tts_handler.read().await;
            let res = tts.list_voices()?;
            Ok(serde_json::to_value(res).unwrap())
        },
        "synthesize_speech" => {
            let req: tts_handler::SynthesisRequest = serde_json::from_value(args.get("request").cloned().unwrap_or_default()).map_err(|e| e.to_string())?;
            let tts = app_state.tts_handler.read().await;
            let res = tts.synthesize(req).await?;
            Ok(serde_json::to_value(res).unwrap())
        },

        "get_node_info" => {
            if let Some(me) = app_state.noslop.personal().await {
                return Ok(serde_json::json!({ "public_key": me.public_key_b64, "onion_address": me.onion }));
            }
            let ident_dir = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/root")).join(".hainet/identity");
            let pub_key = std::fs::read_to_string(ident_dir.join("ed25519_pub.b64")).unwrap_or_default().trim().to_string();
            let mut onion = std::fs::read_to_string("/var/lib/hainet/onion.txt")
                .unwrap_or_default()
                .trim()
                .to_string();
            if onion.is_empty() {
                if let Some(derived) = derive_onion_address(&pub_key) {
                    onion = derived;
                }
            }
            Ok(serde_json::json!({
                "public_key": pub_key,
                "onion_address": onion
            }))
        },

        // ====================================================================
        // --- Compute Router (from hainet-collab / PPLPWR) ---
        // ====================================================================

        // Returns the live hardware profile of this node (CPU, RAM, GPU, score).
        // The frontend ComputeNode page uses this to display real hardware stats.
        "get_hardware_profile" => {
            debug!("Fetching hardware profile from hainet-collab");
            let profile = app_state.hardware_profile.read().await;
            Ok(json!({
                "cpu_cores": profile.cpu_cores,
                "cpu_model": profile.cpu_model,
                "ram_total_gb": profile.ram_total_gb,
                "ram_available_gb": profile.ram_available_gb,
                "gpu": profile.gpu.as_ref().map(|g| json!({
                    "name": g.name,
                    "vram_mb": g.vram_mb,
                    "cuda_version": g.cuda_version,
                    "driver_version": g.driver_version,
                    "temperature_c": g.temperature_c,
                    "utilization_pct": g.utilization_pct,
                })),
                "disk_total_gb": profile.disk_total_gb,
                "disk_available_gb": profile.disk_available_gb,
                "os": profile.os,
                "arch": profile.arch,
                "capability_score": profile.capability_score,
            }))
        },

        // Refreshes hardware detection and returns updated profile.
        // Useful when user wants to re-scan after hardware changes.
        "refresh_hardware_profile" => {
            debug!("Refreshing hardware profile");
            let new_profile = hainet_collab::hardware::HardwareProfile::detect();
            let mut profile = app_state.hardware_profile.write().await;
            *profile = new_profile;
            debug!("Hardware profile refreshed: {} cores, {:.1} GB RAM, score {:.1}",
                profile.cpu_cores, profile.ram_total_gb, profile.capability_score);
            Ok(json!({"status": "refreshed"}))
        },

        // ====================================================================
        // --- Social Feed (from hainet-social / gChat) ---
        // ====================================================================

        // Returns the in-memory social feed posts.
        // In Phase 4 completion, these will come from the gossip engine.
        "get_social_feed" => {
            debug!("Fetching social feed from SQLite");
            let rows = sqlx::query("SELECT * FROM posts ORDER BY timestamp DESC")
                .fetch_all(&app_state.social_db.pool).await.unwrap_or_default();
            let mut posts = Vec::new();
            for row in rows {
                use sqlx::Row;
                posts.push(json!({
                    "id": row.try_get::<String, _>("id").unwrap_or_default(),
                    "author": row.try_get::<String, _>("author").unwrap_or_default(),
                    "content": row.try_get::<String, _>("content").unwrap_or_default(),
                    "timestamp": row.try_get::<String, _>("timestamp").unwrap_or_default(),
                    "media_id": row.try_get::<Option<String>, _>("media_id").unwrap_or_default(),
                    "media_type": row.try_get::<Option<String>, _>("media_type").unwrap_or_default(),
                }));
            }
            Ok(json!({ "posts": posts, "total": posts.len() }))
        },

        // Creates a new local post and adds it to the social feed.
        // In Phase 4, this will also broadcast via the gossip engine.
        "create_post" => {
            let content = args.get("content")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let author = args.get("author")
                .and_then(|v| v.as_str())
                .unwrap_or("My Node (You)")
                .to_string();

            debug!("Creating social post from '{}': '{}'", author, &content[..content.len().min(50)]);

            let post = crate::SocialPost {
                id: uuid::Uuid::new_v4().to_string(),
                author,
                content,
                timestamp: chrono::Utc::now().to_rfc3339(),
                media_id: None,
                media_type: None,
            };

            let _ = sqlx::query("INSERT INTO posts (id, author, content, timestamp) VALUES (?, ?, ?, ?)")
                .bind(&post.id).bind(&post.author).bind(&post.content).bind(&post.timestamp)
                .execute(&app_state.social_db.pool).await;

            debug!("Social post created and saved to SQLite");
            Ok(json!({"status": "posted", "post": post}))
        },

        // Returns the number of connected peers from the gossip engine.
        "get_peer_count" => {
            let gossip = app_state.gossip_engine.read().await;
            let count = gossip.peer_count().await;
            debug!("Peer count: {}", count);
            Ok(json!({"peer_count": count}))
        },

        // Returns the list of mesh peers known to the gossip engine.
        // In Phase 4, this will be populated by actual P2P connections.
        "get_mesh_peers" => {
            // The NoSlop Hub's contact list carries which identity each contact knows and the
            // followers the Hub accepted while the phone was away; the older table is the fallback.
            if let Ok(contacts) = app_state.noslop.store.contacts_all().await {
                if !contacts.is_empty() {
                    let peers: Vec<Value> = contacts.iter().map(|c| json!({
                        "public_key": c.public_key,
                        "is_trusted": c.is_trusted,
                        "handle": c.handle,
                        "onion_address": c.onion_address,
                        "enc_public_key": c.enc_public_key,
                        "identity": c.identity,
                        "relationship": c.relationship,
                        "is_temporary": c.is_temporary,
                    })).collect();
                    return Ok(json!({ "peers": peers, "total": peers.len() }));
                }
            }
            debug!("Fetching mesh peers from SQLite");
            // Exclude the hub's own node_id from the peer list to prevent the
            // mobile app from adding itself as a contact on sync-pull.
            let ident_dir = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/root")).join(".hainet/identity");
            let my_node_id = std::fs::read_to_string(ident_dir.join("ed25519_pub.b64")).unwrap_or_default().trim().to_string();
            let rows = sqlx::query("SELECT * FROM mesh_peers")
                .fetch_all(&app_state.social_db.pool).await.unwrap_or_default();
            let mut peers = Vec::new();
            for row in rows {
                use sqlx::Row;
                let pub_key = row.try_get::<String, _>("public_key").unwrap_or_default();
                if pub_key == my_node_id {
                    continue; // Skip self
                }
                peers.push(json!({
                    "public_key": pub_key,
                    "is_trusted": row.try_get::<bool, _>("is_trusted").unwrap_or_default(),
                    "handle": row.try_get::<String, _>("handle").unwrap_or_default(),
                    "onion_address": row.try_get::<String, _>("onion_address").unwrap_or_default(),
                    "enc_public_key": row.try_get::<String, _>("enc_public_key").unwrap_or_default(),
                }));
            }
            Ok(json!({ "peers": peers, "total": peers.len() }))
        },
        "get_mesh_settings" => {
            let storage = settings_state.read().await;
            let is_discoverable = storage.get_setting("mesh.is_discoverable").await.ok().flatten().unwrap_or_else(|| "false".to_string()) == "true";
            let is_creator = storage.get_setting("mesh.is_creator").await.ok().flatten().unwrap_or_else(|| "false".to_string()) == "true";
            let fund_me_link = storage.get_setting("mesh.fund_me_link").await.ok().flatten().unwrap_or_default();
            Ok(json!({
                "is_discoverable": is_discoverable,
                "is_creator": is_creator,
                "fund_me_link": fund_me_link
            }))
        },
        "save_mesh_settings" => {
            let is_discoverable = args.get("is_discoverable").and_then(|v| v.as_bool()).unwrap_or(false);
            let is_creator = args.get("is_creator").and_then(|v| v.as_bool()).unwrap_or(false);
            let fund_me_link = args.get("fund_me_link").and_then(|v| v.as_str()).unwrap_or("").to_string();

            let storage = settings_state.write().await;
            let _ = storage.save_setting("mesh.is_discoverable", &is_discoverable.to_string()).await;
            let _ = storage.save_setting("mesh.is_creator", &is_creator.to_string()).await;
            let _ = storage.save_setting("mesh.fund_me_link", &fund_me_link).await;
            
            Ok(json!({"status": "saved"}))
        },

        // ====================================================================
        // --- Provider Configuration (persist to settings.db) ---
        // ====================================================================

        // Saves AI provider configuration (Ollama URL, OpenRouter key) to the
        // settings database for persistence across restarts.
        "save_provider_config" => {
            let ollama_url = args.get("ollama_url")
                .and_then(|v| v.as_str())
                .unwrap_or("http://127.0.0.1:11434")
                .to_string();
            let openrouter_key = args.get("openrouter_key")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            debug!("Saving provider config: ollama_url={}, openrouter_key=***", ollama_url);

            // Persist via the settings storage (using the existing key-value pattern)
            let storage = settings_state.write().await;
            storage.save_setting("provider.ollama_url", &ollama_url).await.map_err(|e| e.to_string())?;
            storage.save_setting("provider.openrouter_key", &openrouter_key).await.map_err(|e| e.to_string())?;

            debug!("Provider config saved successfully");
            Ok(json!({"status": "saved"}))
        },

        // Loads the saved AI provider configuration from settings.db.
        "get_provider_config" => {
            debug!("Loading provider config from settings.db");
            let storage = settings_state.read().await;
            let ollama_url = storage.get_setting("provider.ollama_url").await
                .ok().flatten()
                .unwrap_or_else(|| "http://127.0.0.1:11434".to_string());
            let openrouter_key = storage.get_setting("provider.openrouter_key").await
                .ok().flatten()
                .unwrap_or_else(|| "".to_string());

            debug!("Provider config loaded: ollama_url={}", ollama_url);
            Ok(json!({
                "ollama_url": ollama_url,
                "openrouter_key": openrouter_key,
            }))
        },
        
        // ====================================================================
        // --- Mobile Data Sync (Phase 2) ---
        // ====================================================================
        "sync_dms" => {
            debug!("Mobile requested DM sync");
            // Placeholder: Returns Hub's DM state
            Ok(json!({"status": "ok", "dms": []}))
        },
        "sync_contacts" => {
            debug!("Mobile requested Contacts sync");
            // Placeholder: Returns Hub's Contacts state
            Ok(json!({"status": "ok", "contacts": []}))
        },

        // ====================================================================
        // --- Mobile Data Sync (Phase 3) ---
        // ====================================================================
        "sync_push_peers" => {
            debug!("Mobile pushing Contacts/Peers to Hub Firewall");
            if let Some(peers) = args.get("peers").and_then(|p| p.as_array()) {
                let engine = app_state.gossip_engine.read().await;
                // Read the hub's own node_id so we never store it as a peer
                let ident_dir = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/root")).join(".hainet/identity");
                let my_node_id = std::fs::read_to_string(ident_dir.join("ed25519_pub.b64")).unwrap_or_default().trim().to_string();
                for peer in peers {
                    let pub_key = peer.get("public_key").and_then(|v| v.as_str()).unwrap_or_default();
                    // Skip self — the mobile app shares its identity with the hub,
                    // so the hub must never store itself as its own peer.
                    if pub_key == my_node_id {
                        tracing::debug!("sync_push_peers: Skipping self ({})", &my_node_id[..20.min(my_node_id.len())]);
                        continue;
                    }
                    let handle = peer.get("handle").and_then(|v| v.as_str()).unwrap_or_default();
                    let is_trusted = peer.get("is_trusted").and_then(|v| v.as_bool()).unwrap_or(false);
                    let onion = peer.get("onion_address").and_then(|v| v.as_str()).unwrap_or_default();
                    let enc_pub = peer.get("enc_public_key").and_then(|v| v.as_str()).unwrap_or_default();

                    let _ = sqlx::query("INSERT OR REPLACE INTO mesh_peers (public_key, is_trusted, handle, onion_address, enc_public_key) VALUES (?, ?, ?, ?, ?)")
                        .bind(pub_key).bind(is_trusted).bind(handle).bind(onion).bind(enc_pub)
                        .execute(&app_state.social_db.pool).await;

                    if is_trusted {
                        engine.trust_peer(pub_key.to_string()).await;
                    } else {
                        engine.untrust_peer(pub_key).await;
                    }
                }
                if !peers.is_empty() {
                    let placeholders = peers.iter().map(|_| "?").collect::<Vec<_>>().join(",");
                    let query = format!("DELETE FROM mesh_peers WHERE public_key NOT IN ({})", placeholders);
                    let mut q = sqlx::query(&query);
                    for peer in peers {
                        if let Some(pub_key) = peer.get("public_key").and_then(|v| v.as_str()) {
                            q = q.bind(pub_key);
                        }
                    }
                    let _ = q.execute(&app_state.social_db.pool).await;
                } else {
                    let _ = sqlx::query("DELETE FROM mesh_peers").execute(&app_state.social_db.pool).await;
                }
                // Older clients: their contacts as personal contacts of the Hub node.
                if let Ok(jargs) = hainet_noslop::json::parse(&args.to_string()) {
                    if let Err(e) = hainet_noslop::hub::api::contacts_push(&app_state.noslop, &jargs).await {
                        tracing::warn!("sync_push_peers: could not update the Hub contact list: {}", e);
                    }
                }
                Ok(json!({"status": "ok", "peers_processed": peers.len()}))
            } else {
                Err("Missing 'peers' array".to_string())
            }
        },
        
        "sync_push_dms" => {
            tracing::info!("API_ROUTER: sync_push_dms called, updating SQLite");
            if let Some(dms) = args.get("dms").and_then(|d| d.as_array()) {
                for dm in dms {
                    let id = dm.get("id").and_then(|v| v.as_str()).unwrap_or_default();
                    let peer = dm.get("peer").and_then(|v| v.as_str()).unwrap_or_default();
                    let sender = dm.get("sender").and_then(|v| v.as_str()).unwrap_or_default();
                    let content = dm.get("content").and_then(|v| v.as_str()).unwrap_or_default();
                    let timestamp = dm.get("timestamp").and_then(|v| v.as_i64()).unwrap_or_default();
                    let media_id = dm.get("mediaId").and_then(|v| v.as_str());
                    let media_type = dm.get("mediaType").and_then(|v| v.as_str());

                    let _ = sqlx::query("INSERT OR REPLACE INTO dms (id, peer, sender, content, timestamp, media_id, media_type) VALUES (?, ?, ?, ?, ?, ?, ?)")
                        .bind(id).bind(peer).bind(sender).bind(content).bind(timestamp).bind(media_id).bind(media_type)
                        .execute(&app_state.social_db.pool).await;
                }
                Ok(json!({"status": "ok", "dms_processed": dms.len()}))
            } else {
                Err("Missing 'dms' array".to_string())
            }
        },
        "get_dms" => {
            let rows = sqlx::query("SELECT * FROM dms ORDER BY timestamp ASC")
                .fetch_all(&app_state.social_db.pool).await.unwrap_or_default();
            let mut dms = Vec::new();
            for row in rows {
                use sqlx::Row;
                dms.push(json!({
                    "id": row.try_get::<String, _>("id").unwrap_or_default(),
                    "peer": row.try_get::<String, _>("peer").unwrap_or_default(),
                    "sender": row.try_get::<String, _>("sender").unwrap_or_default(),
                    "content": row.try_get::<String, _>("content").unwrap_or_default(),
                    "timestamp": row.try_get::<i64, _>("timestamp").unwrap_or_default(),
                    // Never null: the phone reads these with optString, which turns null into the text "null"
                    // (a phantom attachment that can never download).
                    "mediaId": row.try_get::<Option<String>, _>("media_id").ok().flatten().unwrap_or_default(),
                    "mediaType": row.try_get::<Option<String>, _>("media_type").ok().flatten().unwrap_or_default(),
                    "sent_by_hub": row.try_get::<i64, _>("sent_by_hub").unwrap_or(0) != 0,
                }));
            }
            Ok(json!({ "dms": dms }))
        },
        "sync_pull_dms" => {
            let since = args.get("since").and_then(|v| v.as_i64()).unwrap_or(0);
            let rows = sqlx::query("SELECT * FROM dms WHERE timestamp > ? ORDER BY timestamp ASC LIMIT 200")
                .bind(since)
                .fetch_all(&app_state.social_db.pool).await.unwrap_or_default();
            let mut dms = Vec::new();
            for row in rows {
                use sqlx::Row;
                dms.push(json!({
                    "id": row.try_get::<String, _>("id").unwrap_or_default(),
                    "peer": row.try_get::<String, _>("peer").unwrap_or_default(),
                    "sender": row.try_get::<String, _>("sender").unwrap_or_default(),
                    "content": row.try_get::<String, _>("content").unwrap_or_default(),
                    "timestamp": row.try_get::<i64, _>("timestamp").unwrap_or_default(),
                    // Never null: the phone reads these with optString, which turns null into the text "null"
                    // (a phantom attachment that can never download).
                    "mediaId": row.try_get::<Option<String>, _>("media_id").ok().flatten().unwrap_or_default(),
                    "mediaType": row.try_get::<Option<String>, _>("media_type").ok().flatten().unwrap_or_default(),
                    "sent_by_hub": row.try_get::<i64, _>("sent_by_hub").unwrap_or(0) != 0,
                }));
            }
            Ok(json!({"status": "ok", "dms": dms}))
        },

        "send_dm" => {
            let peer = args.get("peer_id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let content = args.get("content").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            
            if peer.is_empty() || content.is_empty() {
                return Err("Missing peer_id or content".to_string());
            }

            // NOSLOP_HUB_WEBUI_SEND_V1: the Hub sends it now (DM v2, from the identity this contact knows) instead
            // of storing it for the phone to send later from the personal identity, which a creator follower drops.
            let rec = hainet_noslop::hub::dms::send_from_web_ui(&app_state.noslop, &peer, &content).await?;
            sqlx::query("INSERT OR REPLACE INTO dms (id, peer, sender, content, timestamp, media_id, media_type, sent_by_hub) VALUES (?, ?, ?, ?, ?, NULL, NULL, 1)")
                .bind(&rec.id).bind(&rec.peer).bind(&rec.sender).bind(&rec.content).bind(rec.timestamp)
                .execute(&app_state.social_db.pool).await.map_err(|e| e.to_string())?;
            info!("NOSLOP_HUB: Web UI DM {} sent to {}", rec.id, &rec.peer.chars().take(16).collect::<String>());
            Ok(json!({"status": "ok", "id": rec.id, "timestamp": rec.timestamp, "sender": rec.sender}))
        },

        // "sync_push_packets" / "sync_pull_packets" and every "hub_*" / "creator_*" command are handled
        // by the NoSlop Home HUB node (hainet_noslop::hub::api) before this router is reached.

        // The phone's recent posts for the Web UI feed: stored here, sent to nobody.
        "hub_feed_push" => {
            let posts = args.get("posts").and_then(|p| p.as_array()).cloned().unwrap_or_default();
            let n = store_portal_posts(&app_state, posts.iter()).await;
            Ok(json!({"status": "ok", "posts_stored": n}))
        },

        // Fallback for unimplemented endpoints
        _ => {
            error!("Unimplemented API command: {}", cmd);
            Err(format!("Command '{}' not yet ported to HTTP API", cmd))
        }
    }
}

/// Stores `PostPayload`s in the Web UI's `posts` table (what the older `sync_push_packets` did with
/// every POST it saw). Returns how many were stored.
pub async fn store_portal_posts<'a, I: Iterator<Item = &'a Value>>(app_state: &AppState, posts: I) -> usize {
    let mut n = 0;
    for payload in posts {
        let id = payload.get("id").and_then(|v| v.as_str()).unwrap_or_default();
        if id.is_empty() { continue; }
        let content = payload.get("content").and_then(|v| v.as_str()).unwrap_or_default();
        let timestamp = payload.get("timestamp").and_then(|v| v.as_u64()).map(|t| t.to_string()).unwrap_or_else(|| payload.get("timestamp").and_then(|v| v.as_str()).unwrap_or_default().to_string());
        let author = payload.get("author_name").and_then(|v| v.as_str()).unwrap_or("Unknown");
        let media_id = payload.get("media_id").and_then(|v| v.as_str()).map(|s| s.to_string());
        let media_type = payload.get("media_metadata").and_then(|m| m.get("type")).and_then(|v| v.as_str()).map(|s| s.to_string());
        if sqlx::query("INSERT OR REPLACE INTO posts (id, author, content, timestamp, media_id, media_type) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(id).bind(author).bind(content).bind(timestamp).bind(media_id).bind(media_type)
            .execute(&app_state.social_db.pool).await.is_ok() { n += 1; }
    }
    n
}
