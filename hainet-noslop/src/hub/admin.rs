//! START OF FILE hainet-noslop/src/hub/admin.rs
//!
//! Direct messages between the owner's phone and the Hub's Admin AI (`admin_<personal key>`, the
//! contact `NoSlopRepository.ensureAdminPeerExists` creates with the owner's own X25519 key).
//!
//! NoSlop 0.7+ only sends DM v2, so the owner's message is opened as v2 (sender = owner key,
//! recipient = `admin_<owner key>`), with the legacy v1 form as fallback. A `DM_ACK` v3 is returned at
//! once, so the phone marks the message delivered instead of re-sending it while the AI thinks, and
//! the reply is sealed as v2 the other way round.

use crate::{dm, json::JValue, keys::Identity, packet::Packet};

/// `DmPacketHandler.ACK_PLAINTEXT`.
pub const ACK_PLAINTEXT: &str = "ACK";

pub fn admin_id(owner: &Identity) -> String { format!("admin_{}", owner.public_key_b64) }

/// Whether `pkt` is the owner's message to the Admin AI.
pub fn is_owner_to_admin(owner: &Identity, pkt: &Packet) -> bool {
    pkt.ptype == "MESSAGE" && pkt.sender_id == owner.public_key_b64 && pkt.target_user_id.as_deref() == Some(admin_id(owner).as_str())
}

/// Opens the owner's message. Returns (message id, text). A JSON body `{"content": ...}` gives its content.
pub fn open_owner_message(owner: &Identity, pkt: &Packet) -> Result<(String, String), String> {
    let p = pkt.payload.as_ref().ok_or("no payload")?;
    let id = p.get_str("id").ok_or("no id")?;
    let ct = p.get_str("ciphertext").ok_or("no ciphertext")?;
    let nonce = p.get_str("nonce").ok_or("no nonce")?;
    let ts = p.get_i64("timestamp").unwrap_or(0);
    let secret = owner.x25519_secret();
    let plain = dm::decrypt_v2(ct, nonce, &owner.enc_public_key_b64, &secret, &owner.public_key_b64, &admin_id(owner), id, None, ts, dm::DM_AAD_MESSAGE)
        .or_else(|_| dm::decrypt_v1(ct, nonce, &owner.enc_public_key_b64, &secret))
        .map_err(|e| format!("cannot open the message: {}", e))?;
    let text = crate::json::parse(&plain).ok().and_then(|v| v.get_str("content").map(String::from)).unwrap_or(plain);
    Ok((id.to_string(), text))
}

/// The Admin AI's reply: a v2 `MESSAGE` from `admin_<owner>` to the owner, as the phone reads it
/// from the inbox (`DmPacketHandler.handleDirectMessage`).
pub fn reply_packet(owner: &Identity, text: &str, now: i64) -> Result<Packet, String> {
    let me = admin_id(owner);
    let secret = owner.x25519_secret();
    let body = JValue::obj().with("content", JValue::str(text)).render();
    let msg_id = uuid::Uuid::new_v4().to_string();
    let (ct, iv) = dm::encrypt_v2(&body, &owner.enc_public_key_b64, &secret, &me, &owner.public_key_b64, &msg_id, None, now, dm::DM_AAD_MESSAGE).map_err(|e| e.to_string())?;
    let payload = JValue::obj()
        .with("id", JValue::str(&msg_id))
        .with("nonce", JValue::str(iv))
        .with("ciphertext", JValue::str(ct))
        .with("timestamp", JValue::int(now))
        .with("v", JValue::int(2));
    Ok(Packet::new("MESSAGE", &me, Some(&owner.public_key_b64), payload, 1))
}

/// The `DM_ACK` (v3, with the legacy form alongside, as `DmPacketHandler.sendDmAck` writes it) for
/// the owner's message `msg_id`, so the phone stops re-sending it (`handleDmAck`).
pub fn ack_packet(owner: &Identity, msg_id: &str, now: i64) -> Result<Packet, String> {
    let me = admin_id(owner);
    let secret = owner.x25519_secret();
    let (ack_ct, ack_iv) = dm::encrypt_v2(ACK_PLAINTEXT, &owner.enc_public_key_b64, &secret, &me, &owner.public_key_b64, msg_id, None, now, dm::DM_AAD_ACK).map_err(|e| e.to_string())?;
    let (legacy_ct, legacy_iv) = dm::encrypt_v2(ACK_PLAINTEXT, &owner.enc_public_key_b64, &secret, &me, &owner.public_key_b64, msg_id, None, now, dm::DM_AAD_MESSAGE).map_err(|e| e.to_string())?;
    let payload = JValue::obj()
        .with("msg_id", JValue::str(msg_id))
        .with("nonce", JValue::str(legacy_iv))
        .with("ciphertext", JValue::str(legacy_ct))
        .with("timestamp", JValue::int(now))
        .with("v", JValue::int(3))
        .with("ack_nonce", JValue::str(ack_iv))
        .with("ack_ciphertext", JValue::str(ack_ct));
    Ok(Packet::new("DM_ACK", &me, Some(&owner.public_key_b64), payload, 1))
}
