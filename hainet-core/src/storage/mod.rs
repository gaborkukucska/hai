//!<!-- # START OF FILE hainet-core/src/storage/mod.rs -->
//! Storage subsystem for HAI-Net Core
//!
//! Provides content-addressed storage (CAS) and peer-to-peer file synchronization
//! across devices in the local hub.
//!
//! ## Constitutional Compliance
//! - Article I (Privacy First): All storage is local-first, encrypted at rest
//! - Article III (Decentralization): No central storage authority
//! - Article IV (Community Focus): Voluntary resource sharing across hub

pub mod cas;
pub mod sync;

pub use cas::{ContentAddressedStore, ContentHash};
pub use sync::{P2PFileSync, SyncRequest, SyncResponse};

use anyhow::Result;
use std::path::PathBuf;

/// Storage manager coordinating CAS and P2P sync
pub struct StorageManager {
    store: ContentAddressedStore,
    sync: P2PFileSync,
}

impl StorageManager {
    /// Create new storage manager
    pub fn new(base_path: PathBuf) -> Result<Self> {
        let store = ContentAddressedStore::new(base_path.join("cas"))?;
        let sync = P2PFileSync::new(store.clone());
        
        Ok(Self { store, sync })
    }
    
    /// Get content-addressed store
    pub fn store(&self) -> &ContentAddressedStore {
        &self.store
    }
    
    /// Get P2P sync manager
    pub fn sync(&self) -> &P2PFileSync {
        &self.sync
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    
    #[test]
    fn test_storage_manager_creation() {
        let dir = tempdir().unwrap();
        let manager = StorageManager::new(dir.path().to_path_buf());
        assert!(manager.is_ok());
    }
}
