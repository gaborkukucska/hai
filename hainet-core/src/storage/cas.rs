//!<!-- # START OF FILE hainet-core/src/storage/cas.rs -->
//! Content-Addressed Storage (CAS)
//!
//! Provides BLAKE3-based content addressing with local storage backend.
//! Files are stored by their content hash, enabling deduplication and
//! efficient P2P synchronization.

use anyhow::{Context, Result};
use blake3;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Content hash using BLAKE3 (32 bytes)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContentHash([u8; 32]);

impl ContentHash {
    /// Create hash from bytes
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let hash = blake3::hash(bytes);
        Self(*hash.as_bytes())
    }

    /// Get hash as hex string
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    /// Parse hash from hex string
    pub fn from_hex(s: &str) -> Result<Self> {
        let bytes = hex::decode(s).context("Invalid hex string")?;
        if bytes.len() != 32 {
            anyhow::bail!("Hash must be 32 bytes, got {}", bytes.len());
        }
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&bytes);
        Ok(Self(hash))
    }

    /// Get raw bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

/// Metadata for stored content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentMetadata {
    pub hash: ContentHash,
    pub size: u64,
    pub stored_at: std::time::SystemTime,
    pub original_path: Option<PathBuf>,
}

/// Content-addressed storage with BLAKE3 hashing
#[derive(Clone)]
pub struct ContentAddressedStore {
    base_path: PathBuf,
    metadata: Arc<RwLock<std::collections::HashMap<ContentHash, ContentMetadata>>>,
}

impl ContentAddressedStore {
    /// Create new CAS with specified base path
    pub fn new(base_path: PathBuf) -> Result<Self> {
        fs::create_dir_all(&base_path)
            .context("Failed to create CAS directory")?;
        
        info!("Initialized content-addressed store at {:?}", base_path);
        
        Ok(Self {
            base_path,
            metadata: Arc::new(RwLock::new(std::collections::HashMap::new())),
        })
    }

    /// Store content and return its hash
    pub async fn put(&self, content: &[u8], original_path: Option<PathBuf>) -> Result<ContentHash> {
        let hash = ContentHash::from_bytes(content);
        let path = self.content_path(&hash);

        // Check if already stored
        if path.exists() {
            debug!("Content already exists: {}", hash);
            return Ok(hash);
        }

        // Create parent directory
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .context("Failed to create content directory")?;
        }

        // Write content
        let mut file = fs::File::create(&path)
            .context("Failed to create content file")?;
        file.write_all(content)
            .context("Failed to write content")?;

        // Store metadata
        let metadata = ContentMetadata {
            hash,
            size: content.len() as u64,
            stored_at: std::time::SystemTime::now(),
            original_path,
        };
        
        self.metadata.write().await.insert(hash, metadata);

        info!("Stored {} bytes with hash {}", content.len(), hash);
        Ok(hash)
    }

    /// Retrieve content by hash
    pub async fn get(&self, hash: &ContentHash) -> Result<Vec<u8>> {
        let path = self.content_path(hash);

        if !path.exists() {
            anyhow::bail!("Content not found: {}", hash);
        }

        let content = fs::read(&path)
            .context("Failed to read content")?;

        // Verify hash
        let actual_hash = ContentHash::from_bytes(&content);
        if &actual_hash != hash {
            warn!("Hash mismatch for {}: expected {}, got {}", 
                  path.display(), hash, actual_hash);
            anyhow::bail!("Content hash verification failed");
        }

        debug!("Retrieved {} bytes for hash {}", content.len(), hash);
        Ok(content)
    }

    /// Check if content exists
    pub fn has(&self, hash: &ContentHash) -> bool {
        self.content_path(hash).exists()
    }

    /// Get metadata for content
    pub async fn get_metadata(&self, hash: &ContentHash) -> Option<ContentMetadata> {
        self.metadata.read().await.get(hash).cloned()
    }

    /// Delete content
    pub async fn delete(&self, hash: &ContentHash) -> Result<()> {
        let path = self.content_path(hash);
        
        if path.exists() {
            fs::remove_file(&path)
                .context("Failed to delete content")?;
            self.metadata.write().await.remove(hash);
            info!("Deleted content: {}", hash);
        }
        
        Ok(())
    }

    /// List all stored content hashes
    pub async fn list_all(&self) -> Vec<ContentHash> {
        self.metadata.read().await.keys().copied().collect()
    }

    /// Get total stored bytes
    pub async fn total_size(&self) -> u64 {
        self.metadata.read().await.values()
            .map(|m| m.size)
            .sum()
    }

    /// Get content file path for hash
    fn content_path(&self, hash: &ContentHash) -> PathBuf {
        let hex = hash.to_hex();
        // Use first 2 chars for directory sharding
        let dir = &hex[0..2];
        self.base_path.join(dir).join(&hex)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_content_hash_creation() {
        let data = b"Hello, HAI-Net!";
        let hash = ContentHash::from_bytes(data);
        assert_eq!(hash.as_bytes().len(), 32);
    }

    #[test]
    fn test_content_hash_hex() {
        let data = b"test";
        let hash = ContentHash::from_bytes(data);
        let hex = hash.to_hex();
        assert_eq!(hex.len(), 64); // 32 bytes * 2 hex chars
        
        let parsed = ContentHash::from_hex(&hex).unwrap();
        assert_eq!(hash, parsed);
    }

    #[test]
    fn test_content_hash_invalid_hex() {
        assert!(ContentHash::from_hex("invalid").is_err());
        assert!(ContentHash::from_hex("aa").is_err()); // Too short
    }

    #[tokio::test]
    async fn test_cas_put_get() {
        let dir = tempdir().unwrap();
        let store = ContentAddressedStore::new(dir.path().to_path_buf()).unwrap();

        let data = b"Test content for CAS";
        let hash = store.put(data, None).await.unwrap();

        let retrieved = store.get(&hash).await.unwrap();
        assert_eq!(data, retrieved.as_slice());
    }

    #[tokio::test]
    async fn test_cas_duplicate_put() {
        let dir = tempdir().unwrap();
        let store = ContentAddressedStore::new(dir.path().to_path_buf()).unwrap();

        let data = b"Duplicate test";
        let hash1 = store.put(data, None).await.unwrap();
        let hash2 = store.put(data, None).await.unwrap();

        assert_eq!(hash1, hash2);
    }

    #[tokio::test]
    async fn test_cas_has() {
        let dir = tempdir().unwrap();
        let store = ContentAddressedStore::new(dir.path().to_path_buf()).unwrap();

        let data = b"Existence test";
        let hash = store.put(data, None).await.unwrap();

        assert!(store.has(&hash));
        
        let fake_hash = ContentHash::from_bytes(b"nonexistent");
        assert!(!store.has(&fake_hash));
    }

    #[tokio::test]
    async fn test_cas_delete() {
        let dir = tempdir().unwrap();
        let store = ContentAddressedStore::new(dir.path().to_path_buf()).unwrap();

        let data = b"Delete test";
        let hash = store.put(data, None).await.unwrap();
        assert!(store.has(&hash));

        store.delete(&hash).await.unwrap();
        assert!(!store.has(&hash));
    }

    #[tokio::test]
    async fn test_cas_metadata() {
        let dir = tempdir().unwrap();
        let store = ContentAddressedStore::new(dir.path().to_path_buf()).unwrap();

        let data = b"Metadata test";
        let original_path = Some(PathBuf::from("/test/file.txt"));
        let hash = store.put(data, original_path.clone()).await.unwrap();

        let metadata = store.get_metadata(&hash).await.unwrap();
        assert_eq!(metadata.hash, hash);
        assert_eq!(metadata.size, data.len() as u64);
        assert_eq!(metadata.original_path, original_path);
    }

    #[tokio::test]
    async fn test_cas_total_size() {
        let dir = tempdir().unwrap();
        let store = ContentAddressedStore::new(dir.path().to_path_buf()).unwrap();

        let data1 = b"First file";
        let data2 = b"Second file content";
        
        store.put(data1, None).await.unwrap();
        store.put(data2, None).await.unwrap();

        let total = store.total_size().await;
        assert_eq!(total, (data1.len() + data2.len()) as u64);
    }
}
