//! Simple LRU cache for web responses

use std::collections::HashMap;

pub struct ResponseCache {
    cache: HashMap<String, serde_json::Value>,
    max_size: usize,
    access_order: Vec<String>,
}

impl ResponseCache {
    pub fn new(max_size: usize) -> Self {
        Self {
            cache: HashMap::new(),
            max_size,
            access_order: Vec::new(),
        }
    }

    pub fn get(&self, key: &str) -> Option<serde_json::Value> {
        self.cache.get(key).cloned()
    }

    pub fn insert(&mut self, key: String, value: serde_json::Value) {
        // If key exists, remove it from access order
        if let Some(pos) = self.access_order.iter().position(|k| k == &key) {
            self.access_order.remove(pos);
        }

        // Add to end (most recent)
        self.access_order.push(key.clone());
        self.cache.insert(key, value);

        // Evict oldest if over capacity
        while self.cache.len() > self.max_size {
            if let Some(oldest) = self.access_order.first().cloned() {
                self.access_order.remove(0);
                self.cache.remove(&oldest);
            }
        }
    }

    pub fn clear(&mut self) {
        self.cache.clear();
        self.access_order.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_cache_basic() {
        let mut cache = ResponseCache::new(2);
        
        cache.insert("key1".to_string(), json!({"value": 1}));
        cache.insert("key2".to_string(), json!({"value": 2}));
        
        assert!(cache.get("key1").is_some());
        assert!(cache.get("key2").is_some());
        
        // This should evict key1
        cache.insert("key3".to_string(), json!({"value": 3}));
        
        assert!(cache.get("key1").is_none());
        assert!(cache.get("key2").is_some());
        assert!(cache.get("key3").is_some());
    }
}
