//! START OF FILE hainet-noslop/src/hub/tests.rs
//!
//! Behaviour tests for the Home HUB node: in-memory SQLite, a fixed clock, and a recorder in place
//! of Tor. Followers are simulated with the same canonical strings the vector tests prove against
//! the Android app's own libraries.

use super::*;
use crate::{canon, envelope, json::{self, JValue}, keys::{self, Identity}, packet::Packet};
use std::sync::Mutex as StdMutex;

const NOW: i64 = 1_763_700_000_000;

fn ident(seed_byte: u8) -> (Identity, [String; 4]) {
    let seed = [seed_byte; 32];
    let sk = ed25519_dalek::SigningKey::from_bytes(&seed);
    let pub_b64 = keys::ed25519_public_b64(&sk.verifying_key().to_bytes());
    let mut pkcs8 = keys::ED25519_PKCS8_HEADER.to_vec();
    pkcs8.extend_from_slice(&seed);
    let priv_b64 = keys::b64_encode(&pkcs8);
    let xs = x25519_dalek::StaticSecret::from([seed_byte.wrapping_add(1); 32]);
    let xp = x25519_dalek::PublicKey::from(&xs);
    let mut xpub = vec![0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x6e, 0x03, 0x21, 0x00];
    xpub.extend_from_slice(xp.as_bytes());
    let xpub_b64 = keys::b64_encode(&xpub);
    let xpriv_b64 = keys::b64_encode(&xs.to_bytes());
    let strs = [pub_b64.clone(), priv_b64.clone(), xpub_b64.clone(), xpriv_b64.clone()];
    (Identity::from_b64(&pub_b64, &priv_b64, &xpub_b64, &xpriv_b64).unwrap(), strs)
}

struct Rig {
    node: Arc<HubNode>,
    sent: Arc<StdMutex<Vec<(String, String)>>>,
    owner: Identity,
    creator: Identity,
    _dir: std::path::PathBuf,
}

async fn rig(creator_mode: bool) -> Rig {
    let dir = std::env::temp_dir().join(format!("nh-test-{}", uuid::Uuid::new_v4()));
    let (owner, o) = ident(1);
    let (creator, c) = ident(2);
    std::fs::create_dir_all(dir.join("identity")).unwrap();
    for (f, v) in ["ed25519_pub.b64", "ed25519_priv.b64", "x25519_pub.b64", "x25519_priv.b64"].iter().zip(o.iter()) {
        std::fs::write(dir.join("identity").join(f), v).unwrap();
    }
    let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
    let store = store::Store::open(pool).await.unwrap();
    let mut cfg = HubConfig::from_env(dir.join("identity"), dir.clone());
    cfg.identity_dir = dir.join("identity");
    cfg.noslop_dir = dir.join("noslop");
    let sent = Arc::new(StdMutex::new(Vec::new()));
    let s2 = sent.clone();
    let send: SendFn = Arc::new(move |onion, line| { s2.lock().unwrap().push((onion, line)); Box::pin(async { true }) });
    let node = HubNode::new(cfg, store, Some(send), false, Arc::new(|| NOW)).await;
    let args = JValue::obj().with("public_key", JValue::str(&c[0])).with("private_key", JValue::str(&c[1]))
        .with("enc_public_key", JValue::str(&c[2])).with("enc_private_key", JValue::str(&c[3])).with("display_name", JValue::str("Studio.tag12345"));
    api::handle(&node, "creator_identity_set", &args).await.unwrap().unwrap();
    let prof = JValue::obj().with("display_name", JValue::str("Studio.tag12345")).with("handle", JValue::str("Studio"))
        .with("bio", JValue::str("films")).with("fund_me_link", JValue::str("https://example.org/fund"))
        .with("is_creator", JValue::Bool(creator_mode)).with("is_discoverable", JValue::Bool(true));
    api::handle(&node, "creator_profile_set", &prof).await.unwrap().unwrap();
    api::handle(&node, "creator_hosting_set", &JValue::obj().with("enabled", JValue::Bool(true))).await.unwrap().unwrap();
    Rig { node, sent, owner, creator, _dir: dir }
}

fn signed(p: Packet, id: &Identity, now: i64) -> String { envelope::sign(&p, id, now).unwrap().to_wire() }

fn connection_request(follower: &Identity, creator: &Identity, nonce: &str, ts: i64) -> Packet {
    let username = "Fan.abcdefgh";
    let sig = follower.sign(&canon::handshake_v2(&follower.public_key_b64, username, &follower.onion, &follower.enc_public_key_b64, &creator.public_key_b64, nonce, ts, None, None, None));
    let payload = JValue::obj()
        .with("id", JValue::str("req-1"))
        .with("from_user_id", JValue::str(&follower.public_key_b64))
        .with("from_username", JValue::str(username))
        .with("from_display_name", JValue::str(username))
        .with("from_home_node", JValue::str(&follower.onion))
        .with("from_encryption_public_key", JValue::str(&follower.enc_public_key_b64))
        .with("timestamp", JValue::int(ts))
        .with("request_nonce", JValue::str(nonce))
        .with("target_user_id", JValue::str(&creator.public_key_b64))
        .with("version", JValue::int(2));
    let mut p = Packet::new("CONNECTION_REQUEST", &follower.public_key_b64, Some(&creator.public_key_b64), payload, 3);
    p.signature = Some(sig);
    p
}

fn creator_post(creator: &Identity, id: &str, ts: i64, media: Option<(&str, &str, i64)>) -> JValue {
    let content = format!("hello 😀 {}", id);
    let (mid, sha) = (media.map(|m| m.0), media.map(|m| m.1));
    let parts = [Some(id), Some(creator.public_key_b64.as_str()), Some(content.as_str()), Some(&*ts.to_string()), None, Some("public"), mid, None];
    let mut v = parts.to_vec();
    if let Some(h) = sha { v.push(Some(h)); }
    let sig = creator.sign(&canon::encode_for_signing(&v));
    JValue::obj()
        .with("id", JValue::str(id))
        .with("author_id", JValue::str(&creator.public_key_b64))
        .with("author_name", JValue::str("Studio"))
        .with("author_public_key", JValue::str(&creator.public_key_b64))
        .with("content", JValue::str(&content))
        .with("timestamp", JValue::int(ts))
        .with("privacy", JValue::str("public"))
        .with("signature", JValue::str(sig))
        .with_opt("media_id", mid.map(JValue::str))
        .with_opt("media_metadata", media.map(|(m, h, size)| JValue::obj().with("id", JValue::str(m)).with("type", JValue::str("video"))
            .with("mime_type", JValue::str("application/octet-stream")).with("size", JValue::int(size)).with("chunk_count", JValue::int(0)).with("sha256", JValue::str(h))))
}

async fn accept_follower(r: &Rig, follower: &Identity) {
    let line = signed(connection_request(follower, &r.creator, "nonce-0123456789", NOW - 1000), follower, NOW);
    let replies = r.node.handle_frame(&line, Listener::Creator).await;
    assert_eq!(replies.len(), 1, "handshake answered on the requester's connection");
}

fn sent_of(r: &Rig, ptype: &str) -> Vec<(String, Packet)> {
    r.sent.lock().unwrap().iter().filter_map(|(o, l)| Packet::from_wire(l).ok().filter(|p| p.ptype == ptype).map(|p| (o.clone(), p))).collect()
}

async fn settle() { for _ in 0..20 { tokio::task::yield_now().await; } tokio::time::sleep(std::time::Duration::from_millis(50)).await; }

#[tokio::test]
async fn unsigned_forged_and_replayed_frames_are_dropped() {
    let r = rig(true).await;
    let (stranger, _) = ident(9);
    let p = Packet::new("TYPING", &stranger.public_key_b64, Some(&r.owner.public_key_b64), JValue::obj().with("is_typing", JValue::Bool(true)), 3);
    assert!(r.node.handle_frame(&p.to_wire(), Listener::Personal).await.is_empty());
    assert_eq!(r.node.store.inbox_stats().await.unwrap().0, 0, "no envelope: dropped");
    let mut forged = envelope::sign(&p, &stranger, NOW).unwrap();
    forged.sender_id = r.owner.public_key_b64.clone();
    r.node.handle_frame(&forged.to_wire(), Listener::Personal).await;
    assert_eq!(r.node.store.inbox_stats().await.unwrap().0, 0, "envelope by another key: dropped");
    let good = signed(p, &stranger, NOW);
    r.node.handle_frame(&good, Listener::Personal).await;
    r.node.handle_frame(&good, Listener::Personal).await;
    assert_eq!(r.node.store.inbox_stats().await.unwrap().0, 1, "valid once, replay dropped");
    let (items, _) = r.node.store.inbox_pull(0, 10, 1 << 20).await.unwrap();
    assert_eq!(items[0].packet, good, "stored exactly as received");
}

#[tokio::test]
async fn follower_is_auto_accepted_with_a_valid_creator_handshake() {
    let r = rig(true).await;
    let (fan, _) = ident(7);
    let line = signed(connection_request(&fan, &r.creator, "nonce-0123456789", NOW - 1000), &fan, NOW);
    let replies = r.node.handle_frame(&line, Listener::Creator).await;
    assert_eq!(replies.len(), 1);
    let hs = Packet::from_wire(&replies[0]).unwrap();
    assert_eq!(hs.ptype, "USER_HANDSHAKE");
    assert_eq!(envelope::verify(&hs, NOW), envelope::Verdict::Valid);
    assert_eq!(hs.sender_id, r.creator.public_key_b64);
    let p = hs.payload.as_ref().unwrap();
    assert_eq!(p.get_str("in_reply_to_nonce"), Some("nonce-0123456789"));
    assert_eq!(p.get_str("from_home_node"), Some(r.creator.onion.as_str()));
    let s = canon::handshake_v2(&r.creator.public_key_b64, "Studio.tag12345", &r.creator.onion, &r.creator.enc_public_key_b64, &fan.public_key_b64, "nonce-0123456789", p.get_i64("timestamp").unwrap(), None, Some("films"), None);
    assert!(keys::verify(&s, hs.signature.as_deref().unwrap(), &r.creator.public_key_b64), "v2 handshake signature with the creator profile");
    let c = r.node.store.contact_get(&fan.public_key_b64).await.unwrap().unwrap();
    assert_eq!((c.identity.as_str(), c.relationship.as_str(), c.source.as_str()), ("creator", "ACCEPTED", "hub"));
    let (items, _) = r.node.store.inbox_pull(0, 10, 1 << 20).await.unwrap();
    assert_eq!(items[0].kind, "creator_follower_accepted");
    // A full contact push from a phone that hasn't seen the follower yet keeps the Hub's row.
    api::handle(&r.node, "hub_contacts_push", &JValue::obj().with("contacts", JValue::Arr(vec![])).with("full", JValue::Bool(true))).await.unwrap().unwrap();
    assert!(r.node.store.contact_get(&fan.public_key_b64).await.unwrap().is_some());
}

#[tokio::test]
async fn creator_mode_off_and_bad_requests_go_to_the_phone() {
    let r = rig(false).await;
    let (fan, _) = ident(7);
    let line = signed(connection_request(&fan, &r.creator, "nonce-0123456789", NOW - 1000), &fan, NOW);
    assert!(r.node.handle_frame(&line, Listener::Creator).await.is_empty());
    let (items, _) = r.node.store.inbox_pull(0, 10, 1 << 20).await.unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].kind, "packet", "the owner decides when creator mode is off");
    assert!(r.node.store.contact_get(&fan.public_key_b64).await.unwrap().is_none());
}

#[tokio::test]
async fn declined_probe_gets_a_rejection_and_blocked_peer_is_ignored() {
    let r = rig(true).await;
    let (fan, _) = ident(7);
    let c = JValue::obj().with("public_key", JValue::str(&fan.public_key_b64)).with("identity", JValue::str("creator")).with("relationship", JValue::str("NONE"))
        .with("declined_nonce", JValue::str("nonce-0123456789"));
    api::handle(&r.node, "hub_contacts_push", &JValue::obj().with("contacts", JValue::Arr(vec![c])).with("full", JValue::Bool(true))).await.unwrap().unwrap();
    let replies = r.node.handle_frame(&signed(connection_request(&fan, &r.creator, "nonce-0123456789", NOW - 1000), &fan, NOW), Listener::Creator).await;
    assert_eq!(Packet::from_wire(&replies[0]).unwrap().ptype, "CONNECTION_REJECTED");
    let (b, _) = ident(8);
    let c = JValue::obj().with("public_key", JValue::str(&b.public_key_b64)).with("identity", JValue::str("creator")).with("relationship", JValue::str("BLOCKED"));
    api::handle(&r.node, "hub_contacts_push", &JValue::obj().with("contacts", JValue::Arr(vec![c])).with("full", JValue::Bool(false))).await.unwrap().unwrap();
    assert!(r.node.handle_frame(&signed(connection_request(&b, &r.creator, "nonce-abcdefghijkl", NOW - 1000), &b, NOW), Listener::Creator).await.is_empty());
    assert_eq!(r.node.store.inbox_stats().await.unwrap().0, 0);
}

#[tokio::test]
async fn followers_get_the_creators_public_posts_and_nothing_else() {
    let r = rig(true).await;
    let (fan, _) = ident(7);
    accept_follower(&r, &fan).await;
    let posts = vec![creator_post(&r.creator, "p1", NOW - 10_000, None), creator_post(&r.creator, "p2", NOW - 5_000, None)];
    let mut forged = creator_post(&r.creator, "p3", NOW, None);
    forged.set("content", JValue::str("altered"));
    let (other, _) = ident(5);
    let foreign = creator_post(&other, "p4", NOW, None);
    let res = api::handle(&r.node, "creator_content_sync", &JValue::obj().with("posts", JValue::Arr(vec![posts[0].clone(), posts[1].clone(), forged, foreign])).with("full", JValue::Bool(true))).await.unwrap().unwrap();
    assert_eq!(res["posts"], 2);
    assert_eq!(res["rejected"].as_array().unwrap().len(), 2, "a tampered post and another author's post are refused");
    let req = Packet::new("SYNC_REQUEST", &fan.public_key_b64, Some(&r.creator.public_key_b64), JValue::obj().with("since", JValue::int(0)), 3);
    r.node.handle_frame(&signed(req, &fan, NOW), Listener::Creator).await;
    settle().await;
    let out = sent_of(&r, "SYNC_RESPONSE");
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].0, fan.onion);
    assert_eq!(envelope::verify(&out[0].1, NOW), envelope::Verdict::Valid);
    let served = out[0].1.p("posts").unwrap().as_array().unwrap().clone();
    assert_eq!(served.len(), 2);
    for p in &served { content::verify_post(p).expect("served post verifies on the follower"); }
    // Not forwarded to the phone: the Hub answered it.
    assert_eq!(r.node.store.inbox_pull(0, 10, 1 << 20).await.unwrap().0.iter().filter(|i| i.kind == "packet").count(), 0);
    // Inventory: a follower that already has p1 gets only p2.
    r.sent.lock().unwrap().clear();
    let p1 = r.node.store.post_get("p1").await.unwrap().unwrap();
    let inv = JValue::Arr(vec![JValue::obj().with("id", JValue::str("p1")).with("hash", JValue::str(content::inventory_hash("p1", &p1.author_id, &p1.content, p1.timestamp)))]);
    let req = Packet::new("INVENTORY_SYNC_REQUEST", &fan.public_key_b64, Some(&r.creator.public_key_b64), JValue::obj().with("inventory", inv), 3);
    r.node.handle_frame(&signed(req, &fan, NOW), Listener::Creator).await;
    settle().await;
    let out = sent_of(&r, "SYNC_RESPONSE");
    let ids: Vec<_> = out[0].1.p("posts").unwrap().as_array().unwrap().iter().map(|p| p.get_str("id").unwrap().to_string()).collect();
    assert_eq!(ids, vec!["p2"]);
    // A stranger's sync request is not answered.
    r.sent.lock().unwrap().clear();
    let (stranger, _) = ident(11);
    let req = Packet::new("SYNC_REQUEST", &stranger.public_key_b64, Some(&r.creator.public_key_b64), JValue::obj().with("since", JValue::int(0)), 3);
    r.node.handle_frame(&signed(req, &stranger, NOW), Listener::Creator).await;
    settle().await;
    assert!(sent_of(&r, "SYNC_RESPONSE").is_empty());
}

#[tokio::test]
async fn creator_media_is_uploaded_verified_and_served_in_chunks() {
    let r = rig(true).await;
    let (fan, _) = ident(7);
    accept_follower(&r, &fan).await;
    let bytes: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
    let sha = envelope::sha256_hex(&bytes);
    let post = creator_post(&r.creator, "pm", NOW - 1000, Some(("vid_1.mp4", &sha, bytes.len() as i64)));
    let res = api::handle(&r.node, "creator_content_sync", &JValue::obj().with("posts", JValue::Arr(vec![post]))).await.unwrap().unwrap();
    assert_eq!(res["media_needed"][0]["media_id"], "vid_1.mp4");
    let put = |off: usize, end: usize, sha: &str| JValue::obj().with("media_id", JValue::str("vid_1.mp4")).with("type", JValue::str("video")).with("mime_type", JValue::str("video/mp4"))
        .with("size", JValue::int(bytes.len() as i64)).with("sha256", JValue::str(sha)).with("offset", JValue::int(off as i64)).with("data", JValue::str(keys::b64_encode(&bytes[off..end])));
    // Out of order: told where to resume.
    let res = api::handle(&r.node, "creator_media_put", &put(1000, 2000, &sha)).await.unwrap().unwrap();
    assert_eq!(res["status"], "resume");
    api::handle(&r.node, "creator_media_put", &put(0, 200_000, &sha)).await.unwrap().unwrap();
    let res = api::handle(&r.node, "creator_media_put", &put(200_000, bytes.len(), &sha)).await.unwrap().unwrap();
    assert_eq!(res["complete"], true);
    // A chunk request from the follower.
    let req = JValue::obj().with("media_id", JValue::str("vid_1.mp4")).with("chunk_index", JValue::int(0)).with("chunk_size", JValue::int(262144))
        .with("byte_offset", JValue::int(262144)).with("byte_length", JValue::int(262144)).with("timestamp", JValue::int(NOW));
    r.node.handle_frame(&signed(Packet::new("MEDIA_REQUEST", &fan.public_key_b64, Some(&r.creator.public_key_b64), req, 1), &fan, NOW), Listener::Creator).await;
    settle().await;
    let chunk = &sent_of(&r, "MEDIA_CHUNK")[0].1;
    let data = keys::b64_decode(chunk.p_str("data").unwrap()).unwrap();
    assert_eq!(data, bytes[262144..].to_vec());
    assert_eq!(chunk.p_i64("total_size"), Some(bytes.len() as i64));
    // A stranger with a signed request is answered at the onion of its own key; unsigned: not answered.
    r.sent.lock().unwrap().clear();
    let (stranger, _) = ident(12);
    let mut req = Packet::new("MEDIA_REQUEST", &stranger.public_key_b64, Some(&r.creator.public_key_b64),
        JValue::obj().with("media_id", JValue::str("vid_1.mp4")).with("chunk_index", JValue::int(0)).with("chunk_size", JValue::int(1000)).with("timestamp", JValue::int(NOW)), 1);
    r.node.handle_frame(&signed(req.clone(), &stranger, NOW), Listener::Creator).await;
    settle().await;
    assert!(sent_of(&r, "MEDIA_CHUNK").is_empty());
    req.signature = Some(stranger.sign(&canon::media_request(&stranger.public_key_b64, Some(&r.creator.public_key_b64), "vid_1.mp4", 0, 1000, None, None, Some(NOW))));
    req.id = Some("another".into());
    r.node.handle_frame(&signed(req, &stranger, NOW), Listener::Creator).await;
    settle().await;
    let out = sent_of(&r, "MEDIA_CHUNK");
    assert_eq!(out[0].0, stranger.onion);
    // A corrupt upload is refused.
    let post = creator_post(&r.creator, "pm2", NOW - 900, Some(("vid_2.mp4", &sha, bytes.len() as i64)));
    api::handle(&r.node, "creator_content_sync", &JValue::obj().with("posts", JValue::Arr(vec![post]))).await.unwrap().unwrap();
    let mut bad = bytes.clone();
    bad[5] ^= 1;
    let p = JValue::obj().with("media_id", JValue::str("vid_2.mp4")).with("type", JValue::str("video")).with("size", JValue::int(bad.len() as i64)).with("sha256", JValue::str(&sha)).with("offset", JValue::int(0)).with("data", JValue::str(keys::b64_encode(&bad)));
    assert!(api::handle(&r.node, "creator_media_put", &p).await.unwrap().is_err());
}

#[tokio::test]
async fn hub_send_resigns_own_packets_keeps_relays_and_tracks_creator_posts() {
    let r = rig(true).await;
    let (friend, _) = ident(6);
    // An own packet with a stale (phone-made) envelope is signed again at send time.
    let own = envelope::sign(&Packet::new("TYPING", &r.owner.public_key_b64, Some(&friend.public_key_b64), JValue::obj().with("is_typing", JValue::Bool(true)), 3), &r.owner, NOW - 2 * envelope::MAX_SKEW_MS).unwrap();
    // Someone else's packet the phone relays (its own envelope).
    let relay = envelope::sign(&Packet::new("POST", &friend.public_key_b64, None, JValue::obj().with("x", JValue::Num("1.50".into())), 5), &friend, NOW).unwrap();
    let post = creator_post(&r.creator, "live1", NOW, None);
    let mut live = Packet::new("POST", &r.creator.public_key_b64, None, post, 6);
    live.signature = None;
    let items = JValue::Arr(vec![
        JValue::obj().with("onion", JValue::str(&friend.onion)).with("packet", own.to_value()),
        JValue::obj().with("onion", JValue::str(&friend.onion)).with("packet", relay.to_value()),
        JValue::obj().with("onion", JValue::str(&friend.onion)).with("packet", live.to_value()),
        JValue::obj().with("onion", JValue::str("example.com")).with("packet", relay.to_value()),
    ]);
    let wire = JValue::obj().with("items", items).render();
    let res = api::handle(&r.node, "hub_send", &json::parse(&wire).unwrap()).await.unwrap().unwrap();
    assert_eq!(res["queued"], 3);
    assert_eq!(res["rejected"].as_array().unwrap().len(), 1, "never dials a non-onion host");
    settle().await;
    let sent = r.sent.lock().unwrap().clone();
    for (_, line) in &sent {
        let p = Packet::from_wire(line).unwrap();
        assert_eq!(envelope::verify(&p, NOW), envelope::Verdict::Valid, "{} goes out with a valid envelope", p.ptype);
    }
    assert!(sent.iter().any(|(_, l)| l == &relay.to_wire()), "a relay is passed on byte for byte");
    assert!(r.node.store.post_get("live1").await.unwrap().is_some(), "the creator's live POST is kept for followers");
    // DELETE_POST from the creator removes it.
    let sig = r.creator.sign(&canon::efs(&["live1", &r.creator.public_key_b64, &NOW.to_string()]));
    let del = Packet::new("DELETE_POST", &r.creator.public_key_b64, None, JValue::obj().with("post_id", JValue::str("live1")).with("author_id", JValue::str(&r.creator.public_key_b64)).with("timestamp", JValue::int(NOW)).with("signature", JValue::str(sig)), 6);
    let items = JValue::Arr(vec![JValue::obj().with("onion", JValue::str(&friend.onion)).with("packet", del.to_value())]);
    api::handle(&r.node, "hub_send", &JValue::obj().with("items", items)).await.unwrap().unwrap();
    assert!(r.node.store.post_get("live1").await.unwrap().is_none());
}

#[tokio::test]
async fn owner_requests_must_be_signed_fresh_and_unique() {
    let r = rig(true).await;
    let nonces = api::NonceCache::default();
    let body = br#"{"cmd":"hub_status","args":{}}"#;
    let mk = |id: &Identity, ts: i64, nonce: &str| {
        let sig = id.sign(&canon::hub_api_request("POST", "/api/invoke", ts, nonce, &envelope::sha256_hex(body)));
        HashMap::from([
            ("x-noslop-hub-key".to_string(), id.public_key_b64.clone()), ("x-noslop-hub-ts".to_string(), ts.to_string()),
            ("x-noslop-hub-nonce".to_string(), nonce.to_string()), ("x-noslop-hub-sig".to_string(), sig),
        ])
    };
    let h = mk(&r.owner, NOW, "nonce_aaaaaaaaaaaaaaaa");
    assert!(api::verify_owner_request(&r.node, &nonces, "POST", "/api/invoke", &h, body).await.is_ok());
    assert!(api::verify_owner_request(&r.node, &nonces, "POST", "/api/invoke", &h, body).await.is_err(), "replay");
    assert!(api::verify_owner_request(&r.node, &nonces, "POST", "/api/invoke", &mk(&r.creator, NOW, "nonce_bbbbbbbbbbbbbbbb"), body).await.is_err(), "not the owner");
    assert!(api::verify_owner_request(&r.node, &nonces, "POST", "/api/invoke", &mk(&r.owner, NOW - 10 * 60_000, "nonce_cccccccccccccccc"), body).await.is_err(), "stale");
    assert!(api::verify_owner_request(&r.node, &nonces, "POST", "/api/invoke", &mk(&r.owner, NOW, "nonce_dddddddddddddddd"), b"{}").await.is_err(), "body changed");
    assert!(api::verify_owner_request(&r.node, &nonces, "POST", "/api/invoke", &HashMap::new(), body).await.is_err(), "unsigned");
}

#[tokio::test]
async fn admin_ai_messages_open_as_v2_and_replies_carry_an_ack() {
    let r = rig(true).await;
    let me = admin::admin_id(&r.owner);
    let secret = r.owner.x25519_secret();
    let (ct, iv) = crate::dm::encrypt_v2(r#"{"content":"hello hub"}"#, &r.owner.enc_public_key_b64, &secret, &r.owner.public_key_b64, &me, "m1", None, NOW, crate::dm::DM_AAD_MESSAGE).unwrap();
    let pkt = Packet::new("MESSAGE", &r.owner.public_key_b64, Some(&me), JValue::obj().with("id", JValue::str("m1")).with("nonce", JValue::str(iv)).with("ciphertext", JValue::str(ct)).with("timestamp", JValue::int(NOW)).with("v", JValue::int(2)), 3);
    assert!(admin::is_owner_to_admin(&r.owner, &pkt));
    assert_eq!(admin::open_owner_message(&r.owner, &pkt).unwrap(), ("m1".to_string(), "hello hub".to_string()));
    let reply = admin::reply_packet(&r.owner, "hi owner", NOW).unwrap();
    let ack = admin::ack_packet(&r.owner, "m1", NOW).unwrap();
    let rp = reply.payload.as_ref().unwrap();
    // What DmPacketHandler does on the phone: sender = admin_<key>, recipient = owner, their key = the admin contact's (= own).
    let text = crate::dm::decrypt_v2(rp.get_str("ciphertext").unwrap(), rp.get_str("nonce").unwrap(), &r.owner.enc_public_key_b64, &secret, &me, &r.owner.public_key_b64, rp.get_str("id").unwrap(), None, NOW, crate::dm::DM_AAD_MESSAGE).unwrap();
    assert_eq!(text, r#"{"content":"hi owner"}"#);
    let ap = ack.payload.as_ref().unwrap();
    let a = crate::dm::decrypt_v2(ap.get_str("ack_ciphertext").unwrap(), ap.get_str("ack_nonce").unwrap(), &r.owner.enc_public_key_b64, &secret, &me, &r.owner.public_key_b64, "m1", None, NOW, crate::dm::DM_AAD_ACK).unwrap();
    assert_eq!(a, "ACK");
    // hub_send hands the owner's Admin AI message to the hook instead of Tor.
    let items = JValue::Arr(vec![JValue::obj().with("onion", JValue::str(&r.owner.onion)).with("packet", pkt.to_value())]);
    let res = api::handle(&r.node, "hub_send", &JValue::obj().with("items", items)).await.unwrap().unwrap();
    assert_eq!(res["handled_by_hub"], 1);
}

#[tokio::test]
async fn presence_announces_the_creator_to_followers_and_discovery_to_trusted_contacts() {
    let r = rig(true).await;
    let (fan, _) = ident(7);
    accept_follower(&r, &fan).await;
    let (friend, _) = ident(6);
    let c = JValue::obj().with("public_key", JValue::str(&friend.public_key_b64)).with("identity", JValue::str("personal")).with("relationship", JValue::str("ACCEPTED"))
        .with("is_trusted", JValue::Bool(true)).with("onion_address", JValue::str(&friend.onion));
    api::handle(&r.node, "hub_contacts_push", &JValue::obj().with("contacts", JValue::Arr(vec![c])).with("full", JValue::Bool(false))).await.unwrap().unwrap();
    r.sent.lock().unwrap().clear();
    creator::announce_presence(&r.node).await;
    settle().await;
    let ann = sent_of(&r, "ANNOUNCE_PEER");
    assert_eq!(ann.len(), 1, "presence only to the creator's own contacts");
    assert_eq!(ann[0].0, fan.onion);
    let a = &ann[0].1;
    assert!(keys::verify(&canon::announce_peer_v2(&r.creator.public_key_b64, Some(&r.creator.onion), NOW), a.p_str("signature").unwrap(), &r.creator.public_key_b64));
    let disc = sent_of(&r, "ANNOUNCE_DISCOVERABLE");
    assert_eq!(disc.len(), 2);
    for (onion, d) in &disc {
        assert_eq!(envelope::verify(d, NOW), envelope::Verdict::Valid);
        assert_eq!(d.hops, Some(if *onion == fan.onion { 4 } else { 3 }));
        let s = canon::discoverable(&r.creator.public_key_b64, "Studio", &r.creator.onion, &r.creator.enc_public_key_b64, true, Some("https://example.org/fund"), None, Some("films"), NOW);
        assert!(keys::verify(&s, d.p_str("signature").unwrap(), &r.creator.public_key_b64));
    }
}

#[tokio::test]
async fn follower_comments_on_creator_posts_are_kept_and_served() {
    let r = rig(true).await;
    let (fan, _) = ident(7);
    accept_follower(&r, &fan).await;
    api::handle(&r.node, "creator_content_sync", &JValue::obj().with("posts", JValue::Arr(vec![creator_post(&r.creator, "p1", NOW - 10_000, None)]))).await.unwrap().unwrap();
    let ts = NOW - 100;
    let sig = fan.sign(&canon::encode_for_signing(&[Some("p1"), Some("c1"), Some("nice"), Some(&ts.to_string()), None]));
    let payload = JValue::obj().with("post_id", JValue::str("p1")).with("comment", JValue::obj().with("id", JValue::str("c1")).with("author_id", JValue::str(&fan.public_key_b64))
        .with("author_name", JValue::str("Fan")).with("content", JValue::str("nice")).with("timestamp", JValue::int(ts)).with("signature", JValue::str(sig)));
    r.node.handle_frame(&signed(Packet::new("COMMENT", &fan.public_key_b64, Some(&r.creator.public_key_b64), payload, 1), &fan, NOW), Listener::Creator).await;
    let req = Packet::new("SYNC_REQUEST", &fan.public_key_b64, Some(&r.creator.public_key_b64), JValue::obj().with("since", JValue::int(0)), 3);
    r.node.handle_frame(&signed(req, &fan, NOW), Listener::Creator).await;
    tokio::time::sleep(std::time::Duration::from_millis(700)).await;
    let out = sent_of(&r, "SYNC_RESPONSE");
    let comments: Vec<_> = out.iter().flat_map(|(_, p)| p.p("comments").unwrap().as_array().unwrap().clone()).collect();
    assert_eq!(comments.len(), 1);
    content::verify_comment(&comments[0]).unwrap();
    // The phone gets the comment too.
    assert!(r.node.store.inbox_pull(0, 10, 1 << 20).await.unwrap().0.iter().any(|i| i.packet.contains("\"COMMENT\"")));
}

#[tokio::test]
async fn inbox_survives_until_acknowledged_and_legacy_pull_drains() {
    let r = rig(true).await;
    let (stranger, _) = ident(9);
    for i in 0..3 {
        let mut p = Packet::new("TYPING", &stranger.public_key_b64, Some(&r.owner.public_key_b64), JValue::obj().with("n", JValue::int(i)), 3);
        p.id = Some(format!("t{}", i));
        r.node.handle_frame(&signed(p, &stranger, NOW), Listener::Personal).await;
    }
    let res = api::handle(&r.node, "hub_inbox_pull", &JValue::obj().with("after", JValue::int(0)).with("limit", JValue::int(2))).await.unwrap().unwrap();
    assert_eq!(res["items"].as_array().unwrap().len(), 2);
    assert_eq!(res["more"], true);
    let again = api::handle(&r.node, "hub_inbox_pull", &JValue::obj().with("after", JValue::int(0))).await.unwrap().unwrap();
    assert_eq!(again["items"].as_array().unwrap().len(), 3, "nothing is lost before an ACK");
    let last = res["items"][1]["seq"].as_i64().unwrap();
    api::handle(&r.node, "hub_inbox_ack", &JValue::obj().with("up_to", JValue::int(last))).await.unwrap().unwrap();
    let legacy = api::handle(&r.node, "sync_pull_packets", &JValue::obj()).await.unwrap().unwrap();
    assert_eq!(legacy["packets"].as_array().unwrap().len(), 1);
    assert_eq!(r.node.store.inbox_stats().await.unwrap().0, 0);
}

#[tokio::test]
async fn backups_keep_a_sha256_sidecar_and_prune_with_it() {
    let r = rig(true).await;
    let dir = r.node.cfg.backup_dir();
    std::fs::create_dir_all(&dir).unwrap();
    // Older backups (and their sidecars) beyond the keep limit must go together.
    let keep = r.node.cfg.backup_keep.max(1);
    for i in 0..keep {
        let n = format!("noslop_backup_{}.bin", NOW - 1000 - i as i64);
        std::fs::write(dir.join(&n), b"old").unwrap();
        std::fs::write(dir.join(&n).with_extension("sha256"), b"oldsha").unwrap();
    }
    let tmp = dir.join("upload.tmp");
    std::fs::write(&tmp, b"archive").unwrap();
    let sha = envelope::sha256_hex(b"archive");
    let res = api::backup_store(&r.node, &tmp, &sha, 7).await.unwrap();
    assert_eq!(res["sha256"], sha);
    let latest = api::backup_latest(&r.node).await.unwrap();
    assert_eq!(std::fs::read_to_string(latest.with_extension("sha256")).unwrap(), sha);
    let names = api::backup_list(&r.node).await;
    assert_eq!(names.len(), keep, "listing holds backups only, within the limit");
    assert!(names.iter().all(|n| n.ends_with(".bin")));
    let sidecars = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".sha256")).count();
    assert_eq!(sidecars, keep, "the pruned backup's sidecar is removed too");
    assert!(!dir.join(format!("noslop_backup_{}.sha256", NOW - 1000 - (keep as i64 - 1))).exists());
}

// --- NOSLOP_HUB_DM_VIEW_V1 ------------------------------------------------------------------------------

fn dm_packet(from: &Identity, to: &Identity, id: &str, body: &str, group: Option<&str>, ts: i64) -> Packet {
    let (ct, iv) = crate::dm::encrypt_v2(body, &to.enc_public_key_b64, &from.x25519_secret(), &from.public_key_b64, &to.public_key_b64, id, group, ts, crate::dm::DM_AAD_MESSAGE).unwrap();
    let payload = JValue::obj().with("id", JValue::str(id)).with("nonce", JValue::str(iv)).with("ciphertext", JValue::str(ct))
        .with_opt("group_id", group.map(JValue::str)).with("timestamp", JValue::int(ts)).with("v", JValue::int(2));
    Packet::new("MESSAGE", &from.public_key_b64, Some(&to.public_key_b64), payload, 1)
}

async fn add_contact(r: &Rig, who: &Identity, identity: &str) {
    let c = store::Contact {
        public_key: who.public_key_b64.clone(), identity: identity.into(), relationship: "ACCEPTED".into(), is_trusted: true,
        is_temporary: false, handle: "Gabby".into(), onion_address: who.onion.clone(), enc_public_key: who.enc_public_key_b64.clone(),
        source: "phone".into(), declined_nonce: String::new(),
    };
    r.node.store.contact_upsert(&c, NOW).await.unwrap();
}

fn capture_dms(r: &Rig) -> Arc<StdMutex<Vec<dms::DmRecord>>> {
    let got = Arc::new(StdMutex::new(Vec::new()));
    let g = got.clone();
    let node = r.node.clone();
    futures_lite_block(async move {
        node.set_dm_hook(Arc::new(move |rec| { g.lock().unwrap().push(rec); Box::pin(async {}) })).await;
    });
    got
}

/// Runs a short future to completion inside the current runtime (test helper).
fn futures_lite_block<F: std::future::Future<Output = ()> + Send + 'static>(f: F) {
    tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(f));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dms_to_either_identity_are_opened_for_the_web_ui_and_still_reach_the_phone() {
    let r = rig(true).await;
    let got = capture_dms(&r);
    let (gabby, _) = ident(7);
    add_contact(&r, &gabby, "creator").await;
    // To the creator identity (Gabby follows the creator), with a media attachment.
    let body = r#"{"content":"hi creator 😀","media":{"id":"m1","type":"image"}}"#;
    let to_creator = signed(dm_packet(&gabby, &r.creator, "dm-1", body, None, NOW - 5), &gabby, NOW);
    r.node.handle_frame(&to_creator, Listener::Creator).await;
    // To the personal identity, plain text.
    let to_me = signed(dm_packet(&gabby, &r.owner, "dm-2", "plain hello", None, NOW - 4), &gabby, NOW);
    r.node.handle_frame(&to_me, Listener::Personal).await;
    settle().await;
    let recs = got.lock().unwrap().clone();
    assert_eq!(recs.len(), 2, "both opened: {:?}", recs);
    let a = recs.iter().find(|d| d.id == "dm-1").unwrap();
    assert_eq!((a.peer.as_str(), a.sender.as_str(), a.content.as_str()), (gabby.public_key_b64.as_str(), gabby.public_key_b64.as_str(), "hi creator 😀"));
    assert_eq!((a.media_id.as_deref(), a.media_type.as_deref(), a.timestamp, a.incoming), (Some("m1"), Some("image"), NOW - 5, true));
    let b = recs.iter().find(|d| d.id == "dm-2").unwrap();
    assert_eq!((b.content.as_str(), b.media_id.clone()), ("plain hello", None));
    // The phone still gets both packets, unchanged.
    let (items, _) = r.node.store.inbox_pull(0, 10, 1 << 20).await.unwrap();
    assert!(items.iter().any(|i| i.packet == to_creator) && items.iter().any(|i| i.packet == to_me));
    // Two notification events, in arrival order; a re-delivery adds none.
    r.node.store.dm_event_add("dm-1", &gabby.public_key_b64, &gabby.public_key_b64, "again", NOW).await.unwrap();
    let res = api::handle(&r.node, "hub_dm_events", &JValue::obj().with("after", JValue::int(0))).await.unwrap().unwrap();
    let ev = res["events"].as_array().unwrap();
    assert_eq!(ev.len(), 2);
    assert_eq!(ev.iter().map(|e| e["preview"].as_str().unwrap()).collect::<Vec<_>>().len(), 2);
    assert!(ev.iter().any(|e| e["preview"] == "hi creator 😀") && ev.iter().any(|e| e["preview"] == "plain hello"));
    assert_eq!(res["latest"], ev[1]["seq"]);
    let none = api::handle(&r.node, "hub_dm_events", &JValue::obj().with("after", res["latest"].as_i64().map(JValue::int).unwrap())).await.unwrap().unwrap();
    assert!(none["events"].as_array().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_owners_outgoing_dms_show_without_a_notification() {
    let r = rig(true).await;
    let got = capture_dms(&r);
    let (gabby, _) = ident(7);
    add_contact(&r, &gabby, "creator").await;
    let out = envelope::sign(&dm_packet(&r.creator, &gabby, "dm-out", r#"{"content":"thanks!"}"#, None, NOW), &r.creator, NOW).unwrap();
    let items = JValue::Arr(vec![JValue::obj().with("onion", JValue::str(&gabby.onion)).with("packet", out.to_value())]);
    let res = api::handle(&r.node, "hub_send", &JValue::obj().with("items", items)).await.unwrap().unwrap();
    assert_eq!(res["queued"], 1, "still delivered");
    let recs = got.lock().unwrap().clone();
    assert_eq!(recs.len(), 1);
    assert_eq!((recs[0].peer.as_str(), recs[0].sender.as_str(), recs[0].content.as_str(), recs[0].incoming),
        (gabby.public_key_b64.as_str(), r.creator.public_key_b64.as_str(), "thanks!", false));
    let ev = api::handle(&r.node, "hub_dm_events", &JValue::obj()).await.unwrap().unwrap();
    assert!(ev["events"].as_array().unwrap().is_empty(), "no notification for our own message");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn group_unknown_sender_forged_and_admin_dms_are_not_opened() {
    let r = rig(true).await;
    let got = capture_dms(&r);
    let (gabby, _) = ident(7);
    let (stranger, _) = ident(8);
    add_contact(&r, &gabby, "personal").await;
    // A group message: keys come from a group directory the Hub does not keep.
    r.node.handle_frame(&signed(dm_packet(&gabby, &r.owner, "g-1", "group hi", Some("grp"), NOW), &gabby, NOW), Listener::Personal).await;
    // From someone who is not a contact (no X25519 key on record).
    r.node.handle_frame(&signed(dm_packet(&stranger, &r.owner, "s-1", "who am i", None, NOW), &stranger, NOW), Listener::Personal).await;
    // Signed by Gabby, but the ciphertext is the stranger's (wrong keys): fails authentication.
    let mut forged = dm_packet(&stranger, &r.owner, "f-1", "forged", None, NOW);
    forged.sender_id = gabby.public_key_b64.clone();
    r.node.handle_frame(&signed(forged, &gabby, NOW), Listener::Personal).await;
    // The owner's conversation with the Admin AI is handled elsewhere.
    let admin = envelope::sign(&admin_dm(&r.owner, "a-1", "status?"), &r.owner, NOW).unwrap();
    let items = JValue::Arr(vec![JValue::obj().with("onion", JValue::str(&r.owner.onion)).with("packet", admin.to_value())]);
    api::handle(&r.node, "hub_send", &JValue::obj().with("items", items)).await.unwrap().unwrap();
    settle().await;
    assert!(got.lock().unwrap().is_empty(), "{:?}", got.lock().unwrap());
    let ev = api::handle(&r.node, "hub_dm_events", &JValue::obj()).await.unwrap().unwrap();
    assert!(ev["events"].as_array().unwrap().is_empty());
}

fn admin_dm(owner: &Identity, id: &str, text: &str) -> Packet {
    let admin = admin::admin_id(owner);
    let (ct, iv) = crate::dm::encrypt_v2(text, &owner.enc_public_key_b64, &owner.x25519_secret(), &owner.public_key_b64, &admin, id, None, NOW, crate::dm::DM_AAD_MESSAGE).unwrap();
    let payload = JValue::obj().with("id", JValue::str(id)).with("nonce", JValue::str(iv)).with("ciphertext", JValue::str(ct)).with("timestamp", JValue::int(NOW)).with("v", JValue::int(2));
    Packet::new("MESSAGE", &owner.public_key_b64, Some(&admin), payload, 1)
}

#[tokio::test]
async fn a_schema_v1_database_gains_the_dm_events_table() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
    sqlx::query("CREATE TABLE nh_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO nh_meta (key, value) VALUES ('schema_version', '1')").execute(&pool).await.unwrap();
    let st = store::Store::open(pool).await.unwrap();
    assert_eq!(st.meta_get("schema_version").await.unwrap().as_deref(), Some("2"));
    assert!(st.dm_event_add("x", "p", "p", "hi", NOW).await.unwrap());
    assert!(!st.dm_event_add("x", "p", "p", "hi", NOW).await.unwrap(), "once per message id");
}

// --- NOSLOP_HUB_WEBUI_SEND_V1 ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn web_ui_dms_go_out_from_the_identity_each_contact_knows() {
    let r = rig(true).await;
    let (gabby, _) = ident(7);
    let (friend, _) = ident(9);
    add_contact(&r, &gabby, "creator").await;
    add_contact(&r, &friend, "personal").await;
    let a = dms::send_from_web_ui(&r.node, &gabby.public_key_b64, "hello from the Hub ✨").await.unwrap();
    let b = dms::send_from_web_ui(&r.node, &friend.public_key_b64, "hi friend").await.unwrap();
    assert_eq!((a.sender.as_str(), a.incoming), (r.creator.public_key_b64.as_str(), false), "a creator follower hears from the creator");
    assert_eq!(b.sender, r.owner.public_key_b64);
    settle().await;
    let out = sent_of(&r, "MESSAGE");
    assert_eq!(out.len(), 2, "{:?}", out);
    for (onion, pkt, to, me, text) in [(&gabby.onion, &a, &gabby, &r.creator, "hello from the Hub ✨"), (&friend.onion, &b, &friend, &r.owner, "hi friend")] {
        let (o, p) = out.iter().find(|(_, p)| p.p_str("id") == Some(pkt.id.as_str())).unwrap();
        assert_eq!(o, onion);
        assert_eq!((p.sender_id.as_str(), p.target_user_id.as_deref()), (me.public_key_b64.as_str(), Some(to.public_key_b64.as_str())));
        assert_eq!(envelope::verify(p, NOW), envelope::Verdict::Valid, "signed by the sending identity");
        // The contact opens it exactly as DmPacketHandler does.
        let pl = p.payload.as_ref().unwrap();
        let plain = crate::dm::decrypt_v2(pl.get_str("ciphertext").unwrap(), pl.get_str("nonce").unwrap(), &me.enc_public_key_b64, &to.x25519_secret(),
            &me.public_key_b64, &to.public_key_b64, &pkt.id, None, pl.get_i64("timestamp").unwrap(), crate::dm::DM_AAD_MESSAGE).unwrap();
        assert_eq!(dms::parse_body(&plain).0, text);
        assert_eq!(pl.get_i64("v"), Some(2));
    }
}

#[tokio::test]
async fn web_ui_dms_are_refused_without_keys_or_to_blocked_contacts() {
    let r = rig(false).await;
    let (gabby, _) = ident(7);
    let (stranger, _) = ident(8);
    assert!(dms::send_from_web_ui(&r.node, &stranger.public_key_b64, "hi").await.is_err(), "not a contact");
    add_contact(&r, &gabby, "personal").await;
    let mut c = r.node.store.contact_get(&gabby.public_key_b64).await.unwrap().unwrap();
    c.relationship = "BLOCKED".into();
    r.node.store.contact_upsert(&c, NOW).await.unwrap();
    assert!(dms::send_from_web_ui(&r.node, &gabby.public_key_b64, "hi").await.is_err(), "blocked");
    c.relationship = "ACCEPTED".into();
    c.enc_public_key = String::new();
    r.node.store.contact_upsert(&c, NOW).await.unwrap();
    assert!(dms::send_from_web_ui(&r.node, &gabby.public_key_b64, "hi").await.is_err(), "no X25519 key yet");
    assert!(dms::send_from_web_ui(&r.node, &gabby.public_key_b64, "").await.is_err(), "empty");
    assert!(sent_of(&r, "MESSAGE").is_empty());
}
