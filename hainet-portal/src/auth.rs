use anyhow::Result;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use aes_gcm::{
    aead::{Aead, KeyInit, generic_array::GenericArray},
    Aes256Gcm, Nonce,
};
use jsonwebtoken::{encode, Header, EncodingKey};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
}

pub fn get_hainet_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".hainet")
}

pub fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2.hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("Hashing failed: {}", e))?;
    Ok(password_hash.to_string())
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    let parsed_hash = match PasswordHash::new(hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    Argon2::default().verify_password(password.as_bytes(), &parsed_hash).is_ok()
}

pub fn generate_jwt(secret: &str) -> Result<String> {
    let expiration = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::hours(24))
        .expect("valid timestamp")
        .timestamp() as usize;

    let claims = Claims {
        sub: "hainet_user".to_owned(),
        exp: expiration,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?;
    Ok(token)
}

pub fn encrypt_seed(seed_phrase: &str, passphrase: &str) -> Result<Vec<u8>> {
    // Derive a 32-byte key from the passphrase (simplistic key derivation for demo, usually use argon2 or pbkdf2)
    // We'll use argon2 to derive a fixed 32-byte key
    let salt = b"hainet_static_salt_for_encryption"; // Static salt since we hash the pass separately
    let mut key = [0u8; 32];
    
    argon2::Argon2::default().hash_password_into(
        passphrase.as_bytes(),
        salt,
        &mut key
    ).map_err(|e| anyhow::anyhow!("Key derivation failed: {}", e))?;

    let cipher = Aes256Gcm::new(GenericArray::from_slice(&key));
    
    // In a real scenario, use a random nonce and prepend it to the ciphertext
    let nonce = Nonce::from_slice(b"unique nonce"); // 12-bytes
    let ciphertext = cipher.encrypt(nonce, seed_phrase.as_bytes())
        .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))?;
        
    Ok(ciphertext)
}
