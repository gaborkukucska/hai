//! START OF FILE hainet-noslop/src/hub/creator.rs
//!
//! What the Hub does for the creator identity while creator hosting is on. Each handler follows
//! the phone's own (named next to it), limited to the creator's own content: a creator node never
//! mirrors other creators (NoSlop-Discovery-Mirroring-Home-HUB-Enhancement-Plan §2, rules 1-2).

use super::{content, store::Contact, tor, HubNode, Retry};
use crate::{canon, json::JValue, keys, packet::Packet};
use std::sync::Arc;
use tracing::{debug, info, warn};

pub(crate) struct Outcome {
    /// Store the packet for the phone, with this inbox kind.
    pub inbox: Option<&'static str>,
    /// Frames to write back on the connection the packet came on.
    pub replies: Vec<String>,
}

impl Outcome {
    fn inbox() -> Self { Outcome { inbox: Some("packet"), replies: vec![] } }
    fn handled() -> Self { Outcome { inbox: None, replies: vec![] } }
}

/// `MediaManager.MAX_CHUNK_BYTES`.
pub const MAX_CHUNK_BYTES: i64 = 256 * 1024;
/// `MediaManager.MAX_OUTBOUND_BYTES_PER_WINDOW` over `OUTBOUND_WINDOW_MS`.
pub(crate) const MEDIA_BYTES_PER_WINDOW_PUB: i64 = 16 * 1024 * 1024;
pub(crate) const MEDIA_WINDOW_MS_PUB: i64 = 60_000;
/// `MediaManager.SIGNED_REQUEST_MAX_SKEW_MS`.
const SIGNED_REQUEST_MAX_SKEW_MS: i64 = 5 * 60_000;
/// `HandshakePacketHandler`: at most 5 auto-accepts per sender per hour.
pub(crate) const AUTO_ACCEPTS_PER_HOUR_PUB: usize = 5;

pub(crate) async fn handle(node: &Arc<HubNode>, pkt: &Packet) -> Outcome {
    match pkt.ptype.as_str() {
        "CONNECTION_REQUEST" => connection_request(node, pkt).await,
        "SYNC_REQUEST" => sync_request(node, pkt).await,
        "INVENTORY_SYNC_REQUEST" => inventory_sync_request(node, pkt).await,
        "MEDIA_REQUEST" => media_request(node, pkt).await,
        "MEDIA_RELAY_REQUEST" => media_relay_request(node, pkt).await,
        "COMMENT" | "EDIT_COMMENT" | "DELETE_COMMENT" | "REACTION" => {
            apply_engagement(node, pkt).await;
            Outcome::inbox()
        }
        _ => Outcome::inbox(),
    }
}

// --- Handshakes -------------------------------------------------------------------------------

/// `HandshakePacketHandler.handleConnectionRequest`, for a request addressed to the creator identity.
async fn connection_request(node: &Arc<HubNode>, pkt: &Packet) -> Outcome {
    let Some(creator) = node.creator().await else { return Outcome::inbox() };
    let Some(p) = pkt.payload.as_ref() else { return Outcome::handled() };
    let (Some(from), Some(ts)) = (p.get_str("from_user_id"), p.get_i64("timestamp")) else { return Outcome::handled() };
    let target = p.get_str("target_user_id").or(pkt.target_user_id.as_deref());
    if target != Some(creator.public_key_b64.as_str()) { return Outcome::inbox(); }
    if pkt.target_user_id.is_some() && pkt.target_user_id.as_deref() != target {
        warn!("NOSLOP_HUB: rejected CONNECTION_REQUEST: envelope target does not match signed target");
        return Outcome::handled();
    }
    // The Hub only auto-accepts a request its sender made itself (not one relayed under another key).
    if pkt.sender_id != from { return Outcome::inbox(); }
    let nonce = p.get_str("request_nonce").unwrap_or("");
    let sig = pkt.signature.as_deref().or(p.get_str("signature")).unwrap_or("");
    let username = p.get_str("from_username").unwrap_or("");
    let home = p.get_str("from_home_node").unwrap_or("");
    let enc = p.get_str("from_encryption_public_key").unwrap_or("");
    let signed = canon::handshake_v2(from, username, home, enc, &creator.public_key_b64, nonce, ts, p.get_str("author_avatar_b64"), p.get_str("bio"), p.get_str("intro"));
    if nonce.trim().is_empty() || !keys::verify(&signed, sig, from) {
        // A legacy or forged request: the phone records "older NoSlop version" for legacy ones.
        return Outcome::inbox();
    }
    let now = node.now();
    let is_old = now - ts > 5 * 60_000;
    let is_very_old = now - ts > 60 * 60_000;
    let is_future = ts - now > 5 * 60_000;
    let existing = node.store.contact_get(from).await.ok().flatten();
    if existing.as_ref().map(|c| c.relationship == "BLOCKED").unwrap_or(false) {
        info!("NOSLOP_HUB: ignored CONNECTION_REQUEST from blocked peer {}", tor::short(from));
        return Outcome::handled();
    }
    let reply_fresh = !is_old && !is_future && node.claim_reply_slot(from, ts).await;

    if let Some(c) = &existing {
        if !c.declined_nonce.is_empty() && c.declined_nonce == nonce && c.relationship == "NONE" {
            let mut replies = vec![];
            if reply_fresh {
                if let Some(w) = node.wire_for(&rejection_packet(&creator, from, now)).await { replies.push(w); }
            }
            info!("NOSLOP_HUB: CONNECTION_REQUEST from {} repeats a request the owner declined", tor::short(from));
            return Outcome { inbox: None, replies };
        }
    }
    if existing.is_none() && is_very_old { return Outcome::handled(); }
    if existing.is_none() {
        if let Ok(Some(deleted_at)) = node.store.meta_get(&format!("deleted_{}", from)).await {
            if deleted_at.parse::<i64>().map(|d| ts <= d && now - d < 7 * 24 * 3_600_000).unwrap_or(false) {
                warn!("NOSLOP_HUB: ignored CONNECTION_REQUEST from recently deleted peer {}", tor::short(from));
                return Outcome::handled();
            }
        }
    }
    let profile = node.profile().await;
    if !profile.is_creator { return Outcome::inbox(); } // creator mode off: the owner decides

    let accepted_creator_contact = existing.as_ref().map(|c| c.relationship == "ACCEPTED" && c.identity == "creator").unwrap_or(false);
    if existing.as_ref().map(|c| c.relationship == "ACCEPTED" && c.identity != "creator").unwrap_or(false) {
        return Outcome::inbox(); // a personal contact addressing the creator identity: the phone decides
    }
    if accepted_creator_contact && is_old { return Outcome::handled(); }
    let crossing = existing.as_ref().map(|c| c.relationship == "OUTGOING_PENDING").unwrap_or(false);
    if !accepted_creator_contact && !crossing && !node.auto_accept_allowed(from, now).await {
        return Outcome::inbox(); // over the auto-accept limit: the owner is asked
    }

    let onion = if keys::is_onion_of(from, home) { home.trim().to_ascii_lowercase() } else { keys::onion_of(from).unwrap_or_default() };
    let contact = Contact {
        public_key: from.to_string(),
        identity: "creator".into(),
        relationship: "ACCEPTED".into(),
        is_trusted: true,
        is_temporary: true,
        handle: username.to_string(),
        onion_address: onion.clone(),
        enc_public_key: if enc.is_empty() { existing.as_ref().map(|c| c.enc_public_key.clone()).unwrap_or_default() } else { enc.to_string() },
        source: existing.as_ref().map(|c| c.source.clone()).unwrap_or_else(|| "hub".into()),
        declined_nonce: String::new(),
    };
    if let Err(e) = node.store.contact_upsert(&contact, now).await { warn!("NOSLOP_HUB: could not store follower: {}", e); return Outcome::inbox(); }
    let hs = handshake_packet(&creator, &profile, from, nonce, now);
    let mut replies = vec![];
    if reply_fresh {
        if let Some(w) = node.wire_for(&hs).await { replies.push(w); }
    }
    if replies.is_empty() && !onion.is_empty() { node.deliver(onion, hs, Retry::Spool); }
    info!("NOSLOP_HUB: auto-accepted follower {} for the creator identity{}", tor::short(from), if replies.is_empty() { "" } else { " (answered on their connection)" });
    Outcome { inbox: Some("creator_follower_accepted"), replies }
}

/// `MeshSocialRepository.acceptConnectionRequest`: the `USER_HANDSHAKE` as the creator identity,
/// echoing the requester's nonce, with the creator profile (never the personal one).
pub(crate) fn handshake_packet(creator: &keys::Identity, profile: &super::CreatorProfile, to: &str, nonce: &str, now: i64) -> Packet {
    let name = if profile.display_name.is_empty() { profile.handle.clone() } else { profile.display_name.clone() };
    let signed = canon::handshake_v2(&creator.public_key_b64, &name, &creator.onion, &creator.enc_public_key_b64, to, nonce, now, profile.avatar_b64.as_deref(), profile.bio.as_deref(), None);
    let sig = creator.sign(&signed);
    let payload = JValue::obj()
        .with("id", JValue::str(uuid::Uuid::new_v4().to_string()))
        .with("from_user_id", JValue::str(&creator.public_key_b64))
        .with("from_username", JValue::str(&name))
        .with("from_display_name", JValue::str(&name))
        .with_opt("author_avatar_b64", profile.avatar_b64.clone().map(JValue::Str))
        .with_opt("bio", profile.bio.clone().map(JValue::Str))
        .with("from_home_node", JValue::str(&creator.onion))
        .with("from_encryption_public_key", JValue::str(&creator.enc_public_key_b64))
        .with("timestamp", JValue::int(now))
        .with("in_reply_to_nonce", JValue::str(nonce))
        .with("target_user_id", JValue::str(to))
        .with("version", JValue::int(2));
    let mut p = Packet::new("USER_HANDSHAKE", &creator.public_key_b64, Some(to), payload, 3);
    p.signature = Some(sig);
    p
}

/// `buildConnectionRejectedPacket`.
fn rejection_packet(creator: &keys::Identity, to: &str, now: i64) -> Packet {
    let sig = creator.sign(&canon::connection_rejected(&creator.public_key_b64, now));
    let payload = JValue::obj()
        .with("from_user_id", JValue::str(&creator.public_key_b64))
        .with("timestamp", JValue::int(now))
        .with("signature", JValue::str(&sig));
    let mut p = Packet::new("CONNECTION_REJECTED", &creator.public_key_b64, Some(to), payload, 3);
    p.signature = Some(sig);
    p
}

// --- Sync -------------------------------------------------------------------------------------

/// The requester when it is one of the creator identity's contacts (the phone answers only peers it
/// knows); `None` otherwise.
async fn creator_contact(node: &HubNode, key: &str) -> Option<Contact> {
    let c = node.store.contact_get(key).await.ok().flatten()?;
    if c.relationship == "BLOCKED" || c.identity != "creator" || !tor::is_valid_onion(&c.onion_address) { return None; }
    Some(c)
}

/// `SyncPacketHandler.handleSyncRequest`, for the creator identity: the creator's public posts since
/// `since`, then the comments and reactions on them, in batches of 25, each its own `SYNC_RESPONSE`.
async fn sync_request(node: &Arc<HubNode>, pkt: &Packet) -> Outcome {
    let Some(contact) = creator_contact(node, &pkt.sender_id).await else { return Outcome::inbox() };
    let since = pkt.p_i64("since").unwrap_or(0);
    let Some(creator) = node.creator().await else { return Outcome::inbox() };
    let all = node.store.posts_since(&creator.public_key_b64, 0).await.unwrap_or_default();
    let posts: Vec<_> = all.iter().filter(|p| p.timestamp >= since).collect();
    let ids: Vec<String> = all.iter().map(|p| p.id.clone()).collect();
    let comments = node.store.comments_for_posts_since(&ids, since).await.unwrap_or_default();
    let reactions = node.store.reactions_for_posts_since(&ids, since).await.unwrap_or_default();
    let post_values: Vec<JValue> = posts.iter().filter_map(|p| crate::json::parse(&p.payload).ok()).collect();
    info!("NOSLOP_HUB: SYNC_REQUEST from follower {} — {} posts, {} comments, {} reactions", tor::short(&pkt.sender_id), post_values.len(), comments.len(), reactions.len());
    send_sync_batches(node, &creator, &contact, post_values, parse_all(&comments), parse_all(&reactions));
    Outcome::handled()
}

/// `SyncPacketHandler.handleInventorySyncRequest`, for the creator identity: posts the requester is
/// missing or holds in another version, and the engagement of the last year on every creator post.
async fn inventory_sync_request(node: &Arc<HubNode>, pkt: &Packet) -> Outcome {
    let Some(contact) = creator_contact(node, &pkt.sender_id).await else { return Outcome::inbox() };
    let Some(creator) = node.creator().await else { return Outcome::inbox() };
    let mut theirs = std::collections::HashMap::new();
    if let Some(JValue::Arr(items)) = pkt.p("inventory") {
        for it in items.iter().take(100_000) {
            if let (Some(id), Some(h)) = (it.get_str("id"), it.get_str("hash")) { theirs.insert(id.to_string(), h.to_string()); }
        }
    }
    let all = node.store.posts_since(&creator.public_key_b64, 0).await.unwrap_or_default();
    let missing: Vec<JValue> = all.iter()
        .filter(|p| theirs.get(&p.id).map(|h| *h != content::inventory_hash(&p.id, &p.author_id, &p.content, p.timestamp)).unwrap_or(true))
        .filter_map(|p| crate::json::parse(&p.payload).ok()).collect();
    let ids: Vec<String> = all.iter().map(|p| p.id.clone()).collect();
    let cutoff = node.now() - 365 * 24 * 3_600_000;
    let comments = node.store.comments_for_posts_since(&ids, cutoff).await.unwrap_or_default();
    let reactions = node.store.reactions_for_posts_since(&ids, cutoff).await.unwrap_or_default();
    info!("NOSLOP_HUB: INVENTORY_SYNC_REQUEST from follower {} — {} of {} posts missing, {} comments, {} reactions", tor::short(&pkt.sender_id), missing.len(), all.len(), comments.len(), reactions.len());
    send_sync_batches(node, &creator, &contact, missing, parse_all(&comments), parse_all(&reactions));
    Outcome::handled()
}

fn parse_all(rows: &[String]) -> Vec<JValue> { rows.iter().filter_map(|r| crate::json::parse(r).ok()).collect() }

fn send_sync_batches(node: &Arc<HubNode>, creator: &keys::Identity, contact: &Contact, posts: Vec<JValue>, comments: Vec<JValue>, reactions: Vec<JValue>) {
    let node = node.clone();
    let sender = creator.public_key_b64.clone();
    let (to, onion) = (contact.public_key.clone(), contact.onion_address.clone());
    tokio::spawn(async move {
        let mut batches = vec![];
        for b in posts.chunks(25) { batches.push((b.to_vec(), vec![], vec![])); }
        for b in comments.chunks(25) { batches.push((vec![], b.to_vec(), vec![])); }
        for b in reactions.chunks(25) { batches.push((vec![], vec![], b.to_vec())); }
        for (p, c, r) in batches {
            let payload = JValue::obj().with("posts", JValue::Arr(p)).with("comments", JValue::Arr(c)).with("reactions", JValue::Arr(r));
            let pkt = Packet::new("SYNC_RESPONSE", &sender, Some(&to), payload, 3);
            node.deliver(onion.clone(), pkt, Retry::Sync);
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    });
}

// --- Media ------------------------------------------------------------------------------------

/// `MediaManager.isValidMediaId`.
pub fn is_valid_media_id(id: &str) -> bool {
    if id.is_empty() || id.len() > 128 || id.contains("..") || id.contains('/') || id.contains('\\') { return false; }
    let b = id.as_bytes();
    let ok = |c: u8, first: bool| c.is_ascii_alphanumeric() || c == b'_' || c == b'-' || (!first && c == b'.');
    ok(b[0], true) && b[1..].iter().all(|c| ok(*c, false))
}

/// `MediaManager.replyOnionFor`: a known contact's stored onion, or — for a stranger — the onion of
/// its key, only for a request signed by that key, fresh, and not seen before.
async fn reply_onion_for(node: &HubNode, pkt: &Packet) -> Option<String> {
    if let Ok(Some(c)) = node.store.contact_get(&pkt.sender_id).await {
        if c.relationship == "BLOCKED" { return None; }
        if tor::is_valid_onion(&c.onion_address) { return Some(c.onion_address); }
    }
    let p = pkt.payload.as_ref()?;
    let sig = pkt.signature.as_deref()?;
    let ts = p.get_i64("timestamp")?;
    if (node.now() - ts).abs() > SIGNED_REQUEST_MAX_SKEW_MS { return None; }
    let signed = canon::media_request(&pkt.sender_id, pkt.target_user_id.as_deref(), p.get_str("media_id")?, p.get_i64("chunk_index")?, p.get_i64("chunk_size")?, p.get_i64("byte_offset"), p.get_i64("byte_length"), Some(ts));
    if !keys::verify(&signed, sig, &pkt.sender_id) { return None; }
    if !node.remember_stranger_request(sig).await { return None; }
    keys::onion_of(&pkt.sender_id)
}

/// The Hub only holds the creator's public post media, and serves a file only while a stored
/// creator post uses it (`MediaManager.isAuthorizedByOwnerIndex`: public post media → anyone not blocked).
async fn media_servable(node: &HubNode, media_id: &str) -> Option<super::store::MediaRow> {
    let row = node.store.media_get(media_id).await.ok().flatten()?;
    if !row.complete || !node.store.media_in_use(media_id).await.unwrap_or(false) { return None; }
    Some(row)
}

/// `MediaManager.handleMediaRequest` for media held on the Hub. Anything the Hub doesn't hold goes
/// to the phone, which may still have it.
async fn media_request(node: &Arc<HubNode>, pkt: &Packet) -> Outcome {
    let Some(p) = pkt.payload.as_ref() else { return Outcome::handled() };
    let Some(media_id) = p.get_str("media_id").filter(|m| is_valid_media_id(m)) else { return Outcome::handled() };
    let Some(row) = media_servable(node, media_id).await else { return Outcome::inbox() };
    let Some(onion) = reply_onion_for(node, pkt).await else {
        warn!("NOSLOP_HUB: not answering MEDIA_REQUEST for {} from {}: unknown requester and no valid signature", media_id, tor::short(&pkt.sender_id));
        return Outcome::handled();
    };
    let Some(creator) = node.creator().await else { return Outcome::handled() };
    let chunk_size = p.get_i64("chunk_size").unwrap_or(-1);
    let chunk_index = p.get_i64("chunk_index").unwrap_or(-1);
    let byte_offset = p.get_i64("byte_offset");
    let byte_length = p.get_i64("byte_length");
    let is_metadata = chunk_size == 0 && byte_length.map(|l| l == 0).unwrap_or(true);
    if is_metadata {
        let meta = JValue::obj()
            .with("id", JValue::str(&row.media_id))
            .with("type", JValue::str(&row.mtype))
            .with("mime_type", JValue::str(&row.mime_type))
            .with("size", JValue::int(row.size))
            .with("chunk_count", JValue::int(999))
            .with("sha256", JValue::str(&row.sha256));
        node.deliver(onion, Packet::new("MEDIA_METADATA_RESPONSE", &creator.public_key_b64, Some(&pkt.sender_id), meta, 1), Retry::Once);
        return Outcome::handled();
    }
    // MediaManager.chunkRequestRejection
    let req_len = byte_length.unwrap_or(chunk_size);
    if !(1..=MAX_CHUNK_BYTES).contains(&chunk_size) || !(1..=MAX_CHUNK_BYTES).contains(&req_len) || byte_offset.map(|o| o < 0).unwrap_or(false) || chunk_index < 0 {
        warn!("NOSLOP_HUB: rejected MEDIA_REQUEST from {}: bad range", tor::short(&pkt.sender_id));
        return Outcome::handled();
    }
    let total = row.size;
    let offset = byte_offset.unwrap_or(chunk_index.saturating_mul(chunk_size));
    let actual = if offset >= total || offset < 0 { 0 } else { req_len.min(total - offset).clamp(0, MAX_CHUNK_BYTES) };
    let node2 = node.clone();
    let sender = pkt.sender_id.clone();
    let media_id = media_id.to_string();
    tokio::spawn(async move {
        if !node2.admit_media_bytes(&sender, actual).await {
            warn!("NOSLOP_HUB: outbound media limit still exceeded for {}; dropping {}@{}", tor::short(&sender), media_id, offset);
            return;
        }
        let mut data = vec![0u8; actual as usize];
        if actual > 0 {
            use tokio::io::{AsyncReadExt, AsyncSeekExt};
            let path = node2.cfg.media_dir().join(&media_id);
            let read = async {
                let mut f = tokio::fs::File::open(&path).await?;
                f.seek(std::io::SeekFrom::Start(offset as u64)).await?;
                f.read_exact(&mut data).await
            }.await;
            if let Err(e) = read { warn!("NOSLOP_HUB: read error for {}: {}", media_id, e); return; }
        }
        let total_chunks = if byte_offset.is_none() { total / chunk_size + 1 } else { 999 };
        let payload = JValue::obj()
            .with("media_id", JValue::str(&media_id))
            .with("chunk_index", JValue::int(chunk_index))
            .with("total_chunks", JValue::int(total_chunks))
            .with("byte_offset", JValue::int(offset))
            .with("total_size", JValue::int(total))
            .with("data", JValue::str(keys::b64_encode(&data)));
        let out = Packet::new("MEDIA_CHUNK", &creator.public_key_b64, Some(&sender), payload, 1);
        node2.deliver_now(&onion, out, Retry::Once).await;
    });
    Outcome::handled()
}

/// `MediaManager.handleRecoveryRequest`: "do you hold this media?" from a direct contact.
async fn media_relay_request(node: &Arc<HubNode>, pkt: &Packet) -> Outcome {
    let Some(media_id) = pkt.p_str("media_id").filter(|m| is_valid_media_id(m)) else { return Outcome::handled() };
    if media_servable(node, media_id).await.is_none() { return Outcome::inbox(); }
    let Some(contact) = creator_contact(node, &pkt.sender_id).await else { return Outcome::handled() };
    let Some(creator) = node.creator().await else { return Outcome::handled() };
    let payload = JValue::obj().with("media_id", JValue::str(media_id)).with("onion_address", JValue::str(&creator.onion));
    node.deliver(contact.onion_address, Packet::new("MEDIA_RECOVERY_FOUND", &creator.public_key_b64, Some(&pkt.sender_id), payload, 1), Retry::Once);
    Outcome::handled()
}

// --- Content kept on the Hub --------------------------------------------------------------------

/// Applies a creator post change (`POST` / `EDIT_POST` / `DELETE_POST` written by the creator
/// identity, seen when the phone hands it over) to the Hub's copy.
pub(crate) async fn apply_creator_post_packet(node: &HubNode, pkt: &Packet) {
    let Some(creator) = node.creator().await else { return };
    let Some(p) = pkt.payload.as_ref() else { return };
    let now = node.now();
    match pkt.ptype.as_str() {
        "POST" => {
            if p.get_str("author_id") != Some(creator.public_key_b64.as_str()) { return; }
            if let Err(e) = store_creator_post(node, p, &creator.public_key_b64, now).await { debug!("NOSLOP_HUB: creator POST not kept: {}", e); }
        }
        "EDIT_POST" => {
            let (Some(post_id), Some(author)) = (p.get_str("post_id"), p.get_str("author_id")) else { return };
            if author != creator.public_key_b64 { return; }
            let Ok(Some(stored)) = node.store.post_get(post_id).await else { return };
            // PostPacketHandler.handleEditPost: applies when timestamp >= the stored post's.
            if p.get_i64("timestamp").map(|t| t < stored.timestamp).unwrap_or(true) { return; }
            let Ok(mut merged) = crate::json::parse(&stored.payload) else { return };
            // The edit carries the complete signed state (WIRE_PROTOCOL_REFERENCE §3 EDIT_POST).
            for k in ["content", "timestamp", "signature"] {
                match p.get(k) { Some(v) if !v.is_null() => { merged.set(k, v.clone()); } _ => return }
            }
            merged.set("privacy", JValue::str(p.get_str("privacy").unwrap_or("public")));
            for k in ["author_avatar_b64", "media_id", "media_metadata", "clearnet_url"] {
                match p.get(k) { Some(v) if !v.is_null() => { merged.set(k, v.clone()); } _ => { merged.remove(k); } }
            }
            if let Err(e) = store_creator_post(node, &merged, &creator.public_key_b64, now).await { debug!("NOSLOP_HUB: creator EDIT_POST not applied: {}", e); }
        }
        "DELETE_POST" => {
            let (Some(post_id), Some(author), Some(ts), Some(sig)) = (p.get_str("post_id"), p.get_str("author_id"), p.get_i64("timestamp"), p.get_str("signature")) else { return };
            if author != creator.public_key_b64 { return; }
            let ok = keys::verify(&canon::efs(&[post_id, author, &ts.to_string()]), sig, author) || keys::verify(&format!("{}|{}|{}", post_id, author, ts), sig, author);
            if ok { let _ = remove_creator_post(node, post_id).await; info!("NOSLOP_HUB: creator post {} removed (DELETE_POST)", post_id); }
        }
        _ => {}
    }
}

/// Verifies and keeps one creator post: public, written by the creator identity, signed.
pub(crate) async fn store_creator_post(node: &HubNode, p: &JValue, creator_pub: &str, now: i64) -> Result<content::PostInfo, String> {
    let info = content::verify_post(p)?;
    if info.author_id != creator_pub { return Err("not written by the creator identity".into()); }
    if info.privacy != "public" { return Err("only public posts are hosted".into()); }
    if let Some(m) = &info.media_id { if !is_valid_media_id(m) { return Err("invalid media id".into()); } }
    // origin_node is unsigned and only meaningful from the author's own node: dropped, as the phone's sync does.
    let mut kept = p.without_null_members();
    kept.remove("origin_node");
    node.store.post_upsert(&super::store::StoredPost {
        id: info.id.clone(), author_id: info.author_id.clone(), timestamp: info.timestamp, content: info.content.clone(),
        media_id: info.media_id.clone(), payload: kept.render(),
    }, now).await.map_err(|e| e.to_string())?;
    Ok(info)
}

pub(crate) async fn remove_creator_post(node: &HubNode, id: &str) -> Result<(), String> {
    let media = node.store.post_get(id).await.map_err(|e| e.to_string())?.and_then(|p| p.media_id);
    node.store.post_remove(id).await.map_err(|e| e.to_string())?;
    if let Some(m) = media { node.drop_unused_media(&m).await; }
    Ok(())
}

/// `CommentPacketHandler` / `ReactionPacketHandler` receiver rules (WIRE_PROTOCOL_REFERENCE §7.1)
/// for engagement on the creator's posts: verified, same author for edits and deletes, newer wins.
pub(crate) async fn apply_engagement(node: &HubNode, pkt: &Packet) {
    let Some(p) = pkt.payload.as_ref() else { return };
    let Some(post_id) = p.get_str("post_id") else { return };
    if node.store.post_get(post_id).await.ok().flatten().is_none() { return; }
    match pkt.ptype.as_str() {
        "COMMENT" => {
            let Some(sync) = content::comment_payload_to_sync(p) else { return };
            let Ok((id, _, author, ts)) = content::verify_comment(&sync) else { debug!("NOSLOP_HUB: comment signature does not verify"); return };
            if let Ok(Some((_, a, old_ts))) = node.store.comment_get(&id).await { if a != author || ts <= old_ts { return; } }
            let _ = node.store.comment_upsert(&id, post_id, &author, ts, &sync.render()).await;
        }
        "EDIT_COMMENT" => {
            let (Some(cid), Some(author), Some(text), Some(ts), Some(sig)) = (p.get_str("comment_id"), p.get_str("author_id"), p.get_str("content"), p.get_i64("timestamp"), p.get_str("signature")) else { return };
            let Ok(Some((_, a, old_ts))) = node.store.comment_get(cid).await else { return };
            if a != author || ts <= old_ts { return; }
            let avatar = p.get_str("author_avatar_b64");
            if !content::comment_signature_ok(post_id, cid, text, ts, avatar, None, sig, author) { return; }
            let rows = node.store.comments_for_posts_since(&[post_id.to_string()], 0).await.unwrap_or_default();
            if let Some(mut v) = rows.iter().filter_map(|r| crate::json::parse(r).ok()).find(|v| v.get_str("id") == Some(cid)) {
                v.set("content", JValue::str(text)).set("timestamp", JValue::int(ts)).set("signature", JValue::str(sig));
                match avatar { Some(a) => { v.set("author_avatar_b64", JValue::str(a)); } None => { v.remove("author_avatar_b64"); } }
                v.remove("media_metadata"); // the edit signature doesn't cover a media digest
                let _ = node.store.comment_upsert(cid, post_id, author, ts, &v.render()).await;
            }
        }
        "DELETE_COMMENT" => {
            let (Some(cid), Some(author), Some(ts), Some(sig)) = (p.get_str("comment_id"), p.get_str("author_id"), p.get_i64("timestamp"), p.get_str("signature")) else { return };
            let ok = keys::verify(&canon::efs(&[post_id, cid, author, &ts.to_string()]), sig, author) || keys::verify(&format!("{}|{}|{}|{}", post_id, cid, author, ts), sig, author);
            if !ok { return; }
            if let Ok(Some((_, a, _))) = node.store.comment_get(cid).await { if a == author { let _ = node.store.comment_delete(cid).await; } }
        }
        "REACTION" => {
            let (Some(rtype), Some(author), Some(ts), Some(sig)) = (p.get_str("reaction_type"), p.get_str("author_id"), p.get_i64("timestamp"), p.get_str("signature")) else { return };
            if !content::reaction_signature_ok(post_id, rtype, author, ts, sig) { return; }
            let id = content::reaction_id(post_id, author, rtype);
            if node.store.reaction_timestamp(&id).await.ok().flatten().map(|old| ts <= old).unwrap_or(false) { return; }
            if p.get_str("action") == Some("remove") {
                let _ = node.store.reaction_delete(&id).await;
            } else {
                let row = JValue::obj().with("id", JValue::str(&id)).with("post_id", JValue::str(post_id)).with("author_id", JValue::str(author))
                    .with("reaction_type", JValue::str(rtype)).with("timestamp", JValue::int(ts)).with("signature", JValue::str(sig));
                let _ = node.store.reaction_upsert(&id, post_id, author, ts, &row.render()).await;
            }
        }
        _ => {}
    }
}

// --- Presence ---------------------------------------------------------------------------------

/// `MeshSocialRepository.announceCreatorPresence`: the creator's `ANNOUNCE_PEER` to the contacts that
/// know it, and, when discoverable, its `ANNOUNCE_DISCOVERABLE` to every trusted contact (to personal
/// contacts in relay form, one hop less, keeping the creator as sender, as `GossipService.outboundFor`
/// sends it).
pub(crate) async fn announce_presence(node: &Arc<HubNode>) {
    let (Some(creator), profile) = (node.creator().await, node.profile().await) else { return };
    let now = node.now();
    let contacts = node.store.contacts_all().await.unwrap_or_default();
    let sig = creator.sign(&canon::announce_peer_v2(&creator.public_key_b64, Some(&creator.onion), now));
    let announce = JValue::obj()
        .with("author_id", JValue::str(&creator.public_key_b64))
        .with("onion_address", JValue::str(&creator.onion))
        .with("timestamp", JValue::int(now))
        .with("signature", JValue::str(&sig));
    let disc = if profile.is_discoverable {
        let handle = if profile.handle.is_empty() { profile.display_name.clone() } else { profile.handle.clone() };
        let dsig = creator.sign(&canon::discoverable(&creator.public_key_b64, &handle, &creator.onion, &creator.enc_public_key_b64, profile.is_creator, profile.fund_me_link.as_deref(), None, profile.bio.as_deref(), now));
        Some((JValue::obj()
            .with("author_id", JValue::str(&creator.public_key_b64))
            .with("handle", JValue::str(&handle))
            .with("onion_address", JValue::str(&creator.onion))
            .with("enc_public_key", JValue::str(&creator.enc_public_key_b64))
            .with("is_creator", JValue::Bool(profile.is_creator))
            .with_opt("fund_me_link", profile.fund_me_link.clone().map(JValue::Str))
            .with_opt("bio", profile.bio.clone().map(JValue::Str))
            .with("timestamp", JValue::int(now))
            .with("signature", JValue::str(&dsig)), dsig))
    } else { None };
    let mut sent = 0;
    for c in contacts.iter().filter(|c| c.is_trusted && c.relationship == "ACCEPTED" && tor::is_valid_onion(&c.onion_address)) {
        if node.in_cooldown(&c.onion_address).await { continue; }
        let knows_creator = c.identity == "creator";
        if knows_creator {
            let mut p = Packet::new("ANNOUNCE_PEER", &creator.public_key_b64, None, announce.clone(), 3);
            p.signature = Some(sig.clone());
            node.deliver(c.onion_address.clone(), p, Retry::Once);
            sent += 1;
        }
        if let Some((payload, dsig)) = &disc {
            let mut p = Packet::new("ANNOUNCE_DISCOVERABLE", &creator.public_key_b64, None, payload.clone(), if knows_creator { 4 } else { 3 });
            p.signature = Some(dsig.clone());
            node.deliver(c.onion_address.clone(), p, Retry::Once);
        }
    }
    debug!("NOSLOP_HUB: creator presence sent to {} followers{}", sent, if disc.is_some() { " (+ discoverable announcement)" } else { "" });
}
