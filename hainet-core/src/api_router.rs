use serde_json::{Value, json};
use tracing::{error, debug};

use crate::{
    AppState, MetricsState, MetricsStorageState, SettingsState,
    admin_bridge, metrics_handler, settings_handler, tts_handler
};

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
            let ident_dir = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/root")).join(".hainet/identity");
            let pub_key = std::fs::read_to_string(ident_dir.join("ed25519_pub.b64")).unwrap_or_default().trim().to_string();
            let onion = std::fs::read_to_string("/var/lib/tor/hainet/hostname")
                .unwrap_or_default()
                .trim()
                .to_string();
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
            debug!("Fetching mesh peers from SQLite");
            let rows = sqlx::query("SELECT * FROM mesh_peers")
                .fetch_all(&app_state.social_db.pool).await.unwrap_or_default();
            let mut peers = Vec::new();
            for row in rows {
                use sqlx::Row;
                peers.push(json!({
                    "public_key": row.try_get::<String, _>("public_key").unwrap_or_default(),
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
                for peer in peers {
                    let pub_key = peer.get("public_key").and_then(|v| v.as_str()).unwrap_or_default();
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
                    "mediaId": row.try_get::<Option<String>, _>("media_id").unwrap_or_default(),
                    "mediaType": row.try_get::<Option<String>, _>("media_type").unwrap_or_default(),
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
                    "mediaId": row.try_get::<Option<String>, _>("media_id").unwrap_or_default(),
                    "mediaType": row.try_get::<Option<String>, _>("media_type").unwrap_or_default(),
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

            let ident_dir = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/root")).join(".hainet/identity");
            let my_node_id = std::fs::read_to_string(ident_dir.join("ed25519_pub.b64")).unwrap_or_default().trim().to_string();
            
            let id = uuid::Uuid::new_v4().to_string();
            let timestamp = chrono::Utc::now().timestamp_millis();
            
            let _ = sqlx::query("INSERT INTO dms (id, peer, sender, content, timestamp) VALUES (?, ?, ?, ?, ?)")
                .bind(&id).bind(&peer).bind(&my_node_id).bind(&content).bind(timestamp)
                .execute(&app_state.social_db.pool).await;
                
            Ok(json!({"status": "ok", "id": id, "timestamp": timestamp}))
        },

        "sync_push_packets" => {
            debug!("Mobile pushing packets to Hub Firewall");
            if let Some(packets) = args.get("packets").and_then(|p| p.as_array()) {
                let engine = app_state.gossip_engine.read().await;
                
                // FIX: Trust our own mobile node ID so the firewall doesn't drop our outbound broadcasts!
                let ident_dir = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/root")).join(".hainet/identity");
                let my_node_id = std::fs::read_to_string(ident_dir.join("ed25519_pub.b64")).unwrap_or_default().trim().to_string();
                engine.trust_peer(my_node_id.clone()).await;

                for packet_json in packets {
                    // Extract POST directly to bypass strict parsing failures
                    if let Some(ptype) = packet_json.get("type").and_then(|v| v.as_str()) {
                        if ptype == "POST" {
                            if let Some(payload) = packet_json.get("payload") {
                                let id = payload.get("id").and_then(|v| v.as_str()).unwrap_or_default();
                                let content = payload.get("content").and_then(|v| v.as_str()).unwrap_or_default();
                                let timestamp = payload.get("timestamp").and_then(|v| v.as_u64()).map(|t| t.to_string()).unwrap_or_else(|| payload.get("timestamp").and_then(|v| v.as_str()).unwrap_or_default().to_string());
                                let author = payload.get("author_name").and_then(|v| v.as_str()).unwrap_or("Unknown");
                                let media_id = payload.get("media_id").and_then(|v| v.as_str()).map(|s| s.to_string());
                                let media_type = payload.get("media_metadata").and_then(|m| m.get("type")).and_then(|v| v.as_str()).map(|s| s.to_string());
                                
                                if !id.is_empty() {
                                    let _ = sqlx::query("INSERT OR REPLACE INTO posts (id, author, content, timestamp, media_id, media_type) VALUES (?, ?, ?, ?, ?, ?)")
                                        .bind(id).bind(author).bind(content).bind(timestamp).bind(media_id).bind(media_type)
                                        .execute(&app_state.social_db.pool).await;
                                }
                            }
                        } else if ptype == "MESSAGE" {
                            let target_id = packet_json.get("target_user_id").or_else(|| packet_json.get("targetUserId")).and_then(|v| v.as_str()).unwrap_or_default().replace("\n", "").replace("\r", "");
                            let sender_id = packet_json.get("sender_id").or_else(|| packet_json.get("senderId")).and_then(|v| v.as_str()).unwrap_or_default().replace("\n", "").replace("\r", "");
                            let ident_dir = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/root")).join(".hainet/identity");
                            let my_node_id = std::fs::read_to_string(ident_dir.join("ed25519_pub.b64")).unwrap_or_default().trim().to_string();
                            
                            let admin_id = format!("admin_{}", my_node_id);
                            tracing::warn!("API_ROUTER MESSAGE CHECK: sender=[{}], target=[{}], my_node=[{}], admin=[{}]", sender_id, target_id, my_node_id, admin_id);
                            if target_id == admin_id && sender_id == my_node_id {
                                let state_clone = app_state.clone();
                                let pjson_clone = packet_json.clone();
                                tokio::spawn(async move {
                                    crate::handle_incoming_dm(state_clone, pjson_clone).await;
                                });
                            }
                        }
                    }

                    // Attempt strict routing for the engine (deduplication, firewall, etc)
                    if let Ok(packet) = serde_json::from_value::<hainet_social::packets::NetworkPacket>(packet_json.clone()) {
                        let _ = engine.process_incoming(&packet).await;
                    }

                    // Fallback/Guaranteed Tor Routing based on raw JSON properties
                    // This bypasses strict parsing failures (like camelCase targetUserId vs snake_case target_user_id)
                    let packet_str = packet_json.to_string();
                    let target_id = packet_json.get("target_user_id")
                        .or_else(|| packet_json.get("targetUserId"))
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();

                    let db = app_state.social_db.clone();
                    
                    tokio::spawn(async move {
                        if !target_id.is_empty() {
                            // Directed packet: Send only to target
                            let mut sent = false;
                            if let Ok(row) = sqlx::query("SELECT onion_address FROM mesh_peers WHERE public_key = ?")
                                .bind(&target_id)
                                .fetch_one(&db.pool).await 
                            {
                                use sqlx::Row;
                                if let Ok(onion) = row.try_get::<String, _>("onion_address") {
                                    if !onion.is_empty() {
                                        sent = send_packet_over_tor(&onion, &packet_str).await;
                                    }
                                }
                            }
                            
                            // If direct send failed (e.g. peer is offline or onion changed),
                            // fallback to gossip relaying across all trusted peers.
                            if !sent {
                                tracing::warn!("Hub: Direct send to {} failed. Falling back to gossip relay.", target_id);
                                if let Ok(mut packet_obj) = serde_json::from_str::<serde_json::Value>(&packet_str) {
                                    if let Some(obj) = packet_obj.as_object_mut() {
                                        obj.insert("id".to_string(), serde_json::json!(uuid::Uuid::new_v4().to_string()));
                                        obj.insert("hops".to_string(), serde_json::json!(6));
                                    }
                                    let fallback_str = serde_json::to_string(&packet_obj).unwrap_or_else(|_| packet_str.clone());
                                    if let Ok(rows) = sqlx::query("SELECT onion_address FROM mesh_peers WHERE is_trusted = 1 AND public_key != ?")
                                        .bind(&target_id)
                                        .fetch_all(&db.pool).await 
                                    {
                                        for row in rows {
                                            use sqlx::Row;
                                            if let Ok(onion) = row.try_get::<String, _>("onion_address") {
                                                if !onion.is_empty() {
                                                    let payload = fallback_str.clone();
                                                    tokio::spawn(async move {
                                                        send_packet_over_tor(&onion, &payload).await;
                                                    });
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        } else {
                            // Broadcast packet: Send to ALL trusted peers
                            if let Ok(rows) = sqlx::query("SELECT onion_address FROM mesh_peers WHERE is_trusted = 1")
                                .fetch_all(&db.pool).await 
                            {
                                for row in rows {
                                    use sqlx::Row;
                                    if let Ok(onion) = row.try_get::<String, _>("onion_address") {
                                        if !onion.is_empty() {
                                            let payload = packet_str.clone();
                                            tokio::spawn(async move {
                                                send_packet_over_tor(&onion, &payload).await;
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    });
                }
                Ok(json!({"status": "ok", "packets_processed": packets.len()}))
            } else {
                Err("Missing 'packets' array".to_string())
            }
        },
        "sync_pull_packets" => {
            let mut buffer = app_state.incoming_mesh_packets.write().await;
            let packets = buffer.clone();
            buffer.clear();
            Ok(json!({"status": "ok", "packets": packets}))
        },

        // Fallback for unimplemented endpoints
        _ => {
            error!("Unimplemented API command: {}", cmd);
            Err(format!("Command '{}' not yet ported to HTTP API", cmd))
        }
    }
}

async fn send_packet_over_tor(onion: &str, packet_str: &str) -> bool {
    use tokio::io::AsyncWriteExt;
    let target = format!("{}:9999", onion);
    let mut success = false;
    for attempt in 1..=3 {
        let connect_future = tokio_socks::tcp::Socks5Stream::connect("127.0.0.1:9050", &target);
        match tokio::time::timeout(tokio::time::Duration::from_secs(60), connect_future).await {
            Ok(Ok(mut stream)) => {
                if stream.write_all(format!("{}\n", packet_str).as_bytes()).await.is_ok() {
                    tracing::info!("Successfully routed packet to {} on attempt {}", target, attempt);
                    success = true;
                    break;
                } else {
                    tracing::warn!("Failed to write packet to {} on attempt {}", target, attempt);
                }
            }
            Ok(Err(e)) => {
                tracing::warn!("Failed to connect to {} via Tor on attempt {}: {}", target, attempt, e);
            }
            Err(_) => {
                tracing::warn!("Timeout connecting to {} via Tor on attempt {}", target, attempt);
            }
        }
        if attempt < 3 {
            tokio::time::sleep(tokio::time::Duration::from_secs(5 * attempt as u64)).await;
        }
    }
    if !success {
        tracing::error!("Failed to route packet to {} after 3 attempts.", target);
    }
    success
}
