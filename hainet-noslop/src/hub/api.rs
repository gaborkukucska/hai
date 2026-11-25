//! START OF FILE hainet-noslop/src/hub/api.rs
//!
//! The owner's control API of the NoSlop Home HUB (served by `hainet-core` on `/api/invoke`).
//!
//! **Authentication.** Every request from the phone carries an owner signature in four headers:
//! `X-NoSlop-Hub-Key` (the owner's personal public key, which must be the Hub's own identity),
//! `X-NoSlop-Hub-Ts` (ms, within ±5 minutes), `X-NoSlop-Hub-Nonce` (16-64 chars of `[A-Za-z0-9_-]`,
//! never reused within 10 minutes) and `X-NoSlop-Hub-Sig`, an Ed25519 signature over
//! `encodeForSigning("noslop-hub-api-v1", METHOD, PATH, ts, nonce, sha256hex(raw body))`.
//! Being able to reach the API (LAN or onion) is not authentication.
//!
//! Commands (`{"cmd": ..., "args": {...}}`), all returning JSON:
//!
//! | cmd | what it does |
//! |---|---|
//! | `hub_status` | identities, creator hosting and onion state, inbox and content counts |
//! | `hub_send` | `{items:[{onion, packet}]}`: send each packet to the onion the phone chose |
//! | `hub_inbox_pull` | `{after, limit, max_bytes}`: stored packets and notices, oldest first |
//! | `hub_inbox_ack` | `{up_to}`: removes everything up to that sequence number |
//! | `hub_contacts_push` | `{contacts:[...], full}`: the phone's contacts (which identity each knows) |
//! | `creator_identity_set` / `creator_identity_remove` | the creator identity's keys |
//! | `creator_profile_set` | the creator profile and the creator / discoverable switches |
//! | `creator_hosting_set` | `{enabled}`: the Hub takes over (or gives back) the creator onion |
//! | `creator_content_sync` | `{posts, comments, reactions, keep_ids, full}`: the creator's public content |
//! | `creator_media_put` / `creator_media_status` | resumable upload of the creator's public media |
//! | `sync_push_packets` / `sync_pull_packets` | older clients: same paths, without destinations / ACKs |
//! | `creator_onion_republish` | publishes the creator onion again (fresh descriptor) after the phone handed it back |
//! | `dm_media_put` | resumable upload of an attachment of one of the owner's DMs (the Web UI chat shows it) |
//! | `hub_dm_media_info` | `{media_ids}`: whether each DM attachment is on the Hub (`complete`) or still coming |

use super::{admin, content, creator, store::Contact, tor, CreatorProfile, HubNode, Retry};
use crate::{canon, envelope::sha256_hex, json::JValue, keys, packet::Packet};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use tracing::{info, warn};

pub const API_PROTOCOL: i64 = 1;
const MAX_SKEW_MS: i64 = 5 * 60_000;
const NONCE_TTL_MS: i64 = 10 * 60_000;
/// Largest media upload chunk (raw bytes) per `creator_media_put`.
pub const MEDIA_PUT_MAX: usize = 1024 * 1024;

/// Remembers request nonces for [`NONCE_TTL_MS`].
#[derive(Default)]
pub struct NonceCache { seen: Mutex<HashMap<String, i64>> }

impl NonceCache {
    async fn admit(&self, nonce: &str, now: i64) -> bool {
        let mut m = self.seen.lock().await;
        m.retain(|_, t| now - *t < NONCE_TTL_MS);
        if m.contains_key(nonce) { return false; }
        m.insert(nonce.to_string(), now);
        true
    }
}

/// Owner-signature headers, looked up case-insensitively by the caller (keys lower-cased).
pub async fn verify_owner_request(node: &HubNode, nonces: &NonceCache, method: &str, path: &str, headers: &HashMap<String, String>, body: &[u8]) -> Result<(), String> {
    let body_hash = sha256_hex(body);
    verify_owner_request_hashed(node, nonces, method, path, headers, &body_hash).await
}

/// As [`verify_owner_request`] when the body was hashed while streaming it to disk.
pub async fn verify_owner_request_hashed(node: &HubNode, nonces: &NonceCache, method: &str, path: &str, headers: &HashMap<String, String>, body_sha256_hex: &str) -> Result<(), String> {
    let h = |k: &str| headers.get(k).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let (Some(key), Some(ts), Some(nonce), Some(sig)) = (h("x-noslop-hub-key"), h("x-noslop-hub-ts"), h("x-noslop-hub-nonce"), h("x-noslop-hub-sig")) else {
        return Err("missing owner signature".into());
    };
    let owner = node.personal().await.ok_or("this Hub has no owner identity")?;
    if key != owner.public_key_b64 { return Err("not the owner of this Hub".into()); }
    let ts: i64 = ts.parse().map_err(|_| "bad timestamp")?;
    let now = node.now();
    if (now - ts).abs() > MAX_SKEW_MS { return Err("request timestamp too far from the Hub's clock".into()); }
    if nonce.len() < 16 || nonce.len() > 64 || !nonce.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-') { return Err("bad nonce".into()); }
    if !keys::verify(&canon::hub_api_request(method, path, ts, &nonce, body_sha256_hex), &sig, &owner.public_key_b64) {
        return Err("owner signature does not verify".into());
    }
    if !nonces.admit(&nonce, now).await { return Err("replayed request".into()); }
    Ok(())
}

fn arr<'a>(args: &'a JValue, k: &str) -> &'a [JValue] { args.get(k).and_then(|v| v.as_array()).map(|v| v.as_slice()).unwrap_or(&[]) }
fn s<'a>(v: &'a JValue, k: &str) -> &'a str { v.get_str(k).unwrap_or("") }

/// Handles a Hub command, or returns `None` when `cmd` is not one of them. `args` keeps the wire
/// order of the request (packets inside are passed on exactly as the phone wrote them).
pub async fn handle(node: &Arc<HubNode>, cmd: &str, args: &JValue) -> Option<Result<Value, String>> {
    Some(match cmd {
        "hub_status" => status(node).await,
        "hub_send" => send(node, args).await,
        "hub_inbox_pull" => inbox_pull(node, args).await,
        "hub_inbox_ack" => {
            let up_to = args.get_i64("up_to").unwrap_or(0);
            node.store.inbox_ack(up_to).await.map(|n| json!({"status": "ok", "deleted": n})).map_err(|e| e.to_string())
        }
        "hub_contacts_push" => contacts_push(node, args).await,
        "hub_dms_clear" => {
            // NOSLOP_HUB_DM_DELETE_V1: "Clear Chat" on the phone clears the Web UI's copy of that conversation.
            let Some(peer) = args.get_str("peer").filter(|p| !p.is_empty()) else { return Some(Err("peer required".into())) };
            let before = args.get_i64("before").filter(|b| *b > 0).unwrap_or_else(|| node.now());
            super::dms::clear_conversation(node, peer, before).await.map(|(n, kept)| json!({"deleted": n, "before": kept}))
        }
        "hub_dm_media_info" => {
            // NOSLOP_HUB_DM_MEDIA_V1: for the Web UI chat (and the phone, to see what it still has to upload).
            let ids: Vec<String> = arr(args, "media_ids").iter().filter_map(|v| v.as_str().map(String::from)).collect();
            super::dm_media::info(node, &ids).await.map(|items| json!({"items": items.iter().map(|i| i.to_serde()).collect::<Vec<_>>()}))
        }
        "dm_media_put" => super::dm_media::put(node, args).await.map(|v| v.to_serde()),
        "creator_onion_republish" => {
            // NOSLOP_HUB_ONION_REPUBLISH_V1 (see HubNode::republish_creator_onion).
            if node.republish_creator_onion().await {
                info!("NOSLOP_HUB: creator onion published again at the phone's request (fresh descriptor)");
                Ok(json!({"status": "ok", "republished": true}))
            } else {
                Ok(json!({"status": "ok", "republished": false}))
            }
        }
        "hub_dm_events" => {
            // NOSLOP_HUB_DM_VIEW_V1: DMs from contacts that reached the Hub, for the Web UI's notifications.
            let after = args.get_i64("after").unwrap_or(0).max(0);
            let limit = args.get_i64("limit").unwrap_or(200).clamp(1, 500);
            match node.store.dm_events_after(after, limit).await {
                Ok((events, latest)) => Ok(json!({
                    "events": events.iter().map(|e| json!({"seq": e.seq, "dm_id": e.dm_id, "peer": e.peer, "sender": e.sender, "preview": e.preview, "at": e.at})).collect::<Vec<_>>(),
                    "latest": latest,
                })),
                Err(e) => Err(e.to_string()),
            }
        }
        "creator_identity_set" => creator_identity_set(node, args).await,
        "creator_identity_remove" => creator_identity_remove(node).await,
        "creator_profile_set" => {
            let opt = |k: &str| args.get_str(k).filter(|v| !v.trim().is_empty()).map(String::from);
            let p = CreatorProfile {
                display_name: s(args, "display_name").into(), handle: s(args, "handle").into(), bio: opt("bio"), avatar_b64: opt("avatar_b64"),
                fund_me_link: opt("fund_me_link"), is_creator: args.get_bool("is_creator").unwrap_or(false), is_discoverable: args.get_bool("is_discoverable").unwrap_or(false),
            };
            node.set_profile(p).await.map(|_| json!({"status": "ok"}))
        }
        "creator_hosting_set" => {
            let on = args.get_bool("enabled").unwrap_or(false);
            if on && node.creator().await.is_none() { Err("no creator identity on this Hub".into()) } else {
                match node.set_hosting(on).await {
                    Ok(()) => { info!("NOSLOP_HUB: creator hosting {}", if on { "ON" } else { "OFF" }); status(node).await }
                    Err(e) => Err(e),
                }
            }
        }
        "creator_content_sync" => content_sync(node, args).await,
        "creator_media_put" => media_put(node, args).await,
        "creator_media_status" => {
            let mut out = vec![];
            for id in arr(args, "media_ids") {
                let id = id.as_str().unwrap_or("");
                let row = node.store.media_get(id).await.ok().flatten();
                out.push(json!({"media_id": id, "received": row.as_ref().map(|r| r.received).unwrap_or(0), "complete": row.map(|r| r.complete).unwrap_or(false)}));
            }
            Ok(json!({"status": "ok", "media": out}))
        }
        "sync_push_packets" => legacy_push(node, args).await,
        "sync_pull_packets" => legacy_pull(node).await,
        _ => return None,
    })
}

async fn status(node: &Arc<HubNode>) -> Result<Value, String> {
    let personal = node.personal().await;
    let creator = node.creator().await;
    let onion = node.onion_status().await;
    let (inbox_n, inbox_b) = node.store.inbox_stats().await.map_err(|e| e.to_string())?;
    let contacts = node.store.contacts_all().await.map_err(|e| e.to_string())?;
    let posts = node.store.post_ids().await.map_err(|e| e.to_string())?.len();
    let media_bytes = node.store.media_total_bytes().await.map_err(|e| e.to_string())?;
    let profile = node.profile().await;
    let hosting = node.hosting().await;
    Ok(json!({
        "status": "ok",
        "api_protocol": API_PROTOCOL,
        "hub_version": env!("CARGO_PKG_VERSION"),
        "personal": personal.map(|p| json!({"public_key": p.public_key_b64, "onion_address": p.onion})),
        "creator": creator.map(|c| json!({
            "public_key": c.public_key_b64,
            "onion_address": c.onion,
            "hosting": hosting,
            "onion_published": onion.published,
            "onion_error": onion.error,
            "is_creator": profile.is_creator,
            "is_discoverable": profile.is_discoverable,
        })),
        "creator_hosting": hosting,
        "inbox": {"items": inbox_n, "bytes": inbox_b},
        "contacts": {"total": contacts.len(), "creator_followers": contacts.iter().filter(|c| c.identity == "creator" && c.relationship == "ACCEPTED").count(), "awaiting_phone": contacts.iter().filter(|c| c.source == "hub").count()},
        "creator_content": {"posts": posts, "media_bytes": media_bytes, "media_quota": node.cfg.media_quota},
        "server_time": node.now(),
    }))
}

/// The retry policy for a packet the phone hands over.
fn retry_for(ptype: &str) -> Retry {
    match ptype {
        "SYNC_RESPONSE" => Retry::Sync,
        "TYPING" | "READ_RECEIPT" | "ANNOUNCE_PEER" | "ANNOUNCE_DISCOVERABLE" | "USER_EXIT" => Retry::Once,
        t if t.starts_with("MEDIA_") => Retry::Once,
        _ => Retry::Spool,
    }
}

/// What the Hub learns from a packet the phone is sending (creator content, Admin AI messages).
/// Returns true when the packet was for the Hub itself and must not go out.
async fn observe_outgoing(node: &Arc<HubNode>, pkt: &Packet) -> bool {
    if let Some(owner) = node.personal().await {
        if admin::is_owner_to_admin(&owner, pkt) {
            match node.admin_hook.read().await.clone() {
                Some(hook) => { tokio::spawn(hook(pkt.clone())); }
                None => warn!("NOSLOP_HUB: message for the Admin AI, but no Admin AI is running on this Hub"),
            }
            return true;
        }
    }
    if let Some(c) = node.creator().await {
        if pkt.sender_id == c.public_key_b64 || pkt.p_str("author_id") == Some(c.public_key_b64.as_str()) {
            match pkt.ptype.as_str() {
                "POST" | "EDIT_POST" | "DELETE_POST" => creator::apply_creator_post_packet(node, pkt).await,
                _ => {}
            }
        }
    }
    if matches!(pkt.ptype.as_str(), "COMMENT" | "EDIT_COMMENT" | "DELETE_COMMENT" | "REACTION") {
        creator::apply_engagement(node, pkt).await;
    }
    if pkt.ptype == "MESSAGE" {
        // NOSLOP_HUB_DM_VIEW_V1: the owner's own DMs show in the Web UI as soon as they leave through the Hub.
        super::dms::observe(node, pkt, false).await;
    } else if pkt.ptype == "DELETE_MESSAGE" {
        // NOSLOP_HUB_DM_DELETE_V1: the owner deleting their own message on the phone.
        super::dms::observe_delete(node, pkt, false).await;
    }
    false
}

async fn send(node: &Arc<HubNode>, args: &JValue) -> Result<Value, String> {
    let items = arr(args, "items");
    if items.len() > 500 { return Err("too many items (max 500)".into()); }
    let (mut queued, mut local, mut rejected) = (0, 0, vec![]);
    for (i, it) in items.iter().enumerate() {
        let Some(pv) = it.get("packet") else { rejected.push(json!({"index": i, "reason": "no packet"})); continue };
        let pkt = match Packet::from_value(pv) { Ok(p) => p, Err(e) => { rejected.push(json!({"index": i, "reason": e.to_string()})); continue } };
        if observe_outgoing(node, &pkt).await { local += 1; continue; }
        let onion = s(it, "onion");
        if !tor::is_valid_onion(onion) { rejected.push(json!({"index": i, "reason": "invalid onion"})); continue; }
        node.deliver(onion.to_string(), pkt.clone(), retry_for(&pkt.ptype));
        queued += 1;
    }
    Ok(json!({"status": "ok", "queued": queued, "handled_by_hub": local, "rejected": rejected}))
}

async fn inbox_pull(node: &Arc<HubNode>, args: &JValue) -> Result<Value, String> {
    let after = args.get_i64("after").unwrap_or(0);
    let limit = args.get_i64("limit").unwrap_or(200).clamp(1, 500);
    let max_bytes = args.get_i64("max_bytes").unwrap_or(4 * 1024 * 1024).clamp(64 * 1024, 16 * 1024 * 1024);
    let (items, more) = node.store.inbox_pull(after, limit, max_bytes).await.map_err(|e| e.to_string())?;
    let out: Vec<Value> = items.into_iter().map(|i| json!({"seq": i.seq, "kind": i.kind, "packet": i.packet})).collect();
    Ok(json!({"status": "ok", "items": out, "more": more}))
}

fn contact_from(v: &JValue, default_identity: &str) -> Option<Contact> {
    let key = v.get_str("public_key")?.trim();
    if key.is_empty() { return None; }
    let trusted = v.get_bool("is_trusted").unwrap_or(false);
    let identity = match v.get_str("identity") { Some("creator") => "creator", Some("personal") => "personal", _ => default_identity };
    let onion = v.get_str("onion_address").unwrap_or("").trim().to_ascii_lowercase();
    Some(Contact {
        public_key: key.to_string(),
        identity: identity.to_string(),
        relationship: v.get_str("relationship").map(String::from).unwrap_or_else(|| if trusted { "ACCEPTED".into() } else { "NONE".into() }),
        is_trusted: trusted,
        is_temporary: v.get_bool("is_temporary").unwrap_or(false),
        handle: v.get_str("handle").unwrap_or("").to_string(),
        onion_address: if tor::is_valid_onion(&onion) { onion } else { String::new() },
        enc_public_key: v.get_str("enc_public_key").unwrap_or("").to_string(),
        source: "phone".into(),
        declined_nonce: v.get_str("declined_nonce").unwrap_or("").to_string(),
    })
}

/// Contacts as the phone pushes them. A full push replaces the phone's rows (rows the Hub added for
/// followers stay until the phone lists them).
pub async fn contacts_push(node: &Arc<HubNode>, args: &JValue) -> Result<Value, String> {
    let (owner, creator) = (node.personal().await.map(|i| i.public_key_b64), node.creator().await.map(|i| i.public_key_b64));
    let list: Vec<Contact> = arr(args, "contacts").iter().chain(arr(args, "peers").iter())
        .filter_map(|v| contact_from(v, "personal"))
        .filter(|c| Some(&c.public_key) != owner.as_ref() && Some(&c.public_key) != creator.as_ref() && !c.public_key.starts_with("admin_"))
        .collect();
    let now = node.now();
    if args.get_bool("full").unwrap_or(true) {
        let removed = node.store.contacts_replace_from_phone(&list, now).await.map_err(|e| e.to_string())?;
        // The phone now owns every follower it lists; the Hub's own rows for them are replaced above.
        for c in &list {
            if let Ok(Some(row)) = node.store.contact_get(&c.public_key).await { if row.source == "hub" { let _ = node.store.contact_upsert(c, now).await; } }
        }
        Ok(json!({"status": "ok", "contacts": list.len(), "removed": removed.len()}))
    } else {
        for c in &list { node.store.contact_upsert(c, now).await.map_err(|e| e.to_string())?; }
        Ok(json!({"status": "ok", "contacts": list.len()}))
    }
}

fn write_secret(path: &std::path::Path, value: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; opts.mode(0o600); }
    let mut f = opts.open(path)?;
    f.write_all(value.as_bytes())?;
    #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?; }
    Ok(())
}

async fn creator_identity_set(node: &Arc<HubNode>, args: &JValue) -> Result<Value, String> {
    let (pk, sk, epk, esk) = (s(args, "public_key"), s(args, "private_key"), s(args, "enc_public_key"), s(args, "enc_private_key"));
    let id = keys::Identity::from_b64(pk, sk, epk, esk).map_err(|e| format!("creator keys rejected: {}", e))?;
    if let Some(owner) = node.personal().await { if owner.public_key_b64 == id.public_key_b64 { return Err("the creator identity must not be the personal identity".into()); } }
    if let Some(cur) = node.creator().await {
        if cur.public_key_b64 != id.public_key_b64 && !args.get_bool("replace").unwrap_or(false) {
            return Err("this Hub holds a different creator identity; send replace=true to replace it".into());
        }
    }
    let dir = node.cfg.creator_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)); }
    for (f, v) in [("ed25519_pub.b64", pk), ("ed25519_priv.b64", sk), ("x25519_pub.b64", epk), ("x25519_priv.b64", esk), ("onion_address", id.onion.as_str()), ("display_name", s(args, "display_name"))] {
        write_secret(&dir.join(f), v.trim()).map_err(|e| format!("could not write {}: {}", f, e))?;
    }
    let onion = id.onion.clone();
    node.set_creator(Some(id)).await;
    info!("NOSLOP_HUB: creator identity stored ({})", tor::short(&onion));
    Ok(json!({"status": "ok", "onion_address": onion}))
}

async fn creator_identity_remove(node: &Arc<HubNode>) -> Result<Value, String> {
    node.set_hosting(false).await?;
    let dir = node.cfg.creator_dir();
    if dir.exists() { std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?; }
    node.set_creator(None).await;
    for id in node.store.post_ids().await.map_err(|e| e.to_string())? { let _ = creator::remove_creator_post(node, &id).await; }
    for c in node.store.contacts_for_identity("creator").await.map_err(|e| e.to_string())? {
        let _ = sqlx::query("DELETE FROM nh_contacts WHERE public_key = ?").bind(&c.public_key).execute(&node.store.pool).await;
    }
    info!("NOSLOP_HUB: creator identity, its content and its followers removed from the Hub");
    Ok(json!({"status": "ok"}))
}

async fn content_sync(node: &Arc<HubNode>, args: &JValue) -> Result<Value, String> {
    let creator = node.creator().await.ok_or("no creator identity on this Hub")?;
    let now = node.now();
    let (mut stored, mut rejected, mut ids) = (0, vec![], std::collections::HashSet::new());
    for p in arr(args, "posts") {
        match creator::store_creator_post(node, p, &creator.public_key_b64, now).await {
            Ok(info) => { ids.insert(info.id); stored += 1; }
            Err(e) => rejected.push(json!({"id": p.get_str("id"), "reason": e})),
        }
    }
    // `full`: the phone's complete list of creator posts is this call's posts plus `keep_ids` (a large
    // history is sent in batches, the last one naming every id); anything else was deleted on the phone.
    if args.get_bool("full").unwrap_or(false) {
        for k in arr(args, "keep_ids") { if let Some(id) = k.as_str() { ids.insert(id.to_string()); } }
        for id in node.store.post_ids().await.map_err(|e| e.to_string())? {
            if !ids.contains(&id) { creator::remove_creator_post(node, &id).await?; }
        }
    }
    let (mut comments, mut reactions) = (0, 0);
    for c in arr(args, "comments") {
        match content::verify_comment(c) {
            Ok((id, post_id, author, ts)) if node.store.post_get(&post_id).await.ok().flatten().is_some() => {
                node.store.comment_upsert(&id, &post_id, &author, ts, &c.without_null_members().render()).await.map_err(|e| e.to_string())?;
                comments += 1;
            }
            Ok(_) => {}
            Err(e) => rejected.push(json!({"id": c.get_str("id"), "reason": e})),
        }
    }
    for r in arr(args, "reactions") {
        match content::verify_reaction(r) {
            Ok((id, post_id, author, ts)) if node.store.post_get(&post_id).await.ok().flatten().is_some() => {
                node.store.reaction_upsert(&id, &post_id, &author, ts, &r.without_null_members().render()).await.map_err(|e| e.to_string())?;
                reactions += 1;
            }
            Ok(_) => {}
            Err(e) => rejected.push(json!({"id": r.get_str("id"), "reason": e})),
        }
    }
    // Media the phone still has to upload: every stored post's media that is not complete here.
    let mut needed = vec![];
    for id in node.store.post_ids().await.map_err(|e| e.to_string())? {
        let Some(post) = node.store.post_get(&id).await.ok().flatten() else { continue };
        let Some(mid) = post.media_id else { continue };
        let row = node.store.media_get(&mid).await.ok().flatten();
        if row.as_ref().map(|r| r.complete).unwrap_or(false) { continue; }
        let meta = crate::json::parse(&post.payload).ok().and_then(|v| v.get("media_metadata").cloned());
        needed.push(json!({
            "media_id": mid,
            "sha256": meta.as_ref().and_then(|m| m.get_str("sha256").map(String::from)),
            "received": row.map(|r| r.received).unwrap_or(0),
        }));
    }
    Ok(json!({"status": "ok", "posts": stored, "comments": comments, "reactions": reactions, "rejected": rejected, "media_needed": needed}))
}

/// One chunk of a creator media upload. Chunks must arrive in order (`offset` = bytes already
/// received, which the reply and `creator_media_status` report); the file is checked against its
/// SHA-256 when complete and only then served.
async fn media_put(node: &Arc<HubNode>, args: &JValue) -> Result<Value, String> {
    let media_id = s(args, "media_id");
    if !creator::is_valid_media_id(media_id) { return Err("invalid media id".into()); }
    let sha = s(args, "sha256").to_ascii_lowercase();
    if sha.len() != 64 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) { return Err("sha256 must be 64 hex characters".into()); }
    let size = args.get_i64("size").ok_or("missing size")?;
    if size <= 0 { return Err("bad size".into()); }
    let offset = args.get_i64("offset").ok_or("missing offset")?;
    let data = keys::b64_decode(s(args, "data")).map_err(|_| "data is not Base64")?;
    if data.len() > MEDIA_PUT_MAX { return Err(format!("chunk larger than {} bytes", MEDIA_PUT_MAX)); }
    if node.store.post_ids().await.map_err(|e| e.to_string())?.is_empty() { return Err("no creator posts on this Hub yet".into()); }
    if !node.store.media_in_use(media_id).await.map_err(|e| e.to_string())? { return Err("no stored creator post uses this media".into()); }
    let now = node.now();
    let dir = node.cfg.media_dir();
    tokio::fs::create_dir_all(&dir).await.map_err(|e| e.to_string())?;
    let mut row = node.store.media_get(media_id).await.map_err(|e| e.to_string())?.unwrap_or(super::store::MediaRow {
        media_id: media_id.into(), mtype: String::new(), mime_type: String::new(), size, sha256: sha.clone(), received: 0, complete: false,
    });
    if row.complete && row.sha256 == sha { return Ok(json!({"status": "ok", "received": row.received, "complete": true})); }
    if row.sha256 != sha || row.size != size { row.received = 0; row.complete = false; row.sha256 = sha.clone(); row.size = size; }
    row.mtype = s(args, "type").to_string();
    row.mime_type = args.get_str("mime_type").unwrap_or("application/octet-stream").to_string();
    if offset != row.received { return Ok(json!({"status": "resume", "received": row.received, "complete": false})); }
    if offset + data.len() as i64 > size { return Err("chunk runs past the declared size".into()); }
    let used = node.store.media_total_bytes().await.map_err(|e| e.to_string())? - row.received;
    if used + size > node.cfg.media_quota { return Err("the Hub's creator media quota is full".into()); }
    let part = dir.join(format!("{}.part", media_id));
    let mut f = tokio::fs::OpenOptions::new().create(true).write(true).truncate(offset == 0).open(&part).await.map_err(|e| e.to_string())?;
    if offset > 0 {
        use tokio::io::AsyncSeekExt;
        let len = f.metadata().await.map_err(|e| e.to_string())?.len() as i64;
        if len < offset { row.received = 0; node.store.media_put(&row, now).await.map_err(|e| e.to_string())?; return Ok(json!({"status": "resume", "received": 0, "complete": false})); }
        f.set_len(offset as u64).await.map_err(|e| e.to_string())?;
        f.seek(std::io::SeekFrom::Start(offset as u64)).await.map_err(|e| e.to_string())?;
    }
    f.write_all(&data).await.map_err(|e| e.to_string())?;
    f.flush().await.map_err(|e| e.to_string())?;
    drop(f);
    row.received = offset + data.len() as i64;
    if row.received == size {
        let bytes = tokio::fs::read(&part).await.map_err(|e| e.to_string())?;
        if sha256_hex(&bytes) != sha {
            let _ = tokio::fs::remove_file(&part).await;
            row.received = 0;
            node.store.media_put(&row, now).await.map_err(|e| e.to_string())?;
            return Err("the uploaded file does not match its SHA-256; upload it again".into());
        }
        tokio::fs::rename(&part, dir.join(media_id)).await.map_err(|e| e.to_string())?;
        row.complete = true;
        info!("NOSLOP_HUB: creator media {} stored ({} bytes)", media_id, size);
    }
    node.store.media_put(&row, now).await.map_err(|e| e.to_string())?;
    Ok(json!({"status": "ok", "received": row.received, "complete": row.complete}))
}

/// Older clients: `{packets:[...]}` without destinations. A directed packet goes to the target's
/// stored onion (or the onion its key gives); anything else to every trusted personal contact.
async fn legacy_push(node: &Arc<HubNode>, args: &JValue) -> Result<Value, String> {
    let mut queued = 0;
    for pv in arr(args, "packets") {
        let Ok(pkt) = Packet::from_value(pv) else { continue };
        if observe_outgoing(node, &pkt).await { continue; }
        let retry = retry_for(&pkt.ptype);
        if let Some(t) = pkt.target_user_id.clone() {
            let onion = node.store.contact_get(&t).await.ok().flatten().map(|c| c.onion_address).filter(|o| tor::is_valid_onion(o)).or_else(|| keys::onion_of(&t));
            if let Some(o) = onion { node.deliver(o, pkt, retry); queued += 1; }
        } else {
            for c in node.store.contacts_for_identity("personal").await.map_err(|e| e.to_string())? {
                if c.is_trusted && tor::is_valid_onion(&c.onion_address) { node.deliver(c.onion_address, pkt.clone(), retry); queued += 1; }
            }
        }
    }
    Ok(json!({"status": "ok", "packets_processed": queued}))
}

/// Older clients: everything in the inbox, removed as it is handed out (no ACK).
async fn legacy_pull(node: &Arc<HubNode>) -> Result<Value, String> {
    let (items, _) = node.store.inbox_pull(0, 500, 8 * 1024 * 1024).await.map_err(|e| e.to_string())?;
    let mut packets = vec![];
    let mut last = 0;
    for i in &items {
        last = i.seq;
        if i.kind == "packet" { if let Ok(v) = serde_json::from_str::<Value>(&i.packet) { packets.push(v); } }
    }
    if last > 0 { node.store.inbox_ack(last).await.map_err(|e| e.to_string())?; }
    Ok(json!({"status": "ok", "packets": packets}))
}

// --- Backups ----------------------------------------------------------------------------------

/// Keeps an uploaded backup (already written to `tmp` and checked by the caller) and prunes old ones.
pub async fn backup_store(node: &HubNode, tmp: &std::path::Path, sha256: &str, size: u64) -> Result<Value, String> {
    let dir = node.cfg.backup_dir();
    tokio::fs::create_dir_all(&dir).await.map_err(|e| e.to_string())?;
    let name = format!("noslop_backup_{}.bin", node.now());
    tokio::fs::rename(tmp, dir.join(&name)).await.map_err(|e| e.to_string())?;
    #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; let _ = std::fs::set_permissions(dir.join(&name), std::fs::Permissions::from_mode(0o600)); }
    // Sidecar read by GET /api/backup/pull for the X-Backup-Sha256 header.
    let sidecar = dir.join(&name).with_extension("sha256");
    tokio::fs::write(&sidecar, sha256.as_bytes()).await.map_err(|e| e.to_string())?;
    #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; let _ = std::fs::set_permissions(&sidecar, std::fs::Permissions::from_mode(0o600)); }
    let mut all = backup_list(node).await;
    while all.len() > node.cfg.backup_keep.max(1) {
        let old = dir.join(all.remove(0));
        let _ = tokio::fs::remove_file(old.with_extension("sha256")).await;
        let _ = tokio::fs::remove_file(old).await;
    }
    info!("NOSLOP_HUB: backup stored ({} bytes)", size);
    Ok(json!({"status": "ok", "name": name, "size": size, "sha256": sha256}))
}

/// Backup file names, oldest first.
pub async fn backup_list(node: &HubNode) -> Vec<String> {
    let mut names = vec![];
    if let Ok(mut rd) = tokio::fs::read_dir(node.cfg.backup_dir()).await {
        while let Ok(Some(e)) = rd.next_entry().await {
            let n = e.file_name().to_string_lossy().to_string();
            if n.starts_with("noslop_backup_") && n.ends_with(".bin") { names.push(n); }
        }
    }
    names.sort();
    names
}

pub async fn backup_latest(node: &HubNode) -> Option<std::path::PathBuf> {
    backup_list(node).await.pop().map(|n| node.cfg.backup_dir().join(n))
}
