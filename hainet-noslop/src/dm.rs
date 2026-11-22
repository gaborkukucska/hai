//! START OF FILE hainet-noslop/src/dm.rs
//!
//! NoSlop direct-message crypto (`CryptoService.encryptDMV2` / `decryptDMV2` / `decryptDM`).
//!
//! v2: `k_dir = HKDF-SHA256(ikm = X25519(myPriv, theirPub), salt = SHA-256(sorted(senderEd, recipientEd)),
//! info = "noslop-dm-v2|<senderEd>-><recipientEd>")`, ChaCha20-Poly1305 with a 12-byte IV and AAD
//! `encodeForSigning(domain, senderEd, recipientEd, msgId, groupId ?: "", timestamp)`.
//! v1 (receive only): key = SHA3-256(shared secret), no AAD.

use crate::{canon::efs, keys};
use chacha20poly1305::{aead::{Aead, KeyInit, Payload}, ChaCha20Poly1305, Nonce};
use hkdf::Hkdf;
use rand::RngCore;
use sha2::{Digest, Sha256};
use sha3::Sha3_256;
use x25519_dalek::{PublicKey, StaticSecret};

pub const DM_AAD_MESSAGE: &str = "noslop-dm-v2";
pub const DM_AAD_ACK: &str = "noslop-dm-ack-v3";

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum DmError {
    #[error("bad key: {0}")]
    Key(#[from] keys::KeyError),
    #[error("key agreement produced an all-zero secret")]
    ZeroSecret,
    #[error("decryption failed")]
    Decrypt,
    #[error("bad nonce")]
    Nonce,
}

fn shared_secret(my_priv: &StaticSecret, their_enc_pub_b64: &str) -> Result<[u8; 32], DmError> {
    let their = PublicKey::from(keys::x25519_public_raw(their_enc_pub_b64)?);
    let s = my_priv.diffie_hellman(&their).to_bytes();
    // Bouncy Castle's X25519Agreement refuses an all-zero result (small-order public key).
    if s.iter().all(|b| *b == 0) { return Err(DmError::ZeroSecret); }
    Ok(s)
}

/// `CryptoService.deriveDirectionalKeyV2`.
pub fn directional_key_v2(shared: &[u8; 32], sender_ed: &str, recipient_ed: &str) -> [u8; 32] {
    let (a, b) = if sender_ed <= recipient_ed { (sender_ed, recipient_ed) } else { (recipient_ed, sender_ed) };
    let mut h = Sha256::new();
    h.update(a.as_bytes());
    h.update(b.as_bytes());
    let salt = h.finalize();
    let info = format!("noslop-dm-v2|{}->{}", sender_ed, recipient_ed);
    let hk = Hkdf::<Sha256>::new(Some(&salt), shared);
    let mut out = [0u8; 32];
    hk.expand(info.as_bytes(), &mut out).expect("32 bytes is a valid HKDF output length");
    out
}

/// `CryptoService.canonicalDmAadV2`.
pub fn aad_v2(domain: &str, sender_ed: &str, recipient_ed: &str, msg_id: &str, group_id: Option<&str>, timestamp: i64) -> String {
    efs(&[domain, sender_ed, recipient_ed, msg_id, group_id.unwrap_or(""), &timestamp.to_string()])
}

/// Encrypts with an explicit IV (tests and deterministic vectors). Returns (ciphertext_b64, iv_b64).
#[allow(clippy::too_many_arguments)]
pub fn encrypt_v2_with_iv(
    plaintext: &str, their_enc_pub_b64: &str, my_priv: &StaticSecret, sender_ed: &str, recipient_ed: &str,
    msg_id: &str, group_id: Option<&str>, timestamp: i64, domain: &str, iv: [u8; 12],
) -> Result<(String, String), DmError> {
    let shared = shared_secret(my_priv, their_enc_pub_b64)?;
    let key = directional_key_v2(&shared, sender_ed, recipient_ed);
    let aad = aad_v2(domain, sender_ed, recipient_ed, msg_id, group_id, timestamp);
    let cipher = ChaCha20Poly1305::new((&key).into());
    let ct = cipher
        .encrypt(Nonce::from_slice(&iv), Payload { msg: plaintext.as_bytes(), aad: aad.as_bytes() })
        .map_err(|_| DmError::Decrypt)?;
    Ok((keys::b64_encode(&ct), keys::b64_encode(&iv)))
}

/// `CryptoService.encryptDMV2` (random IV).
#[allow(clippy::too_many_arguments)]
pub fn encrypt_v2(
    plaintext: &str, their_enc_pub_b64: &str, my_priv: &StaticSecret, sender_ed: &str, recipient_ed: &str,
    msg_id: &str, group_id: Option<&str>, timestamp: i64, domain: &str,
) -> Result<(String, String), DmError> {
    let mut iv = [0u8; 12];
    rand::rngs::OsRng.fill_bytes(&mut iv);
    encrypt_v2_with_iv(plaintext, their_enc_pub_b64, my_priv, sender_ed, recipient_ed, msg_id, group_id, timestamp, domain, iv)
}

/// `CryptoService.decryptDMV2`.
#[allow(clippy::too_many_arguments)]
pub fn decrypt_v2(
    ciphertext_b64: &str, nonce_b64: &str, their_enc_pub_b64: &str, my_priv: &StaticSecret, sender_ed: &str,
    recipient_ed: &str, msg_id: &str, group_id: Option<&str>, timestamp: i64, domain: &str,
) -> Result<String, DmError> {
    let shared = shared_secret(my_priv, their_enc_pub_b64)?;
    let key = directional_key_v2(&shared, sender_ed, recipient_ed);
    let iv = keys::b64_decode(nonce_b64)?;
    if iv.len() != 12 { return Err(DmError::Nonce); }
    let aad = aad_v2(domain, sender_ed, recipient_ed, msg_id, group_id, timestamp);
    let cipher = ChaCha20Poly1305::new((&key).into());
    let pt = cipher
        .decrypt(Nonce::from_slice(&iv), Payload { msg: &keys::b64_decode(ciphertext_b64)?, aad: aad.as_bytes() })
        .map_err(|_| DmError::Decrypt)?;
    String::from_utf8(pt).map_err(|_| DmError::Decrypt)
}

/// `CryptoService.decryptDM` (legacy v1, receive only).
pub fn decrypt_v1(ciphertext_b64: &str, nonce_b64: &str, their_enc_pub_b64: &str, my_priv: &StaticSecret) -> Result<String, DmError> {
    let shared = shared_secret(my_priv, their_enc_pub_b64)?;
    let key: [u8; 32] = Sha3_256::digest(shared).into();
    let iv = keys::b64_decode(nonce_b64)?;
    if iv.len() != 12 { return Err(DmError::Nonce); }
    let cipher = ChaCha20Poly1305::new((&key).into());
    let pt = cipher.decrypt(Nonce::from_slice(&iv), keys::b64_decode(ciphertext_b64)?.as_slice()).map_err(|_| DmError::Decrypt)?;
    String::from_utf8(pt).map_err(|_| DmError::Decrypt)
}
