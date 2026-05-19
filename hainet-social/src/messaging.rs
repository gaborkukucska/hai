// START OF FILE hainet-social/src/messaging.rs
//! Messaging System
//! 
//! Ports Direct Messaging (DMs) and group chat logic from gChat,
//! integrating with `crypto.rs` to ensure End-to-End (E2E) encryption.

use serde::{Serialize, Deserialize};

/// Represents an encrypted payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedPayload {
    pub nonce: String,      // Base64 encoded nonce
    pub ciphertext: String, // Base64 encoded encrypted data
}

/// Represents a Direct Message or Group Chat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub sender_id: String,
    pub target_id: String, // User ID for DMs, Group ID for group chats
    pub is_group_message: bool,
    pub encrypted_content: EncryptedPayload,
    pub created_at: u64,
}

impl Message {
    /// Create a new pre-encrypted message
    pub fn new(
        sender_id: String,
        target_id: String,
        is_group_message: bool,
        encrypted_content: EncryptedPayload
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
            
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            sender_id,
            target_id,
            is_group_message,
            encrypted_content,
            created_at: now,
        }
    }
}

/// Defines operations for message storage and retrieval
pub trait MessageStorage: Send + Sync {
    /// Store a new incoming or outgoing message
    fn store_message(&self, message: &Message) -> crate::SocialResult<()>;
    
    /// Get Direct Messages between the local user and a specific peer
    fn get_direct_messages(&self, local_user_id: &str, peer_id: &str, limit: usize, offset: usize) -> crate::SocialResult<Vec<Message>>;
    
    /// Get messages for a specific group
    fn get_group_messages(&self, group_id: &str, limit: usize, offset: usize) -> crate::SocialResult<Vec<Message>>;
}

/// Helper functions for encryption integration
pub mod crypto_helpers {
    use super::*;
    use crate::crypto;
    use crate::SocialResult;

    use base64::prelude::*;

    use x25519_dalek::{StaticSecret, PublicKey};

    /// Encrypt a plaintext message for a specific recipient
    pub fn encrypt_for_dm(
        plaintext: &str,
        sender_secret: &StaticSecret,
        recipient_public: &PublicKey
    ) -> SocialResult<EncryptedPayload> {
        // Use crypto module to encrypt
        let (nonce, ciphertext) = crypto::encrypt_for_recipient(plaintext.as_bytes(), sender_secret, recipient_public)
            .map_err(|e| crate::SocialError::CryptoError(e.to_string()))?;
            
        Ok(EncryptedPayload {
            nonce: BASE64_STANDARD.encode(nonce),
            ciphertext: BASE64_STANDARD.encode(ciphertext),
        })
    }

    /// Decrypt a received DM
    pub fn decrypt_dm(
        payload: &EncryptedPayload,
        recipient_secret: &StaticSecret,
        sender_public: &PublicKey
    ) -> SocialResult<String> {
        let nonce_vec = BASE64_STANDARD.decode(&payload.nonce)
            .map_err(|e| crate::SocialError::CryptoError(format!("Invalid nonce base64: {}", e)))?;
            
        let nonce: [u8; 12] = nonce_vec.try_into()
            .map_err(|_| crate::SocialError::CryptoError("Nonce must be exactly 12 bytes".to_string()))?;
            
        let ciphertext = BASE64_STANDARD.decode(&payload.ciphertext)
            .map_err(|e| crate::SocialError::CryptoError(format!("Invalid ciphertext base64: {}", e)))?;
            
        // Use crypto module to decrypt
        let plaintext_bytes = crypto::decrypt_from_sender(&ciphertext, &nonce, recipient_secret, sender_public)
            .map_err(|e| crate::SocialError::CryptoError(e.to_string()))?;
            
        String::from_utf8(plaintext_bytes)
            .map_err(|e| crate::SocialError::CryptoError(format!("Invalid UTF-8 in decrypted message: {}", e)))
    }
}
