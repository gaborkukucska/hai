//! START OF FILE hainet-noslop/src/hub/dms.rs
//!
//! NOSLOP_HUB_DM_VIEW_V1 — the Hub opens the owner's one-to-one DMs for its own Web UI.
//!
//! The Hub holds the keys of both of the owner's identities (personal, and creator while it hosts it), so it
//! can read the DMs that pass through it: those that reach its onions for the phone (stored for the phone
//! unchanged, as before) and those the phone sends out through `hub_send`. Each one it can open (DM v2,
//! `CryptoService.decryptDMV2`, the same keys and AAD the phone uses) goes to [`DmHook`], which `hainet-core`
//! writes to the Web UI's `dms` table; an incoming one also becomes a notification event (`nh_dm_events`).
//! Before, the Web UI only had the plaintext copies the phone pushed (`sync_push_dms`), so a DM showed there
//! only after the phone had pulled, decrypted and copied it.
//!
//! Not opened: group messages (keys come from a group directory the Hub does not keep), the Admin AI's
//! conversation (handled by `admin`), and legacy v1 DMs (unauthenticated; the phone refuses them from any
//! sender that speaks v2, and the Hub does not know which senders those are).

use super::{BoxFut, HubNode};
use crate::{dm, json, packet::Packet};
use std::sync::Arc;
use tracing::debug;

/// One DM as the Web UI shows it (the `dms` table's columns, as `sync_push_dms` fills them).
#[derive(Debug, Clone, PartialEq)]
pub struct DmRecord {
    pub id: String,
    /// The contact's public key: the conversation.
    pub peer: String,
    /// Who wrote it: the contact, or one of the owner's identities.
    pub sender: String,
    pub content: String,
    pub timestamp: i64,
    pub media_id: Option<String>,
    pub media_type: Option<String>,
    /// True when the contact wrote it (a notification), false when the owner did.
    pub incoming: bool,
    /// NOSLOP_HUB_DM_MEDIA_V1: the owner's identity in this conversation, and the attachment's full description
    /// (`MediaMetadata`) when there is one.
    pub me: String,
    pub media: Option<json::JValue>,
}

pub type DmHook = Arc<dyn Fn(DmRecord) -> BoxFut<()> + Send + Sync>;

/// NOSLOP_HUB_DM_DELETE_V1 — what the Web UI's chat must forget. The hook returns how many messages went.
#[derive(Debug, Clone, PartialEq)]
pub enum DmRemoval {
    /// One message, deleted by its author (`DELETE_MESSAGE`).
    Message { id: String, sender: String },
    /// A conversation the owner cleared on the phone, up to `before` (ms).
    Conversation { peer: String, before: i64 },
}

pub type DmRemoveHook = Arc<dyn Fn(DmRemoval) -> BoxFut<u64> + Send + Sync>;

fn cleared_key(peer: &str) -> String { format!("dm_cleared_before_{}", peer) }

/// When the owner last cleared the conversation with `peer` (0: never).
pub async fn cleared_before(node: &Arc<HubNode>, peer: &str) -> i64 {
    node.store.meta_get(&cleared_key(peer)).await.ok().flatten().and_then(|v| v.parse().ok()).unwrap_or(0)
}

/// `hub_dms_clear`: forgets the conversation with `peer` up to `before`, and ignores older copies from now on
/// (the phone's `NOSLOP_DM_CLEAR_STICKS_V1`; a contact may re-send what was cleared).
/// Returns how many messages went and the cleared-up-to time kept (never earlier than an earlier clear).
pub async fn clear_conversation(node: &Arc<HubNode>, peer: &str, before: i64) -> Result<(u64, i64), String> {
    let before = before.max(cleared_before(node, peer).await);
    node.store.meta_set(&cleared_key(peer), &before.to_string()).await.map_err(|e| e.to_string())?;
    node.store.dm_events_remove_peer(peer).await.map_err(|e| e.to_string())?;
    let hook = node.dm_remove_hook.read().await.clone();
    let deleted = match hook { Some(h) => h(DmRemoval::Conversation { peer: peer.to_string(), before }).await, None => 0 };
    super::dm_media::forget_conversation(node, peer, before).await;
    if let Err(e) = node.store.outbox_remove_peer(peer, before).await { debug!("NOSLOP_HUB: outbox of a cleared chat not emptied: {}", e); }
    Ok((deleted, before))
}

/// Applies a one-to-one `DELETE_MESSAGE` to the Web UI's chat: from a contact deleting their own message
/// (`incoming`), or from the owner deleting theirs (`hub_send`). Checked as `DmPacketHandler.handleDeleteMessage`
/// does: the author is the sender, and signed `encodeForSigning(message_id, author_id, timestamp)` (or the older
/// `id|author|ts` text) by the author.
pub async fn observe_delete(node: &Arc<HubNode>, pkt: &Packet, incoming: bool) {
    if pkt.ptype != "DELETE_MESSAGE" { return; }
    let Some(p) = pkt.payload.as_ref() else { return };
    if p.get_str("group_id").map_or(false, |g| !g.is_empty()) { return; }
    let (Some(id), Some(author), Some(ts)) = (p.get_str("message_id"), p.get_str("author_id"), p.get_i64("timestamp")) else { return };
    let Some(sig) = p.get_str("signature").or(pkt.signature.as_deref()) else { return };
    if author != pkt.sender_id { return; }
    let personal = node.personal().await;
    let creator = node.creator().await;
    let is_ours = [&personal, &creator].into_iter().flatten().any(|i| i.public_key_b64 == author);
    if is_ours == incoming { return; } // a contact deletes theirs when incoming; the owner deletes theirs when outgoing
    let ts_s = ts.to_string();
    let ok = crate::keys::verify(&crate::canon::efs(&[id, author, &ts_s]), sig, author)
        || crate::keys::verify(&format!("{}|{}|{}", id, author, ts_s), sig, author);
    if !ok {
        debug!("NOSLOP_HUB: DELETE_MESSAGE for {} does not verify; ignored", id);
        return;
    }
    if let Err(e) = node.store.dm_event_remove(id).await { debug!("NOSLOP_HUB: DM event not removed: {}", e); }
    super::dm_media::forget_dm(node, id).await;
    if let Err(e) = node.store.outbox_remove(id).await { debug!("NOSLOP_HUB: outbox entry not removed: {}", e); }
    if let Some(h) = node.dm_remove_hook.read().await.clone() {
        h(DmRemoval::Message { id: id.to_string(), sender: author.to_string() }).await;
    }
}

/// Longest text kept in a notification event.
pub const PREVIEW_CHARS: usize = 140;

/// Opens `pkt` when it is a one-to-one DM between one of the owner's identities and a contact, and hands it
/// on. `incoming`: the packet reached the Hub from the mesh (true) or the phone is sending it (false).
pub async fn observe(node: &Arc<HubNode>, pkt: &Packet, incoming: bool) {
    if pkt.ptype != "MESSAGE" { return; }
    let Some(rec) = open(node, pkt, incoming).await else { return };
    // NOSLOP_HUB_DM_DELETE_V1: nothing from before the owner cleared this conversation comes back.
    let cleared = cleared_before(node, &rec.peer).await;
    if cleared > 0 && rec.timestamp <= cleared { return; }
    // NOSLOP_HUB_DM_MEDIA_V1: the attachment is kept for the Web UI (fetched from the contact, or uploaded by the phone).
    if let Some(d) = rec.media.as_ref().and_then(super::dm_media::describe) {
        super::dm_media::register(node, d, &rec.id, rec.timestamp, &rec.peer, &rec.me, incoming).await;
    }
    if incoming {
        let preview = preview(&rec);
        if let Err(e) = node.store.dm_event_add(&rec.id, &rec.peer, &rec.sender, &preview, node.now()).await {
            debug!("NOSLOP_HUB: DM event not recorded: {}", e);
        }
    }
    if let Some(hook) = node.dm_hook.read().await.clone() {
        hook(rec).await;
    }
}

/// The DM in `pkt`, or None when it is not one the Hub can open.
pub async fn open(node: &Arc<HubNode>, pkt: &Packet, incoming: bool) -> Option<DmRecord> {
    let p = pkt.payload.as_ref()?;
    if p.get_str("group_id").map_or(false, |g| !g.is_empty()) { return None; }
    if pkt.sender_id.starts_with("admin_") || pkt.target_user_id.as_deref().map_or(false, |t| t.starts_with("admin_")) {
        return None;
    }
    let id = p.get_str("id")?;
    let ct = p.get_str("ciphertext")?;
    let nonce = p.get_str("nonce")?;
    let ts = p.get_i64("timestamp")?; // v2 always carries it (it is in the AAD)
    let personal = node.personal().await;
    let creator = node.creator().await;
    let ours = |k: &str| [&personal, &creator].into_iter().flatten().find(|i| i.public_key_b64 == k).cloned();

    let (peer, candidates) = if incoming {
        // Addressed identity first, then the other one (DmPacketHandler.handleDirectMessage).
        let mut c: Vec<_> = Vec::new();
        match pkt.target_user_id.as_deref() {
            Some(t) => c.extend(ours(t)),
            None => c.extend(personal.clone()),
        }
        if c.is_empty() { return None; } // addressed to someone else: not ours to read
        for other in [&personal, &creator].into_iter().flatten() {
            if !c.iter().any(|i| i.public_key_b64 == other.public_key_b64) { c.push(other.clone()); }
        }
        (pkt.sender_id.clone(), c)
    } else {
        let me = ours(&pkt.sender_id)?;
        (pkt.target_user_id.clone()?, vec![me])
    };
    if ours(&peer).is_some() { return None; }
    let contact = node.store.contact_get(&peer).await.ok().flatten()?;
    let their_enc = if contact.enc_public_key.is_empty() { contact.public_key.clone() } else { contact.enc_public_key.clone() };

    for me in candidates {
        let (sender, recipient) = if incoming { (peer.as_str(), me.public_key_b64.as_str()) } else { (me.public_key_b64.as_str(), peer.as_str()) };
        let secret = me.x25519_secret();
        if let Ok(plain) = dm::decrypt_v2(ct, nonce, &their_enc, &secret, sender, recipient, id, None, ts, dm::DM_AAD_MESSAGE) {
            let (content, media_id, media_type) = parse_body(&plain);
            let media = json::parse(&plain).ok().and_then(|v| v.get("media").filter(|m| matches!(m, json::JValue::Obj(_))).cloned());
            return Some(DmRecord {
                id: id.to_string(),
                peer: peer.clone(),
                sender: sender.to_string(),
                content,
                timestamp: ts,
                media_id,
                media_type,
                incoming,
                me: me.public_key_b64.clone(),
                media,
            });
        }
    }
    debug!("NOSLOP_HUB: DM {} with {} could not be opened here", id, super::tor::short(&peer));
    None
}

/// NOSLOP_HUB_WEBUI_SEND_V1 — a DM typed in the Web UI, sent by the Hub itself (DM v2) from the identity the
/// contact knows (`nh_contacts.identity`, as the phone's `IdentityRouter` reports it): a creator follower only
/// accepts DMs from the creator identity ("Dropping DM …: no X25519 key on record"). Before, the Web UI only
/// stored the text, and the phone re-sent it later, always from the personal identity.
pub async fn send_from_web_ui(node: &Arc<HubNode>, peer: &str, text: &str) -> Result<DmRecord, String> {
    if text.is_empty() { return Err("empty message".into()); }
    if text.len() > MAX_WEB_UI_DM_BYTES { return Err("message too long".into()); }
    let contact = node.store.contact_get(peer).await.map_err(|e| e.to_string())?
        .ok_or("not one of your contacts")?;
    if matches!(contact.relationship.as_str(), "BLOCKED" | "DECLINED") {
        return Err("this contact is blocked or declined".into());
    }
    let me = if contact.identity == "creator" {
        node.creator().await.ok_or("this contact knows you as the creator, and the creator identity is not on this Hub")?
    } else {
        node.personal().await.ok_or("no personal identity on this Hub")?
    };
    if contact.enc_public_key.is_empty() {
        return Err("no encryption key for this contact yet (no accepted handshake)".into());
    }
    if !super::tor::is_valid_onion(&contact.onion_address) {
        return Err("no onion address for this contact".into());
    }
    let id = uuid::Uuid::new_v4().to_string();
    let ts = node.now();
    let body = json::JValue::obj().with("content", json::JValue::str(text)).render();
    let (ct, iv) = dm::encrypt_v2(&body, &contact.enc_public_key, &me.x25519_secret(), &me.public_key_b64, peer, &id, None, ts, dm::DM_AAD_MESSAGE)
        .map_err(|e| e.to_string())?;
    // EncryptedPayload's members, in its order (group_id is null for a DM, so absent).
    let payload = json::JValue::obj().with("id", json::JValue::str(&id)).with("nonce", json::JValue::str(iv))
        .with("ciphertext", json::JValue::str(ct)).with("timestamp", json::JValue::int(ts)).with("v", json::JValue::int(2));
    let pkt = Packet::new("MESSAGE", &me.public_key_b64, Some(peer), payload, 3);
    // NOSLOP_HUB_OUTBOX_V1: kept and tried again until the contact gets it (the phone may be off).
    super::outbox::send(node, &id, peer, &contact.onion_address, pkt).await?;
    Ok(DmRecord { id, peer: peer.to_string(), sender: me.public_key_b64.clone(), content: text.to_string(), timestamp: ts, media_id: None, media_type: None, incoming: false, me: me.public_key_b64.clone(), media: None })
}

/// Longest Web UI message (the phone's composer allows far less; this keeps one packet well under a frame).
pub const MAX_WEB_UI_DM_BYTES: usize = 16 * 1024;

/// The message body as the phone writes it: `{"content": ..., "media": {"id", "type", ...}}`, or plain text.
pub fn parse_body(plain: &str) -> (String, Option<String>, Option<String>) {
    match json::parse(plain) {
        Ok(v) if v.get("content").is_some() || v.get("media").is_some() => {
            let content = v.get_str("content").unwrap_or_default().to_string();
            let media = v.get("media");
            let media_id = media.and_then(|m| m.get_str("id")).map(String::from);
            let media_type = media.and_then(|m| m.get_str("type")).map(String::from);
            (content, media_id, media_type)
        }
        _ => (plain.to_string(), None, None),
    }
}

fn preview(rec: &DmRecord) -> String {
    let text = rec.content.trim();
    if text.is_empty() {
        // NOSLOP_HUB_DM_MEDIA_V1: a picker GIF is a link ("noslop-gif://https://..."), whatever its media type says.
        if rec.media_id.as_deref().map_or(false, |m| m.starts_with("noslop-gif://")) { return "[GIF]".into(); }
        return match rec.media_type.as_deref() { Some(t) => format!("[{}]", t), None => String::new() };
    }
    let mut out: String = text.chars().take(PREVIEW_CHARS).collect();
    if text.chars().count() > PREVIEW_CHARS { out.push('…'); }
    out
}

// END OF FILE hainet-noslop/src/hub/dms.rs
