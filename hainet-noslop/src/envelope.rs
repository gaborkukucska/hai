//! START OF FILE hainet-noslop/src/envelope.rs
//!
//! The envelope signature every NoSlop packet carries since round E2 (`mesh/EnvelopeAuth.kt`,
//! WIRE_PROTOCOL_REFERENCE §1.1):
//!
//! `encodeForSigning("noslop-env-v1", id, type, sender_id, target_user_id, signature, env_ts, payloadDigest)`
//! signed by `sender_id`, where `payloadDigest` is the lower-case hex SHA-256 of Gson's rendering of
//! the payload with null object members removed (empty string for no payload). `hops` is not covered.

use crate::{canon::efs, json::JValue, keys::{self, Identity}, packet::Packet};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};

pub const VERSION: i64 = 1;
const LABEL: &str = "noslop-env-v1";
/// `EnvelopeAuth.MAX_SKEW_MS`: one hour either way.
pub const MAX_SKEW_MS: i64 = 60 * 60 * 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict { Valid, Missing, BadVersion, Stale, Invalid }

pub fn sha256_hex(bytes: &[u8]) -> String { hex::encode(Sha256::digest(bytes)) }

/// `EnvelopeAuth.payloadDigest`.
pub fn payload_digest(payload: Option<&JValue>) -> String {
    let text = match payload { None | Some(JValue::Null) => String::new(), Some(p) => p.without_null_members().render() };
    sha256_hex(text.as_bytes())
}

/// `EnvelopeAuth.signingString`.
pub fn signing_string(p: &Packet, env_ts: i64) -> String {
    efs(&[
        LABEL,
        p.id.as_deref().unwrap_or(""),
        &p.ptype,
        &p.sender_id,
        p.target_user_id.as_deref().unwrap_or(""),
        p.signature.as_deref().unwrap_or(""),
        &env_ts.to_string(),
        &payload_digest(p.payload.as_ref()),
    ])
}

/// Signs `p` as `keys`, which must be the identity named in `sender_id` (`EnvelopeAuth.sign`).
pub fn sign(p: &Packet, keys: &Identity, now_ms: i64) -> Result<Packet, &'static str> {
    if keys.public_key_b64 != p.sender_id { return Err("envelope must be signed by the sender"); }
    let sig = keys.sign(&signing_string(p, now_ms));
    let mut out = p.clone();
    out.env_v = Some(VERSION);
    out.env_ts = Some(now_ms);
    out.env_sig = Some(sig);
    Ok(out)
}

pub fn has_envelope(p: &Packet) -> bool {
    p.env_v.is_some() && p.env_ts.is_some() && p.env_sig.as_deref().map(|s| !s.trim().is_empty()).unwrap_or(false)
}

/// `EnvelopeAuth.verify`.
pub fn verify(p: &Packet, now_ms: i64) -> Verdict {
    let (Some(v), Some(ts), Some(sig)) = (p.env_v, p.env_ts, p.env_sig.as_deref()) else { return Verdict::Missing };
    if sig.trim().is_empty() { return Verdict::Missing; }
    if v != VERSION { return Verdict::BadVersion; }
    if (now_ms - ts).abs() > MAX_SKEW_MS { return Verdict::Stale; }
    if keys::verify(&signing_string(p, ts), sig, &p.sender_id) { Verdict::Valid } else { Verdict::Invalid }
}

/// Replay guard (`EnvelopeAuth.isReplay` / `recordProcessed`): remembers `sender|id|env_sig` for
/// twice the skew window, bounded to `capacity` entries (oldest evicted first).
pub struct ReplayGuard {
    seen: HashMap<String, i64>,
    order: VecDeque<String>,
    capacity: usize,
}

impl ReplayGuard {
    pub fn new(capacity: usize) -> Self { Self { seen: HashMap::new(), order: VecDeque::new(), capacity } }

    fn key(p: &Packet) -> String {
        format!("{}|{}|{}", p.sender_id, p.id.as_deref().unwrap_or(""), p.env_sig.as_deref().unwrap_or(""))
    }

    pub fn is_replay(&self, p: &Packet, now_ms: i64) -> bool {
        self.seen.get(&Self::key(p)).map(|at| now_ms - at <= MAX_SKEW_MS * 2).unwrap_or(false)
    }

    pub fn record(&mut self, p: &Packet, now_ms: i64) {
        let k = Self::key(p);
        if self.seen.insert(k.clone(), now_ms).is_none() {
            self.order.push_back(k);
            while self.order.len() > self.capacity {
                if let Some(old) = self.order.pop_front() { self.seen.remove(&old); }
            }
        }
    }

    /// Checks and records in one step; returns `true` when the packet is new.
    pub fn admit(&mut self, p: &Packet, now_ms: i64) -> bool {
        if self.is_replay(p, now_ms) { return false; }
        self.record(p, now_ms);
        true
    }
}
