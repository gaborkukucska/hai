//! START OF FILE hainet-noslop/tests/vectors.rs
//!
//! Cross-language checks: every value here was produced by the NoSlop app's own libraries (Gson 2.10,
//! Bouncy Castle 1.78.1) running code that mirrors the Kotlin functions (`tests/vector-generator/GenVectors.java`).
//! If one of these fails, the Hub and the phone disagree on the wire.

use hainet_noslop::{canon, dm, envelope, json, keys, packet::Packet};

fn vectors() -> serde_json::Value {
    serde_json::from_str(include_str!("vectors/noslop_vectors.json")).unwrap()
}
fn s<'a>(v: &'a serde_json::Value, k: &str) -> &'a str { v[k].as_str().unwrap() }

#[test]
fn encode_for_signing_matches_kotlin() {
    for case in vectors()["encode_for_signing"].as_array().unwrap() {
        let parts: Vec<Option<String>> = case["parts"].as_array().unwrap().iter().map(|p| p.as_str().map(|x| x.to_string())).collect();
        let refs: Vec<Option<&str>> = parts.iter().map(|p| p.as_deref()).collect();
        assert_eq!(canon::encode_for_signing(&refs), s(case, "encoded"));
    }
}

#[test]
fn payload_rendering_and_digest_match_gson() {
    for case in vectors()["payload_digest"].as_array().unwrap() {
        let parsed = json::parse(s(case, "wire")).unwrap();
        let payload = if parsed.is_null() { None } else { Some(&parsed) };
        let rendered = match payload { None => String::new(), Some(p) => p.without_null_members().render() };
        assert_eq!(rendered, s(case, "rendered"), "wire {}", s(case, "wire"));
        assert_eq!(envelope::payload_digest(payload), s(case, "digest"));
    }
}

#[test]
fn identity_formats_match_bouncy_castle() {
    let v = vectors();
    let id = &v["identity"];
    let seed = hex::decode(s(id, "seed_hex")).unwrap();
    assert_eq!(keys::ed25519_seed(s(id, "private_key_b64")).unwrap().to_vec(), seed);
    let raw = keys::ed25519_public_raw(s(id, "public_key_b64")).unwrap();
    assert_eq!(keys::ed25519_public_b64(&raw), s(id, "public_key_b64"));
    assert_eq!(keys::onion_of(s(id, "public_key_b64")).unwrap(), s(id, "onion"));
    assert!(keys::is_onion_of(s(id, "public_key_b64"), &s(id, "onion").to_uppercase()));
    let seed32: [u8; 32] = seed.try_into().unwrap();
    assert_eq!(keys::b64_encode(&keys::tor_expanded_secret(&seed32)), s(id, "tor_expanded_b64"));
    // The 83-byte BC X25519 private key (v1 OneAsymmetricKey with the public key) is read correctly.
    let ident = keys::Identity::from_b64(s(id, "public_key_b64"), s(id, "private_key_b64"), s(id, "enc_public_key_b64"), s(id, "enc_private_key_b64")).unwrap();
    assert_eq!(ident.onion, s(id, "onion"));
    // Wrong kinds are refused, as CryptoService does.
    assert!(keys::ed25519_seed(s(id, "public_key_b64")).is_err());
    assert!(keys::ed25519_public_raw(s(id, "private_key_b64")).is_err());
    // A mismatched X25519 pair is refused.
    assert!(keys::Identity::from_b64(s(id, "public_key_b64"), s(id, "private_key_b64"), s(&v["dm_v2"], "recipient_enc_pub_b64"), s(id, "enc_private_key_b64")).is_err());
}

#[test]
fn envelope_verifies_and_resigns_identically() {
    let v = vectors();
    let id = &v["identity"];
    let ident = keys::Identity::from_b64(s(id, "public_key_b64"), s(id, "private_key_b64"), s(id, "enc_public_key_b64"), s(id, "enc_private_key_b64")).unwrap();
    let pkt = Packet::from_wire(s(&v["envelope"], "packet_wire")).unwrap();
    let ts = pkt.env_ts.unwrap();
    assert_eq!(envelope::signing_string(&pkt, ts), s(&v["envelope"], "signing_string"));
    assert_eq!(envelope::verify(&pkt, ts + 1000), envelope::Verdict::Valid);
    assert_eq!(envelope::verify(&pkt, ts + envelope::MAX_SKEW_MS + 1), envelope::Verdict::Stale);
    // Ed25519 is deterministic: re-signing gives the same signature as Bouncy Castle.
    let resigned = envelope::sign(&pkt, &ident, ts).unwrap();
    assert_eq!(resigned.env_sig, pkt.env_sig);
    // hops is not covered; the payload is.
    let mut hopped = pkt.clone();
    hopped.hops = Some(1);
    assert_eq!(envelope::verify(&hopped, ts), envelope::Verdict::Valid);
    let mut tampered = pkt.clone();
    tampered.payload.as_mut().unwrap().set("timestamp", json::JValue::int(1));
    assert_eq!(envelope::verify(&tampered, ts), envelope::Verdict::Invalid);
    let mut bare = pkt.clone();
    bare.env_sig = None;
    assert_eq!(envelope::verify(&bare, ts), envelope::Verdict::Missing);
    // Wire round trip keeps the verifiable form.
    let again = Packet::from_wire(&pkt.to_wire()).unwrap();
    assert_eq!(envelope::verify(&again, ts), envelope::Verdict::Valid);
    // Replay guard.
    let mut g = envelope::ReplayGuard::new(10);
    assert!(g.admit(&pkt, ts));
    assert!(!g.admit(&pkt, ts + 5));
}

#[test]
fn canonical_strings_and_signatures_match() {
    let v = vectors();
    let id = &v["identity"];
    let c = &v["canonical"];
    let ident = keys::Identity::from_b64(s(id, "public_key_b64"), s(id, "private_key_b64"), s(id, "enc_public_key_b64"), s(id, "enc_private_key_b64")).unwrap();
    let pubk = s(id, "public_key_b64");
    let onion = s(id, "onion");
    let a = canon::announce_peer_v2(pubk, Some(onion), 1763700000000);
    assert_eq!(a, s(c, "announce_v2"));
    assert_eq!(ident.sign(&a), s(c, "announce_v2_sig"));
    let hs = canon::handshake_v2(pubk, "Alice.abc12345", onion, s(id, "enc_public_key_b64"), "TARGETKEY", "nonce123", 1763700000000, None, Some("bio 😀"), None);
    assert_eq!(hs, s(c, "handshake_v2"));
    assert!(keys::verify(&hs, s(c, "handshake_v2_sig"), pubk));
    let mr = canon::media_request(pubk, Some("TARGETKEY"), "media_1", 0, 262144, Some(0), Some(262144), Some(1763700000000));
    assert_eq!(mr, s(c, "media_req"));
    assert!(keys::verify(&mr, s(c, "media_req_sig"), pubk));
    let d = canon::discoverable(pubk, "Creator.abc", onion, s(id, "enc_public_key_b64"), true, Some("https://x.y/z"), None, Some("my bio"), 1763700000000);
    assert_eq!(d, s(c, "discoverable"));
    assert_eq!(ident.sign(&d), s(c, "discoverable_sig"));
}

#[test]
fn dm_v2_matches_kotlin() {
    let v = vectors();
    let d = &v["dm_v2"];
    let sender_priv = x25519_dalek::StaticSecret::from(keys::x25519_private_raw(s(d, "sender_enc_priv_b64")).unwrap());
    let recip_priv = x25519_dalek::StaticSecret::from(keys::x25519_private_raw(s(d, "recipient_enc_priv_b64")).unwrap());
    let iv: [u8; 12] = keys::b64_decode(s(d, "iv_b64")).unwrap().try_into().unwrap();
    let ts = d["timestamp"].as_i64().unwrap();
    assert_eq!(dm::aad_v2(dm::DM_AAD_MESSAGE, s(d, "sender_ed"), s(d, "recipient_ed"), s(d, "msg_id"), None, ts), s(d, "aad"));
    let (ct, _) = dm::encrypt_v2_with_iv(s(d, "plaintext"), s(d, "recipient_enc_pub_b64"), &sender_priv, s(d, "sender_ed"), s(d, "recipient_ed"), s(d, "msg_id"), None, ts, dm::DM_AAD_MESSAGE, iv).unwrap();
    assert_eq!(ct, s(d, "ciphertext_b64"));
    // The recipient opens it with its own key and the sender's public key.
    let pt = dm::decrypt_v2(&ct, s(d, "iv_b64"), s(d, "sender_enc_pub_b64"), &recip_priv, s(d, "sender_ed"), s(d, "recipient_ed"), s(d, "msg_id"), None, ts, dm::DM_AAD_MESSAGE).unwrap();
    assert_eq!(pt, s(d, "plaintext"));
    // Wrong direction, id, time or domain fail.
    assert!(dm::decrypt_v2(&ct, s(d, "iv_b64"), s(d, "sender_enc_pub_b64"), &recip_priv, s(d, "recipient_ed"), s(d, "sender_ed"), s(d, "msg_id"), None, ts, dm::DM_AAD_MESSAGE).is_err());
    assert!(dm::decrypt_v2(&ct, s(d, "iv_b64"), s(d, "sender_enc_pub_b64"), &recip_priv, s(d, "sender_ed"), s(d, "recipient_ed"), "other", None, ts, dm::DM_AAD_MESSAGE).is_err());
    assert!(dm::decrypt_v2(&ct, s(d, "iv_b64"), s(d, "sender_enc_pub_b64"), &recip_priv, s(d, "sender_ed"), s(d, "recipient_ed"), s(d, "msg_id"), None, ts + 1, dm::DM_AAD_MESSAGE).is_err());
    assert!(dm::decrypt_v2(&ct, s(d, "iv_b64"), s(d, "sender_enc_pub_b64"), &recip_priv, s(d, "sender_ed"), s(d, "recipient_ed"), s(d, "msg_id"), None, ts, dm::DM_AAD_ACK).is_err());
    assert_eq!(hex::encode(dm::directional_key_v2(&recip_priv.diffie_hellman(&x25519_dalek::PublicKey::from(keys::x25519_public_raw(s(d, "sender_enc_pub_b64")).unwrap())).to_bytes(), s(d, "sender_ed"), s(d, "recipient_ed"))), s(d, "key_hex"));
}

/// The Hub owner-API request signature (`noslop-hub-api-v1`). The same value is asserted on the
/// Android side (`HubClientSignatureTest`), so phone and Hub can't drift apart.
pub const HUB_API_VECTOR_SIG: &str = "5V9Qft1L7FN3zcw425qkbqaswUhWnUufLz8prXFjOOVOT9Tct6E9lXO+4QIEIRMQpiQCFvp3MmGBa4t2E5c/AA==";

#[test]
fn hub_api_request_signature_is_pinned() {
    let v = vectors();
    let id = &v["identity"];
    let ident = keys::Identity::from_b64(s(id, "public_key_b64"), s(id, "private_key_b64"), s(id, "enc_public_key_b64"), s(id, "enc_private_key_b64")).unwrap();
    let body_sha = envelope::sha256_hex(br#"{"cmd":"hub_status","args":{}}"#);
    let signed = canon::hub_api_request("POST", "/api/invoke", 1763700000000, "nonce_aaaaaaaaaaaaaaaa", &body_sha);
    assert_eq!(signed, format!("17:noslop-hub-api-v14:POST11:/api/invoke13:176370000000022:nonce_aaaaaaaaaaaaaaaa64:{}", body_sha));
    let sig = ident.sign(&signed);
    assert_eq!(sig, HUB_API_VECTOR_SIG);
}
