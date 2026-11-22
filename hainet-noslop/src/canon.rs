//! START OF FILE hainet-noslop/src/canon.rs
//!
//! The canonical strings NoSlop signs, reproduced exactly (`CryptoService.kt`, `EnvelopeAuth.kt`,
//! `MediaManager.mediaRequestSigningString`, `MeshSocialRepository.announceCreatorPresence`).

/// `CryptoService.encodeForSigning`: each part as `<length>:<part>`, `None` as `0:`.
///
/// The length is Kotlin's `String.length`, i.e. the number of **UTF-16 code units**, not bytes and
/// not characters: `"😀x"` is `3:😀x`.
pub fn encode_for_signing(parts: &[Option<&str>]) -> String {
    let mut out = String::new();
    for p in parts {
        match p {
            None => out.push_str("0:"),
            Some(s) => {
                out.push_str(&s.encode_utf16().count().to_string());
                out.push(':');
                out.push_str(s);
            }
        }
    }
    out
}

/// Shorthand for the common case where every part is present.
pub fn efs(parts: &[&str]) -> String {
    let v: Vec<Option<&str>> = parts.iter().map(|p| Some(*p)).collect();
    encode_for_signing(&v)
}

/// `CryptoService.canonicalAnnouncePeerV2`.
pub fn announce_peer_v2(author_id: &str, onion: Option<&str>, timestamp: i64) -> String {
    efs(&["noslop-announce-v2", author_id, onion.unwrap_or(""), &timestamp.to_string()])
}

/// `CryptoService.canonicalHandshakePayloadV2`. `intro` is appended only when present.
#[allow(clippy::too_many_arguments)]
pub fn handshake_v2(
    from_user_id: &str,
    from_username: &str,
    from_home_node: &str,
    from_enc_pub: &str,
    target_user_id: &str,
    nonce: &str,
    timestamp: i64,
    avatar_b64: Option<&str>,
    bio: Option<&str>,
    intro: Option<&str>,
) -> String {
    let ts = timestamp.to_string();
    let mut parts: Vec<Option<&str>> = vec![
        Some("noslop-hs-v2"), Some(from_user_id), Some(from_username), Some(from_home_node),
        Some(from_enc_pub), Some(target_user_id), Some(nonce), Some(&ts), avatar_b64, bio,
    ];
    if let Some(i) = intro { parts.push(Some(i)); }
    encode_for_signing(&parts)
}

/// `MediaManager.mediaRequestSigningString` (`noslop-media-req-v1`).
#[allow(clippy::too_many_arguments)]
pub fn media_request(
    sender_id: &str,
    target_user_id: Option<&str>,
    media_id: &str,
    chunk_index: i64,
    chunk_size: i64,
    byte_offset: Option<i64>,
    byte_length: Option<i64>,
    timestamp: Option<i64>,
) -> String {
    efs(&[
        "noslop-media-req-v1", sender_id, target_user_id.unwrap_or(""), media_id,
        &chunk_index.to_string(), &chunk_size.to_string(),
        &byte_offset.unwrap_or(-1).to_string(), &byte_length.unwrap_or(-1).to_string(),
        &timestamp.unwrap_or(0).to_string(),
    ])
}

/// What `announceCreatorPresence` signs for `ANNOUNCE_DISCOVERABLE` (colon-joined, the periodic
/// announcement carries no avatar, so that field is empty).
#[allow(clippy::too_many_arguments)]
pub fn discoverable(
    author_id: &str, handle: &str, onion: &str, enc_pub: &str, is_creator: bool,
    fund_me_link: Option<&str>, avatar_b64: Option<&str>, bio: Option<&str>, timestamp: i64,
) -> String {
    format!(
        "{}:{}:{}:{}:{}:{}:{}:{}:{}",
        author_id, handle, onion, enc_pub, is_creator,
        fund_me_link.unwrap_or(""), avatar_b64.unwrap_or(""), bio.unwrap_or(""), timestamp
    )
}

/// `CONNECTION_REJECTED` (`buildConnectionRejectedPacket`): `encodeForSigning(fromUserId, timestamp)`.
pub fn connection_rejected(from_user_id: &str, timestamp: i64) -> String {
    efs(&[from_user_id, &timestamp.to_string()])
}

/// Hub owner-API request signature (new in this Hub, `noslop-hub-api-v1`): binds method, path,
/// timestamp, nonce and the SHA-256 (hex) of the raw request body bytes.
pub fn hub_api_request(method: &str, path: &str, timestamp: i64, nonce: &str, body_sha256_hex: &str) -> String {
    efs(&["noslop-hub-api-v1", method, path, &timestamp.to_string(), nonce, body_sha256_hex])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn utf16_lengths() {
        assert_eq!(encode_for_signing(&[Some("😀x"), None, Some("")]), "3:😀x0:0:");
    }
}
