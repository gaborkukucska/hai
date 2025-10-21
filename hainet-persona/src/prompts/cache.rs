// START OF FILE hainet-persona/src/prompts/cache.rs

//! Prompt caching system for performance optimization

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use crate::prompts::types::PromptCacheKey;

/// Cache entry with TTL support
#[derive(Debug, Clone)]
struct CacheEntry {
    value: String,
    created_at: Instant,
    last_accessed: Instant,
    access_count: u64,
}

/// Prompt cache with LRU eviction and TTL support
pub struct PromptCache {
    cache: Arc<RwLock<HashMap<PromptCacheKey, CacheEntry>>>,
    max_entries: usize,
    ttl: Duration,
}

impl PromptCache {
    /// Create new prompt cache with default settings
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            max_entries: 1000,
            ttl: Duration::from_secs(3600), // 1 hour default TTL
        }
    }

    /// Create new prompt cache with custom settings
    pub fn with_settings(max_entries: usize, ttl: Duration) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            max_entries,
            ttl,
        }
    }

    /// Get cached prompt if available and not expired
    pub fn get(&self, key: &PromptCacheKey) -> Option<String> {
        let mut cache = self.cache.write().unwrap();
        
        if let Some(entry) = cache.get_mut(key) {
            // Check if expired
            if entry.created_at.elapsed() > self.ttl {
                cache.remove(key);
                return None;
            }

            // Update access info
            entry.last_accessed = Instant::now();
            entry.access_count += 1;

            tracing::debug!(
                "Cache hit for key {:?} (access count: {})",
                key,
                entry.access_count
            );

            return Some(entry.value.clone());
        }

        tracing::debug!("Cache miss for key {:?}", key);
        None
    }

    /// Insert prompt into cache
    pub fn insert(&self, key: PromptCacheKey, value: String) {
        let mut cache = self.cache.write().unwrap();

        // Check if we need to evict entries
        if cache.len() >= self.max_entries {
            self.evict_lru(&mut cache);
        }

        let entry = CacheEntry {
            value,
            created_at: Instant::now(),
            last_accessed: Instant::now(),
            access_count: 0,
        };

        cache.insert(key, entry);
        tracing::debug!("Inserted prompt into cache (total entries: {})", cache.len());
    }

    /// Clear all cached prompts
    pub fn clear(&self) {
        let mut cache = self.cache.write().unwrap();
        let count = cache.len();
        cache.clear();
        tracing::info!("Cleared {} cached prompts", count);
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        let cache = self.cache.read().unwrap();
        
        let total_entries = cache.len();
        let total_accesses: u64 = cache.values().map(|e| e.access_count).sum();
        
        let mut expired_count = 0;
        let now = Instant::now();
        for entry in cache.values() {
            if now.duration_since(entry.created_at) > self.ttl {
                expired_count += 1;
            }
        }

        CacheStats {
            total_entries,
            expired_entries: expired_count,
            total_accesses,
            max_entries: self.max_entries,
            ttl_seconds: self.ttl.as_secs(),
        }
    }

    /// Remove expired entries
    pub fn cleanup_expired(&self) {
        let mut cache = self.cache.write().unwrap();
        let now = Instant::now();
        
        let initial_count = cache.len();
        cache.retain(|_, entry| now.duration_since(entry.created_at) <= self.ttl);
        let removed = initial_count - cache.len();
        
        if removed > 0 {
            tracing::info!("Cleaned up {} expired cache entries", removed);
        }
    }

    /// Evict least recently used entry
    fn evict_lru(&self, cache: &mut HashMap<PromptCacheKey, CacheEntry>) {
        if cache.is_empty() {
            return;
        }

        // Find LRU entry
        let lru_key = cache
            .iter()
            .min_by_key(|(_, entry)| entry.last_accessed)
            .map(|(key, _)| key.clone());

        if let Some(key) = lru_key {
            cache.remove(&key);
            tracing::debug!("Evicted LRU cache entry");
        }
    }

    /// Invalidate cache entries for a specific agent
    pub fn invalidate_agent(&self, agent_id: &crate::prompts::types::AgentId) {
        let mut cache = self.cache.write().unwrap();
        let initial_count = cache.len();
        
        cache.retain(|key, _| &key.agent_id != agent_id);
        
        let removed = initial_count - cache.len();
        if removed > 0 {
            tracing::info!("Invalidated {} cache entries for agent {:?}", removed, agent_id);
        }
    }

    /// Invalidate cache entries for a specific state
    pub fn invalidate_state(&self, state: crate::prompts::types::AgentState) {
        let mut cache = self.cache.write().unwrap();
        let initial_count = cache.len();
        
        cache.retain(|key, _| key.state != state);
        
        let removed = initial_count - cache.len();
        if removed > 0 {
            tracing::info!("Invalidated {} cache entries for state {:?}", removed, state);
        }
    }
}

impl Default for PromptCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub total_entries: usize,
    pub expired_entries: usize,
    pub total_accesses: u64,
    pub max_entries: usize,
    pub ttl_seconds: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompts::types::{AgentId, AgentState, AgentType, PromptContext};

    #[test]
    fn test_cache_basic_operations() {
        let cache = PromptCache::new();
        
        let agent_id = AgentId::new(AgentType::Admin, "test".to_string());
        let state = AgentState::Idle;
        let context = PromptContext::default();
        
        let key = PromptCacheKey::new(&agent_id, state, &context);
        let value = "test prompt".to_string();
        
        // Should be empty initially
        assert!(cache.get(&key).is_none());
        
        // Insert and retrieve
        cache.insert(key.clone(), value.clone());
        assert_eq!(cache.get(&key), Some(value));
        
        // Stats should reflect one entry
        let stats = cache.stats();
        assert_eq!(stats.total_entries, 1);
        assert_eq!(stats.total_accesses, 1);
    }

    #[test]
    fn test_cache_clear() {
        let cache = PromptCache::new();
        
        let agent_id = AgentId::new(AgentType::Admin, "test".to_string());
        let state = AgentState::Idle;
        let context = PromptContext::default();
        
        let key = PromptCacheKey::new(&agent_id, state, &context);
        cache.insert(key.clone(), "test".to_string());
        
        assert!(cache.get(&key).is_some());
        
        cache.clear();
        assert!(cache.get(&key).is_none());
        assert_eq!(cache.stats().total_entries, 0);
    }

    #[test]
    fn test_cache_lru_eviction() {
        let cache = PromptCache::with_settings(2, Duration::from_secs(3600));
        
        let agent_id = AgentId::new(AgentType::Admin, "test".to_string());
        let context = PromptContext::default();
        
        let key1 = PromptCacheKey::new(&agent_id, AgentState::Idle, &context);
        let key2 = PromptCacheKey::new(&agent_id, AgentState::Planning, &context);
        let key3 = PromptCacheKey::new(&agent_id, AgentState::Working, &context);
        
        cache.insert(key1.clone(), "prompt1".to_string());
        std::thread::sleep(Duration::from_millis(10));
        cache.insert(key2.clone(), "prompt2".to_string());
        
        // Access key1 to make it more recently used
        cache.get(&key1);
        
        // Insert key3, should evict key2 (LRU)
        cache.insert(key3.clone(), "prompt3".to_string());
        
        assert_eq!(cache.stats().total_entries, 2);
        assert!(cache.get(&key1).is_some());
        assert!(cache.get(&key2).is_none());
        assert!(cache.get(&key3).is_some());
    }

    #[test]
    fn test_cache_invalidation() {
        let cache = PromptCache::new();
        
        let agent_id = AgentId::new(AgentType::Admin, "test".to_string());
        let context = PromptContext::default();
        
        let key1 = PromptCacheKey::new(&agent_id, AgentState::Idle, &context);
        let key2 = PromptCacheKey::new(&agent_id, AgentState::Planning, &context);
        
        cache.insert(key1.clone(), "prompt1".to_string());
        cache.insert(key2.clone(), "prompt2".to_string());
        
        // Invalidate by state
        cache.invalidate_state(AgentState::Idle);
        
        assert!(cache.get(&key1).is_none());
        assert!(cache.get(&key2).is_some());
    }

    #[test]
    fn test_cache_ttl() {
        let cache = PromptCache::with_settings(100, Duration::from_millis(50));
        
        let agent_id = AgentId::new(AgentType::Admin, "test".to_string());
        let context = PromptContext::default();
        let key = PromptCacheKey::new(&agent_id, AgentState::Idle, &context);
        
        cache.insert(key.clone(), "prompt".to_string());
        assert!(cache.get(&key).is_some());
        
        // Wait for TTL to expire
        std::thread::sleep(Duration::from_millis(100));
        
        // Should be expired now
        assert!(cache.get(&key).is_none());
    }
}
