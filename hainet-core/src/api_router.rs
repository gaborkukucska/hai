use serde_json::{Value, json};
use tracing::{error, debug};

use crate::{
    AppState, MetricsState, MetricsStorageState, SettingsState,
    admin_bridge, metrics_handler, settings_handler, tts_handler
};

pub async fn handle_invoke(
    cmd: &str,
    args: Value,
    app_state: &AppState,
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
            debug!("Fetching social feed");
            let posts = app_state.social_posts.read().await;
            Ok(json!({
                "posts": *posts,
                "total": posts.len(),
            }))
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
            };

            let mut posts = app_state.social_posts.write().await;
            posts.insert(0, post.clone()); // Newest first

            // TODO Phase 4: Broadcast via gossip engine
            // let gossip = app_state.gossip_engine.read().await;
            // gossip.create_packet(PacketPayload::Post { ... });

            debug!("Social feed now has {} posts", posts.len());
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
            debug!("Fetching mesh peers from gossip engine");
            let gossip = app_state.gossip_engine.read().await;
            let count = gossip.peer_count().await;
            // For now, return the count. Full peer list will come from
            // libp2p integration in Phase 4.
            Ok(json!({
                "peers": [],
                "total": count,
            }))
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
                    if let (Some(pub_key), Some(is_trusted)) = (peer.get("public_key").and_then(|v| v.as_str()), peer.get("is_trusted").and_then(|v| v.as_bool())) {
                        if is_trusted {
                            engine.trust_peer(pub_key.to_string()).await;
                        } else {
                            engine.untrust_peer(pub_key).await;
                        }
                    }
                }
                Ok(json!({"status": "ok", "peers_processed": peers.len()}))
            } else {
                Err("Missing 'peers' array".to_string())
            }
        },
        "sync_push_packets" => {
            debug!("Mobile pushing packets to Hub Firewall");
            if let Some(packets) = args.get("packets").and_then(|p| p.as_array()) {
                let engine = app_state.gossip_engine.read().await;
                for packet_json in packets {
                    if let Ok(packet) = serde_json::from_value::<hainet_social::packets::NetworkPacket>(packet_json.clone()) {
                        let _ = engine.process_incoming(&packet).await;
                        
                        // If it's a POST packet, add it to social_posts so it appears in the Portal
                        if let Some(ptype) = packet_json.get("type").and_then(|v| v.as_str()) {
                            if ptype == "POST" {
                                if let Some(payload) = packet_json.get("payload") {
                                    let id = packet_json.get("id").and_then(|v| v.as_str()).unwrap_or_default();
                                    let content = payload.get("content").and_then(|v| v.as_str()).unwrap_or_default();
                                    let timestamp = payload.get("timestamp").and_then(|v| v.as_u64()).unwrap_or_default();
                                    let author = payload.get("author_name").and_then(|v| v.as_str()).unwrap_or("Unknown");
                                    
                                    let mut posts = app_state.social_posts.write().await;
                                    if !posts.iter().any(|p| p.id == id) {
                                        posts.push(crate::SocialPost {
                                            id: id.to_string(),
                                            author: author.to_string(),
                                            content: content.to_string(),
                                            timestamp: timestamp.to_string(),
                                        });
                                    }
                                }
                            }
                        }
                    }
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
