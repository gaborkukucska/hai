//! START OF FILE hainet-noslop/src/keys.rs
//!
//! NoSlop key formats, read exactly as `CryptoService.kt` reads them.
//!
//! * Ed25519 public: Base64 of the 44-byte X.509 SubjectPublicKeyInfo (`302a300506032b6570032100 || key`),
//!   or a raw 32-byte key. A 48-byte value is a private key and is refused.
//! * Ed25519 private: Base64 of the 48-byte PKCS#8 (`302e020100300506032b657004220420 || seed`), or a
//!   raw 32/64-byte value (the first 32 bytes are the seed). A 44-byte value is a public key and is refused.
//! * X25519 public / private: X.509 / PKCS#8 as produced by Bouncy Castle's `SubjectPublicKeyInfoFactory`
//!   and `PrivateKeyInfoFactory` (the private form is the 83-byte v1 `OneAsymmetricKey` carrying the
//!   public key), or raw 32 bytes.
//!
//! Like `CryptoService`, a structure that does not parse falls back to its last 32 bytes.

use base64::{engine::general_purpose::{STANDARD, STANDARD_NO_PAD}, Engine as _};
use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey, Signature};
use sha2::{Digest as _, Sha512};
use sha3::Sha3_256;

pub const ED25519_PKCS8_HEADER: [u8; 16] = [0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20];
pub const ED25519_X509_HEADER: [u8; 12] = [0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00];

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum KeyError {
    #[error("not valid Base64")]
    Base64,
    #[error("wrong key kind: {0}")]
    WrongKind(&'static str),
    #[error("key too short")]
    TooShort,
    #[error("invalid key")]
    Invalid,
}

/// `android.util.Base64.decode(.., DEFAULT)`: standard alphabet, whitespace ignored, padding optional.
pub fn b64_decode(s: &str) -> Result<Vec<u8>, KeyError> {
    let cleaned: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    STANDARD.decode(&cleaned).or_else(|_| STANDARD_NO_PAD.decode(cleaned.trim_end_matches('='))).map_err(|_| KeyError::Base64)
}

/// `Base64.encodeToString(.., NO_WRAP)`.
pub fn b64_encode(bytes: &[u8]) -> String { STANDARD.encode(bytes) }

// --- Minimal DER reader (enough for SPKI and PKCS#8 / OneAsymmetricKey) -------------------------

fn der_tlv(b: &[u8]) -> Option<(u8, &[u8], &[u8])> {
    if b.len() < 2 { return None; }
    let tag = b[0];
    let (len, hdr) = if b[1] & 0x80 == 0 {
        (b[1] as usize, 2)
    } else {
        let n = (b[1] & 0x7f) as usize;
        if n == 0 || n > 2 || b.len() < 2 + n { return None; }
        let mut l = 0usize;
        for i in 0..n { l = (l << 8) | b[2 + i] as usize; }
        (l, 2 + n)
    };
    if b.len() < hdr + len { return None; }
    Some((tag, &b[hdr..hdr + len], &b[hdr + len..]))
}

fn der_children(content: &[u8]) -> Option<Vec<(u8, &[u8])>> {
    let mut out = Vec::new();
    let mut rest = content;
    while !rest.is_empty() {
        let (t, c, r) = der_tlv(rest)?;
        out.push((t, c));
        rest = r;
    }
    Some(out)
}

/// The raw 32-byte key of a SubjectPublicKeyInfo: `SEQUENCE { SEQUENCE alg, BIT STRING 00 || key }`.
fn spki_raw(der: &[u8]) -> Option<[u8; 32]> {
    let (t, c, rest) = der_tlv(der)?;
    if t != 0x30 || !rest.is_empty() { return None; }
    let kids = der_children(c)?;
    if kids.len() != 2 || kids[0].0 != 0x30 || kids[1].0 != 0x03 { return None; }
    let bits = kids[1].1;
    if bits.len() != 33 || bits[0] != 0 { return None; }
    bits[1..].try_into().ok()
}

/// The raw 32-byte private key of a PKCS#8 / OneAsymmetricKey:
/// `SEQUENCE { INTEGER, SEQUENCE alg, OCTET STRING { OCTET STRING key }, ... }`.
fn pkcs8_raw(der: &[u8]) -> Option<[u8; 32]> {
    let (t, c, rest) = der_tlv(der)?;
    if t != 0x30 || !rest.is_empty() { return None; }
    let kids = der_children(c)?;
    if kids.len() < 3 || kids[0].0 != 0x02 || kids[1].0 != 0x30 || kids[2].0 != 0x04 { return None; }
    let (it, ic, irest) = der_tlv(kids[2].1)?;
    if it != 0x04 || !irest.is_empty() || ic.len() != 32 { return None; }
    ic.try_into().ok()
}

fn last32(b: &[u8]) -> Result<[u8; 32], KeyError> {
    if b.len() < 32 { return Err(KeyError::TooShort); }
    Ok(b[b.len() - 32..].try_into().unwrap())
}

/// Raw Ed25519 public key from NoSlop's Base64 form (`getEd25519PublicKeyParams`).
pub fn ed25519_public_raw(b64: &str) -> Result<[u8; 32], KeyError> {
    let b = b64_decode(b64)?;
    if b.len() == 48 { return Err(KeyError::WrongKind("48-byte PKCS#8 private key given as a public key")); }
    if b.len() == 32 { return Ok(b.try_into().unwrap()); }
    spki_raw(&b).map(Ok).unwrap_or_else(|| last32(&b))
}

/// Raw Ed25519 seed from NoSlop's Base64 form (`getEd25519PrivateKeyParams`).
pub fn ed25519_seed(b64: &str) -> Result<[u8; 32], KeyError> {
    let b = b64_decode(b64)?;
    if b.len() == 44 { return Err(KeyError::WrongKind("44-byte X.509 public key given as a private key")); }
    if b.len() == 32 || b.len() == 64 { return Ok(b[..32].try_into().unwrap()); }
    pkcs8_raw(&b).map(Ok).unwrap_or_else(|| last32(&b))
}

/// Raw X25519 public key (`getX25519PublicKeyParams`).
pub fn x25519_public_raw(b64: &str) -> Result<[u8; 32], KeyError> {
    let b = b64_decode(b64)?;
    if b.len() == 32 { return Ok(b.try_into().unwrap()); }
    spki_raw(&b).map(Ok).unwrap_or_else(|| last32(&b))
}

/// Raw X25519 private key (`getX25519PrivateKeyParams`).
pub fn x25519_private_raw(b64: &str) -> Result<[u8; 32], KeyError> {
    let b = b64_decode(b64)?;
    if b.len() == 32 { return Ok(b.try_into().unwrap()); }
    pkcs8_raw(&b).map(Ok).unwrap_or_else(|| last32(&b))
}

/// NoSlop's Base64 public key string for a raw Ed25519 key (X.509 wrapped).
pub fn ed25519_public_b64(raw: &[u8; 32]) -> String {
    let mut v = ED25519_X509_HEADER.to_vec();
    v.extend_from_slice(raw);
    b64_encode(&v)
}

/// `CryptoService.deriveOnionAddress`: the Tor v3 address of an Ed25519 key.
pub fn onion_of_raw(raw: &[u8; 32]) -> String {
    let mut h = Sha3_256::new();
    h.update(b".onion checksum");
    h.update(raw);
    h.update([0x03u8]);
    let hash = h.finalize();
    let mut data = Vec::with_capacity(35);
    data.extend_from_slice(raw);
    data.extend_from_slice(&hash[..2]);
    data.push(0x03);
    format!("{}.onion", base32::encode(base32::Alphabet::Rfc4648 { padding: false }, &data).to_lowercase())
}

/// The onion of a NoSlop Base64 public key, or `None` when the key can't be read (`onionOf`).
pub fn onion_of(pub_b64: &str) -> Option<String> { ed25519_public_raw(pub_b64).ok().map(|r| onion_of_raw(&r)) }

/// `CryptoService.isOnionOf` (case-insensitive, trimmed).
pub fn is_onion_of(pub_b64: &str, onion: &str) -> bool {
    onion_of(pub_b64).map(|o| o.eq_ignore_ascii_case(onion.trim())).unwrap_or(false)
}

/// Tor's `hs_ed25519_secret_key` body for a seed: SHA-512(seed), clamped (`getRawEd25519Seed`).
pub fn tor_expanded_secret(seed: &[u8; 32]) -> [u8; 64] {
    let mut e: [u8; 64] = Sha512::digest(seed).into();
    e[0] &= 248;
    e[31] &= 127;
    e[31] |= 64;
    e
}

/// The file Tor reads from a `HiddenServiceDir` (`hs_ed25519_secret_key`).
pub fn tor_secret_key_file(seed: &[u8; 32]) -> Vec<u8> {
    let mut v = b"== ed25519v1-secret: type0 ==\0\0\0".to_vec();
    v.extend_from_slice(&tor_expanded_secret(seed));
    v
}

/// `CryptoService.verify`: Ed25519 over the UTF-8 bytes of `payload`. Never panics.
pub fn verify(payload: &str, signature_b64: &str, pub_b64: &str) -> bool {
    let Ok(raw) = ed25519_public_raw(pub_b64) else { return false };
    let Ok(vk) = VerifyingKey::from_bytes(&raw) else { return false };
    let Ok(sig_bytes) = b64_decode(signature_b64) else { return false };
    let Ok(sig) = Signature::from_slice(&sig_bytes) else { return false };
    vk.verify(payload.as_bytes(), &sig).is_ok()
}

/// One NoSlop identity as held by the Hub (the owner's personal or creator identity).
#[derive(Clone)]
pub struct Identity {
    signing: SigningKey,
    /// The exact Base64 public key string the phone uses as `sender_id` (X.509 wrapped).
    pub public_key_b64: String,
    pub onion: String,
    /// X25519 keys in the phone's Base64 form (as given by the phone).
    pub enc_public_key_b64: String,
    enc_private_raw: [u8; 32],
}

impl std::fmt::Debug for Identity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Identity").field("public_key_b64", &self.public_key_b64).field("onion", &self.onion).finish_non_exhaustive()
    }
}

impl Identity {
    /// Builds an identity from the phone's Base64 key strings, checking that the private keys match
    /// the public ones (a mismatch would publish an onion nobody can reach).
    pub fn from_b64(public_key_b64: &str, private_key_b64: &str, enc_public_key_b64: &str, enc_private_key_b64: &str) -> Result<Self, KeyError> {
        let seed = ed25519_seed(private_key_b64)?;
        let signing = SigningKey::from_bytes(&seed);
        let raw_pub = ed25519_public_raw(public_key_b64)?;
        if signing.verifying_key().to_bytes() != raw_pub { return Err(KeyError::Invalid); }
        let enc_private_raw = x25519_private_raw(enc_private_key_b64)?;
        let derived_x = x25519_dalek::PublicKey::from(&x25519_dalek::StaticSecret::from(enc_private_raw));
        if derived_x.to_bytes() != x25519_public_raw(enc_public_key_b64)? { return Err(KeyError::Invalid); }
        Ok(Self {
            signing,
            public_key_b64: public_key_b64.trim().to_string(),
            onion: onion_of_raw(&raw_pub),
            enc_public_key_b64: enc_public_key_b64.trim().to_string(),
            enc_private_raw,
        })
    }

    pub fn seed(&self) -> [u8; 32] { self.signing.to_bytes() }

    /// `CryptoService.sign`: Base64 Ed25519 signature over the UTF-8 bytes of `payload`.
    pub fn sign(&self, payload: &str) -> String { b64_encode(&self.signing.sign(payload.as_bytes()).to_bytes()) }

    pub fn x25519_secret(&self) -> x25519_dalek::StaticSecret { x25519_dalek::StaticSecret::from(self.enc_private_raw) }
}
