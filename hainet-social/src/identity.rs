//! # START OF FILE hainet-social/src/identity.rs
//! Identity System — Ported from gChat's cryptoService.ts
//!
//! Handle.Tripcode deterministic identities using Ed25519 + SHA3-256.
//! Example: "Alice.x7z9ab" where "x7z9ab" is derived from the public key.

use ed25519_dalek::{SigningKey, VerifyingKey, Signer, Verifier, Signature};
use sha3::{Sha3_256, Digest};
use rand::rngs::OsRng;

use crate::SocialResult;

/// A user's cryptographic identity (signing + encryption keypairs)
#[derive(Clone)]
pub struct UserIdentity {
    /// Ed25519 signing key (for packet signatures)
    pub signing_key: SigningKey,
    /// Display name chosen by user
    pub display_name: String,
    /// Deterministic tripcode derived from public key
    pub tripcode: String,
}

impl UserIdentity {
    /// Generate a new random identity
    pub fn generate(display_name: &str) -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        let tripcode = generate_tripcode(&signing_key.verifying_key());

        Self {
            signing_key,
            display_name: display_name.to_string(),
            tripcode,
        }
    }

    /// Restore identity from a seed (deterministic — same seed = same identity)
    pub fn from_seed(display_name: &str, seed: &[u8; 32]) -> Self {
        let signing_key = SigningKey::from_bytes(seed);
        let tripcode = generate_tripcode(&signing_key.verifying_key());

        Self {
            signing_key,
            display_name: display_name.to_string(),
            tripcode,
        }
    }

    /// Get the full Handle.Tripcode display string (e.g., "Alice.x7z9ab")
    pub fn handle(&self) -> String {
        format!("{}.{}", self.display_name, self.tripcode)
    }

    /// Get the public verifying key
    pub fn public_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    /// Get the public key as Base64 string (for network transmission)
    pub fn public_key_base64(&self) -> String {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        STANDARD.encode(self.public_key().as_bytes())
    }

    /// Sign a message with this identity
    pub fn sign(&self, message: &[u8]) -> Signature {
        self.signing_key.sign(message)
    }
}

/// Generate a tripcode from a public key
///
/// Algorithm (from gChat's generateTripcode):
/// 1. Take Ed25519 public key bytes
/// 2. SHA3-256 hash
/// 3. Base32 encode
/// 4. Take first 6 characters, lowercase
pub fn generate_tripcode(public_key: &VerifyingKey) -> String {
    let mut hasher = Sha3_256::new();
    hasher.update(public_key.as_bytes());
    let hash = hasher.finalize();

    // Base32 encode (RFC 4648)
    let encoded = base32_encode(&hash);
    encoded[..6].to_lowercase()
}

/// Verify a signature against a public key
pub fn verify_signature(
    public_key: &VerifyingKey,
    message: &[u8],
    signature: &Signature,
) -> SocialResult<()> {
    public_key
        .verify(message, signature)
        .map_err(|e| crate::SocialError::CryptoError(format!("Signature verification failed: {}", e)))
}

/// Simple Base32 encoding (RFC 4648, no padding)
fn base32_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz234567";
    let mut result = String::new();
    let mut buffer: u64 = 0;
    let mut bits_left: u32 = 0;

    for &byte in data {
        buffer = (buffer << 8) | byte as u64;
        bits_left += 8;

        while bits_left >= 5 {
            bits_left -= 5;
            let index = ((buffer >> bits_left) & 0x1F) as usize;
            result.push(ALPHABET[index] as char);
        }
    }

    if bits_left > 0 {
        let index = ((buffer << (5 - bits_left)) & 0x1F) as usize;
        result.push(ALPHABET[index] as char);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_generation() {
        let identity = UserIdentity::generate("Alice");
        assert_eq!(identity.display_name, "Alice");
        assert_eq!(identity.tripcode.len(), 6);
        assert!(identity.handle().starts_with("Alice."));
    }

    #[test]
    fn test_deterministic_from_seed() {
        let seed = [42u8; 32];
        let id1 = UserIdentity::from_seed("Bob", &seed);
        let id2 = UserIdentity::from_seed("Bob", &seed);

        assert_eq!(id1.tripcode, id2.tripcode);
        assert_eq!(id1.public_key(), id2.public_key());
    }

    #[test]
    fn test_sign_and_verify() {
        let identity = UserIdentity::generate("Charlie");
        let message = b"Hello, HAI-Net!";
        let signature = identity.sign(message);

        assert!(verify_signature(&identity.public_key(), message, &signature).is_ok());
    }

    #[test]
    fn test_tripcode_is_lowercase_alphanumeric() {
        let identity = UserIdentity::generate("Test");
        for c in identity.tripcode.chars() {
            assert!(c.is_ascii_lowercase() || c.is_ascii_digit());
        }
    }
}
