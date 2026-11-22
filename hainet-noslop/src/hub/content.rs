//! START OF FILE hainet-noslop/src/hub/content.rs
//!
//! Signature checks for the content the Hub stores and serves on behalf of the creator, mirroring
//! `MeshPacketVerifier` / `SyncPacketHandler.handleSyncResponse` (WIRE_PROTOCOL_REFERENCE §7):
//!
//! * Post (`PostPayload`): v2 `encodeForSigning(id, authorId, content, timestamp, avatar, privacy,
//!   mediaId, clearnetUrl, sha256)` when `media_metadata.sha256` is set, else the 8-field form.
//! * Comment (`CommentSyncData` / `CommentData`): v2 with the media digest, the 5-field form (with
//!   avatar), the 4-field form, or the pipe form.
//! * Reaction (`ReactionSyncData` / `ReactionPayload`): `encodeForSigning(postId, type, authorId,
//!   timestamp)` or the pipe form.
//!
//! The Hub only ever stores what verifies, so a follower never receives something its own app would
//! reject, and nothing on the Hub can be altered without breaking the creator's signature.

use crate::{canon::encode_for_signing, json::JValue, keys};

#[derive(Debug, Clone, PartialEq)]
pub struct PostInfo {
    pub id: String,
    pub author_id: String,
    pub content: String,
    pub timestamp: i64,
    pub privacy: String,
    pub media_id: Option<String>,
    pub media_sha256: Option<String>,
    /// Whether the v2 form (covering the media digest) verified.
    pub signed_digest: bool,
}

fn need<'a>(v: &'a JValue, k: &'static str) -> Result<&'a str, String> {
    v.get_str(k).ok_or_else(|| format!("missing {}", k))
}

/// Verifies a `PostPayload` (as carried in `POST`, `SYNC_RESPONSE.posts` or pushed by the owner).
pub fn verify_post(p: &JValue) -> Result<PostInfo, String> {
    let id = need(p, "id")?;
    let author = need(p, "author_id")?;
    let content = need(p, "content")?;
    let ts = p.get_i64("timestamp").ok_or("missing timestamp")?;
    let sig = need(p, "signature")?;
    let avatar = p.get_str("author_avatar_b64");
    let privacy = p.get_str("privacy").unwrap_or("public");
    let media_id = p.get_str("media_id");
    let clearnet = p.get_str("clearnet_url");
    let meta = p.get("media_metadata").filter(|m| !m.is_null());
    // S14: media id and metadata must agree.
    match (media_id, meta) {
        (Some(mid), Some(m)) => { if m.get_str("id") != Some(mid) { return Err("media_id does not match media_metadata.id".into()); } }
        (Some(_), None) => return Err("media_id without media_metadata".into()),
        (None, Some(_)) => return Err("media_metadata without media_id".into()),
        (None, None) => {}
    }
    let sha = meta.and_then(|m| m.get_str("sha256"));
    let ts_s = ts.to_string();
    let base = [Some(id), Some(author), Some(content), Some(ts_s.as_str()), avatar, Some(privacy), media_id, clearnet];
    let mut signed_digest = false;
    let ok = if let Some(h) = sha {
        let mut v2 = base.to_vec();
        v2.push(Some(h));
        if keys::verify(&encode_for_signing(&v2), sig, author) { signed_digest = true; true } else { keys::verify(&encode_for_signing(&base), sig, author) }
    } else {
        keys::verify(&encode_for_signing(&base), sig, author)
    };
    if !ok { return Err("post signature does not verify".into()); }
    Ok(PostInfo {
        id: id.into(), author_id: author.into(), content: content.into(), timestamp: ts, privacy: privacy.into(),
        media_id: media_id.map(Into::into), media_sha256: sha.map(Into::into), signed_digest,
    })
}

/// SHA3-256 hex of `id|author|content|timestamp`: the per-post hash of `INVENTORY_SYNC_REQUEST`.
pub fn inventory_hash(id: &str, author: &str, content: &str, timestamp: i64) -> String {
    use sha3::{Digest, Sha3_256};
    hex::encode(Sha3_256::digest(format!("{}|{}|{}|{}", id, author, content, timestamp).as_bytes()))
}

/// Verifies a comment in `CommentSyncData` form (`post_id` inside). Returns (id, post_id, author, timestamp).
pub fn verify_comment(c: &JValue) -> Result<(String, String, String, i64), String> {
    let id = need(c, "id")?;
    let post_id = need(c, "post_id")?;
    let author = need(c, "author_id")?;
    let content = need(c, "content")?;
    let ts = c.get_i64("timestamp").ok_or("missing timestamp")?;
    let sig = need(c, "signature")?;
    let avatar = c.get_str("author_avatar_b64");
    if !comment_signature_ok(post_id, id, content, ts, avatar, c.get("media_metadata").and_then(|m| m.get_str("sha256")), sig, author) {
        return Err("comment signature does not verify".into());
    }
    Ok((id.into(), post_id.into(), author.into(), ts))
}

#[allow(clippy::too_many_arguments)]
pub fn comment_signature_ok(post_id: &str, id: &str, content: &str, ts: i64, avatar: Option<&str>, sha: Option<&str>, sig: &str, author: &str) -> bool {
    let ts_s = ts.to_string();
    if let Some(h) = sha {
        if keys::verify(&encode_for_signing(&[Some(post_id), Some(id), Some(content), Some(&ts_s), avatar, Some(h)]), sig, author) { return true; }
    }
    if keys::verify(&encode_for_signing(&[Some(post_id), Some(id), Some(content), Some(&ts_s), avatar]), sig, author) { return true; }
    if keys::verify(&encode_for_signing(&[Some(post_id), Some(id), Some(content), Some(&ts_s)]), sig, author) { return true; }
    let mut pipe = format!("{}|{}|{}|{}", post_id, id, content, ts);
    if let Some(a) = avatar { pipe.push('|'); pipe.push_str(a); }
    keys::verify(&pipe, sig, author)
}

pub fn reaction_signature_ok(post_id: &str, reaction_type: &str, author: &str, ts: i64, sig: &str) -> bool {
    let ts_s = ts.to_string();
    keys::verify(&encode_for_signing(&[Some(post_id), Some(reaction_type), Some(author), Some(&ts_s)]), sig, author)
        || keys::verify(&format!("{}|{}|{}|{}", post_id, reaction_type, author, ts), sig, author)
}

/// Verifies a `ReactionSyncData`. Returns (id, post_id, author, timestamp).
pub fn verify_reaction(r: &JValue) -> Result<(String, String, String, i64), String> {
    let id = need(r, "id")?;
    let post_id = need(r, "post_id")?;
    let author = need(r, "author_id")?;
    let rtype = need(r, "reaction_type")?;
    let ts = r.get_i64("timestamp").ok_or("missing timestamp")?;
    let sig = need(r, "signature")?;
    if !reaction_signature_ok(post_id, rtype, author, ts, sig) { return Err("reaction signature does not verify".into()); }
    Ok((id.into(), post_id.into(), author.into(), ts))
}

/// A live `COMMENT` payload (`CommentPayload { post_id, comment: CommentData, parent_comment_id }`)
/// in the `CommentSyncData` form used by `SYNC_RESPONSE`.
pub fn comment_payload_to_sync(payload: &JValue) -> Option<JValue> {
    let post_id = payload.get_str("post_id")?;
    let c = payload.get("comment")?;
    Some(
        JValue::obj()
            .with("id", c.get("id")?.clone())
            .with("post_id", JValue::str(post_id))
            .with("author_id", c.get("author_id")?.clone())
            .with("author_name", c.get("author_name").cloned().unwrap_or(JValue::str("")))
            .with_opt("author_avatar_b64", c.get("author_avatar_b64").filter(|v| !v.is_null()).cloned())
            .with("content", c.get("content")?.clone())
            .with("timestamp", c.get("timestamp")?.clone())
            .with("signature", c.get("signature")?.clone())
            .with_opt("parent_comment_id", payload.get("parent_comment_id").filter(|v| !v.is_null()).cloned())
            .with_opt("media_id", c.get("media_id").filter(|v| !v.is_null()).cloned())
            .with_opt("media_type", c.get("media_type").filter(|v| !v.is_null()).cloned())
            .with_opt("media_metadata", c.get("media_metadata").filter(|v| !v.is_null()).cloned()),
    )
}

/// `ReactionPacketHandler`'s row id for a post reaction.
pub fn reaction_id(post_id: &str, author: &str, reaction_type: &str) -> String { format!("{}_{}_{}", post_id, author, reaction_type) }
