//! START OF FILE hainet-noslop/src/packet.rs
//!
//! `NetworkPacket` (`mesh/Packets.kt`) with its payload kept as an order-preserving [`JValue`], so
//! the envelope digest of a received packet can be reproduced and a relayed packet is passed on
//! byte-for-byte equivalent.

use crate::json::{self, JValue, JsonError};

/// Frame ceiling for one newline-delimited packet. The phone's listener uses the same order of
/// magnitude (`FrameReader.maxChars`); a 256 KB media chunk is ~350 KB as Base64 inside JSON.
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub struct Packet {
    pub id: Option<String>,
    pub hops: Option<i64>,
    pub sender_id: String,
    pub target_user_id: Option<String>,
    pub signature: Option<String>,
    pub ptype: String,
    pub payload: Option<JValue>,
    pub env_v: Option<i64>,
    pub env_ts: Option<i64>,
    pub env_sig: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum PacketError {
    #[error("json: {0}")]
    Json(#[from] JsonError),
    #[error("packet is not a JSON object")]
    NotObject,
    #[error("missing or invalid field {0}")]
    Field(&'static str),
    #[error("frame too large")]
    TooLarge,
}

fn opt_str(v: &JValue, k: &'static str) -> Result<Option<String>, PacketError> {
    match v.get(k) {
        None | Some(JValue::Null) => Ok(None),
        Some(JValue::Str(s)) => Ok(Some(s.clone())),
        _ => Err(PacketError::Field(k)),
    }
}

fn opt_int(v: &JValue, k: &'static str) -> Result<Option<i64>, PacketError> {
    match v.get(k) {
        None | Some(JValue::Null) => Ok(None),
        Some(n @ JValue::Num(_)) => n.as_i64().map(Some).ok_or(PacketError::Field(k)),
        _ => Err(PacketError::Field(k)),
    }
}

impl Packet {
    /// A new packet with no envelope (sign it with [`crate::envelope::sign`] before sending).
    pub fn new(ptype: &str, sender_id: &str, target: Option<&str>, payload: JValue, hops: i64) -> Self {
        Packet {
            id: Some(uuid::Uuid::new_v4().to_string()),
            hops: Some(hops),
            sender_id: sender_id.to_string(),
            target_user_id: target.map(|t| t.to_string()),
            signature: None,
            ptype: ptype.to_string(),
            payload: Some(payload),
            env_v: None,
            env_ts: None,
            env_sig: None,
        }
    }

    /// Parses one frame as Gson would read it into `NetworkPacket`. `sender_id` and `type` are required.
    pub fn from_wire(frame: &str) -> Result<Self, PacketError> {
        if frame.len() > MAX_FRAME_BYTES { return Err(PacketError::TooLarge); }
        let v = json::parse(frame)?;
        Self::from_value(&v)
    }

    pub fn from_value(v: &JValue) -> Result<Self, PacketError> {
        if !matches!(v, JValue::Obj(_)) { return Err(PacketError::NotObject); }
        let sender_id = opt_str(v, "sender_id")?.ok_or(PacketError::Field("sender_id"))?;
        let ptype = opt_str(v, "type")?.ok_or(PacketError::Field("type"))?;
        let payload = match v.get("payload") { None | Some(JValue::Null) => None, Some(p) => Some(p.clone()) };
        Ok(Packet {
            id: opt_str(v, "id")?,
            hops: opt_int(v, "hops")?,
            sender_id,
            target_user_id: opt_str(v, "target_user_id")?,
            signature: opt_str(v, "signature")?,
            ptype,
            payload,
            env_v: opt_int(v, "env_v")?,
            env_ts: opt_int(v, "env_ts")?,
            env_sig: opt_str(v, "env_sig")?,
        })
    }

    /// Gson's rendering of the packet (field declaration order, nulls left out).
    pub fn to_value(&self) -> JValue {
        JValue::obj()
            .with_opt("id", self.id.clone().map(JValue::Str))
            .with_opt("hops", self.hops.map(JValue::int))
            .with("sender_id", JValue::str(&self.sender_id))
            .with_opt("target_user_id", self.target_user_id.clone().map(JValue::Str))
            .with_opt("signature", self.signature.clone().map(JValue::Str))
            .with("type", JValue::str(&self.ptype))
            .with_opt("payload", self.payload.clone())
            .with_opt("env_v", self.env_v.map(JValue::int))
            .with_opt("env_ts", self.env_ts.map(JValue::int))
            .with_opt("env_sig", self.env_sig.clone().map(JValue::Str))
    }

    pub fn to_wire(&self) -> String { self.to_value().render() }

    /// Payload member as a string (`None` for absent or null).
    pub fn p_str(&self, key: &str) -> Option<&str> { self.payload.as_ref().and_then(|p| p.get_str(key)) }
    pub fn p_i64(&self, key: &str) -> Option<i64> { self.payload.as_ref().and_then(|p| p.get_i64(key)) }
    pub fn p(&self, key: &str) -> Option<&JValue> { self.payload.as_ref().and_then(|p| p.get(key)) }
}
