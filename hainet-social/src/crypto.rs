//! # START OF FILE hainet-social/src/crypto.rs
//! E2E Encryption — Ported from gChat's cryptoService.ts
//!
//! Uses X25519 key exchange + ChaCha20-Poly1305 for message encryption.
//! This replaces gChat's NaCl box (which used X25519 + XSalsa20-Poly1305).

use chacha20poly1305::{
    aead::{Aead, KeyInit, OsRng},
    ChaCha20Poly1305, Nonce,
};
use x25519_dalek::{PublicKey, StaticSecret};
use sha3::{Sha3_256, Digest};
use rand::RngCore;

use crate::{SocialError, SocialResult};

/// Encryption keypair for DMs and group messages
#[derive(Clone)]
pub struct EncryptionKeys {
    pub secret: StaticSecret,
    pub public: PublicKey,
}

impl EncryptionKeys {
    /// Generate a new random encryption keypair
    pub fn generate() -> Self {
        let secret = StaticSecret::random_from_rng(OsRng);
        let public = PublicKey::from(&secret);
        Self { secret, public }
    }

    /// Restore from a 32-byte secret key
    pub fn from_secret(secret_bytes: [u8; 32]) -> Self {
        let secret = StaticSecret::from(secret_bytes);
        let public = PublicKey::from(&secret);
        Self { secret, public }
    }

    /// Get public key as Base64 for network transmission
    pub fn public_key_base64(&self) -> String {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        STANDARD.encode(self.public.as_bytes())
    }
}

/// Encrypt a message for a specific recipient
///
/// Uses X25519 shared secret + ChaCha20-Poly1305 AEAD
pub fn encrypt_for_recipient(
    plaintext: &[u8],
    sender_secret: &StaticSecret,
    recipient_public: &PublicKey,
) -> SocialResult<(Vec<u8>, [u8; 12])> {
    // Derive shared secret via X25519 Diffie-Hellman
    let shared_secret = sender_secret.diffie_hellman(recipient_public);

    // Derive symmetric key from shared secret using SHA3-256
    let mut hasher = Sha3_256::new();
    hasher.update(shared_secret.as_bytes());
    let symmetric_key = hasher.finalize();

    // Generate random nonce
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Encrypt with ChaCha20-Poly1305
    let cipher = ChaCha20Poly1305::new_from_slice(&symmetric_key[..32])
        .map_err(|e| SocialError::CryptoError(format!("Failed to create cipher: {}", e)))?;

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| SocialError::CryptoError(format!("Encryption failed: {}", e)))?;

    Ok((ciphertext, nonce_bytes))
}

/// Decrypt a message from a sender
pub fn decrypt_from_sender(
    ciphertext: &[u8],
    nonce_bytes: &[u8; 12],
    recipient_secret: &StaticSecret,
    sender_public: &PublicKey,
) -> SocialResult<Vec<u8>> {
    // Derive shared secret (same as sender's due to DH symmetry)
    let shared_secret = recipient_secret.diffie_hellman(sender_public);

    let mut hasher = Sha3_256::new();
    hasher.update(shared_secret.as_bytes());
    let symmetric_key = hasher.finalize();

    let nonce = Nonce::from_slice(nonce_bytes);

    let cipher = ChaCha20Poly1305::new_from_slice(&symmetric_key[..32])
        .map_err(|e| SocialError::CryptoError(format!("Failed to create cipher: {}", e)))?;

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| SocialError::CryptoError(format!("Decryption failed: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let alice = EncryptionKeys::generate();
        let bob = EncryptionKeys::generate();

        let plaintext = b"Hello from HAI-Net!";
        let (ciphertext, nonce) =
            encrypt_for_recipient(plaintext, &alice.secret, &bob.public).unwrap();

        let decrypted =
            decrypt_from_sender(&ciphertext, &nonce, &bob.secret, &alice.public).unwrap();

        assert_eq!(&decrypted, plaintext);
    }

    #[test]
    fn test_wrong_key_fails_decryption() {
        let alice = EncryptionKeys::generate();
        let bob = EncryptionKeys::generate();
        let eve = EncryptionKeys::generate();

        let plaintext = b"Secret message";
        let (ciphertext, nonce) =
            encrypt_for_recipient(plaintext, &alice.secret, &bob.public).unwrap();

        // Eve tries to decrypt with her key — should fail
        let result = decrypt_from_sender(&ciphertext, &nonce, &eve.secret, &alice.public);
        assert!(result.is_err());
    }
}
