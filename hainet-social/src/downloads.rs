// START OF FILE hainet-social/src/downloads.rs
//! Media Download and Chunk Management
//! 
//! Ports `ActiveDownload` and chunk assembly logic from gChat.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::SocialResult;

/// Represents a single chunk of media data
#[derive(Debug, Clone)]
pub struct MediaChunk {
    pub file_id: String,
    pub chunk_index: u32,
    pub total_chunks: u32,
    pub data: Vec<u8>,
}

/// Manages an active file download via the mesh
#[derive(Debug)]
pub struct ActiveDownload {
    pub file_id: String,
    pub total_chunks: u32,
    pub received_chunks: HashSet<u32>,
    pub data_buffer: HashMap<u32, Vec<u8>>,
    pub target_path: Option<String>,
    pub is_complete: bool,
}

impl ActiveDownload {
    pub fn new(file_id: String, total_chunks: u32, target_path: Option<String>) -> Self {
        Self {
            file_id,
            total_chunks,
            received_chunks: HashSet::new(),
            data_buffer: HashMap::new(),
            target_path,
            is_complete: false,
        }
    }

    /// Add a chunk to the active download
    pub fn add_chunk(&mut self, chunk: MediaChunk) -> bool {
        if self.is_complete {
            return false;
        }

        if chunk.chunk_index >= self.total_chunks {
            warn!("Received out of bounds chunk index: {}", chunk.chunk_index);
            return false;
        }

        if self.received_chunks.insert(chunk.chunk_index) {
            self.data_buffer.insert(chunk.chunk_index, chunk.data);
            
            if self.received_chunks.len() as u32 == self.total_chunks {
                self.is_complete = true;
                return true; // Download completed with this chunk
            }
        }
        
        false // Chunk added but not yet complete
    }

    /// Assemble the file into a contiguous byte vector
    pub fn assemble(&self) -> Option<Vec<u8>> {
        if !self.is_complete {
            return None;
        }

        let mut final_data = Vec::new();
        for i in 0..self.total_chunks {
            if let Some(chunk_data) = self.data_buffer.get(&i) {
                final_data.extend_from_slice(chunk_data);
            } else {
                warn!("Missing chunk {} during assembly despite being marked complete", i);
                return None;
            }
        }
        
        Some(final_data)
    }
}

/// Download Manager for coordinating multiple media transfers
#[derive(Clone)]
pub struct DownloadManager {
    downloads: Arc<RwLock<HashMap<String, ActiveDownload>>>,
}

impl DownloadManager {
    pub fn new() -> Self {
        Self {
            downloads: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start a new download
    pub async fn start_download(&self, file_id: String, total_chunks: u32, target_path: Option<String>) {
        let mut downloads = self.downloads.write().await;
        if !downloads.contains_key(&file_id) {
            info!("Starting download for file: {} ({} chunks)", file_id, total_chunks);
            downloads.insert(file_id.clone(), ActiveDownload::new(file_id, total_chunks, target_path));
        }
    }

    /// Process an incoming chunk
    pub async fn process_chunk(&self, chunk: MediaChunk) -> SocialResult<bool> {
        let mut downloads = self.downloads.write().await;
        
        if let Some(download) = downloads.get_mut(&chunk.file_id) {
            let file_id = chunk.file_id.clone();
            let is_complete = download.add_chunk(chunk);
            
            if is_complete {
                info!("Download complete for file: {}", file_id);
                // In a real implementation, we would save to disk here
                return Ok(true);
            }
            return Ok(false);
        }
        
        debug!("Received chunk for unknown download: {}", chunk.file_id);
        Ok(false)
    }
    
    /// Check progress of a download
    pub async fn get_progress(&self, file_id: &str) -> Option<f32> {
        let downloads = self.downloads.read().await;
        downloads.get(file_id).map(|d| {
            if d.total_chunks == 0 {
                1.0
            } else {
                d.received_chunks.len() as f32 / d.total_chunks as f32
            }
        })
    }
}
