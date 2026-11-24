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
}

pub type DmHook = Arc<dyn Fn(DmRecord) -> BoxFut<()> + Send + Sync>;

/// Longest text kept in a notification event.
pub const PREVIEW_CHARS: usize = 140;

/// Opens `pkt` when it is a one-to-one DM between one of the owner's identities and a contact, and hands it
/// on. `incoming`: the packet reached the Hub from the mesh (true) or the phone is sending it (false).
pub async fn observe(node: &Arc<HubNode>, pkt: &Packet, incoming: bool) {
    if pkt.ptype != "MESSAGE" { return; }
    let Some(rec) = open(node, pkt, incoming).await else { return };
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
            return Some(DmRecord {
                id: id.to_string(),
                peer: peer.clone(),
                sender: sender.to_string(),
                content,
                timestamp: ts,
                media_id,
                media_type,
                incoming,
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
    node.deliver(contact.onion_address.clone(), pkt, super::Retry::Spool);
    Ok(DmRecord { id, peer: peer.to_string(), sender: me.public_key_b64.clone(), content: text.to_string(), timestamp: ts, media_id: None, media_type: None, incoming: false })
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
        return match rec.media_type.as_deref() { Some(t) => format!("[{}]", t), None => String::new() };
    }
    let mut out: String = text.chars().take(PREVIEW_CHARS).collect();
    if text.chars().count() > PREVIEW_CHARS { out.push('…'); }
    out
}

// END OF FILE hainet-noslop/src/hub/dms.rs
