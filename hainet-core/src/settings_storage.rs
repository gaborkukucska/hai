//! # START OF FILE hainet-portal/src-tauri/src/settings_storage.rs
//! Settings storage module for HAI-Net Portal
//! Provides persistent storage for user preferences and device configurations

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use tracing::{debug, error, info, warn};
use std::time::{SystemTime, UNIX_EPOCH};

/// Device preference for audio/video devices
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevicePreference {
    pub device_type: String,  // 'microphone', 'speaker', 'camera'
    pub device_id: String,
    pub device_name: String,
    pub is_default: bool,
}

/// Model family preference for AI agent types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPreference {
    pub agent_type: String,  // 'Admin', 'PM', 'Worker'
    pub preferred_family: String,  // 'auto', 'llama3', 'gemma3', 'qwen', 'deepseek', 'phi'
    pub allow_fallback: bool,
}

/// Settings storage manager with SQLite backend
pub struct SettingsStorage {
    pool: SqlitePool,
}

impl SettingsStorage {
    /// Create new settings storage with database connection
    pub async fn new(db_path: &str) -> Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(db_path)
            .await?;
        
        let storage = Self { pool };
        storage.create_tables().await?;
        
        Ok(storage)
    }
    
    /// Create database tables if they don't exist
    async fn create_tables(&self) -> Result<()> {
        // Settings table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at INTEGER NOT NULL
            )
            "#
        )
        .execute(&self.pool)
        .await?;
        
        // Device preferences table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS device_preferences (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                device_type TEXT NOT NULL,
                device_id TEXT NOT NULL,
                device_name TEXT NOT NULL,
                is_default INTEGER DEFAULT 0,
                UNIQUE(device_type, device_id)
            )
            "#
        )
        .execute(&self.pool)
        .await?;
        
        // Create index for faster lookups
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_device_type ON device_preferences(device_type)"
        )
        .execute(&self.pool)
        .await?;
        
        // Model preferences table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS model_preferences (
                agent_type TEXT PRIMARY KEY,
                preferred_family TEXT NOT NULL,
                allow_fallback INTEGER DEFAULT 1,
                updated_at INTEGER NOT NULL
            )
            "#
        )
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    /// Get current Unix timestamp
    fn now() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }
    
    /// Save a single setting
    pub async fn save_setting(&self, key: &str, value: &str) -> Result<()> {
        let timestamp = Self::now();
        
        sqlx::query(
            r#"
            INSERT INTO settings (key, value, updated_at)
            VALUES (?, ?, ?)
            ON CONFLICT(key) DO UPDATE SET
                value = excluded.value,
                updated_at = excluded.updated_at
            "#
        )
        .bind(key)
        .bind(value)
        .bind(timestamp)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    /// Get a single setting
    pub async fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let result = sqlx::query_as::<_, (String,)>(
            "SELECT value FROM settings WHERE key = ?"
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await?;
        
        Ok(result.map(|r| r.0))
    }
    
    /// Load all settings as key-value pairs
    pub async fn load_all_settings(&self) -> Result<Vec<(String, String)>> {
        let results = sqlx::query_as::<_, (String, String)>(
            "SELECT key, value FROM settings ORDER BY key"
        )
        .fetch_all(&self.pool)
        .await?;
        
        Ok(results)
    }
    
    /// Save multiple settings at once (transactional)
    pub async fn save_settings_batch(&self, settings: Vec<(&str, &str)>) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let timestamp = Self::now();
        
        for (key, value) in settings {
            sqlx::query(
                r#"
                INSERT INTO settings (key, value, updated_at)
                VALUES (?, ?, ?)
                ON CONFLICT(key) DO UPDATE SET
                    value = excluded.value,
                    updated_at = excluded.updated_at
                "#
            )
            .bind(key)
            .bind(value)
            .bind(timestamp)
            .execute(&mut *tx)
            .await?;
        }
        
        tx.commit().await?;
        Ok(())
    }
    
    /// Save device preference
    pub async fn save_device_preference(&self, pref: &DevicePreference) -> Result<()> {
        // If setting as default, unset other defaults of same type first
        if pref.is_default {
            sqlx::query(
                "UPDATE device_preferences SET is_default = 0 WHERE device_type = ?"
            )
            .bind(&pref.device_type)
            .execute(&self.pool)
            .await?;
        }
        
        sqlx::query(
            r#"
            INSERT INTO device_preferences (device_type, device_id, device_name, is_default)
            VALUES (?, ?, ?, ?)
            ON CONFLICT(device_type, device_id) DO UPDATE SET
                device_name = excluded.device_name,
                is_default = excluded.is_default
            "#
        )
        .bind(&pref.device_type)
        .bind(&pref.device_id)
        .bind(&pref.device_name)
        .bind(pref.is_default as i32)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    /// Get device preferences by type
    pub async fn get_device_preferences(&self, device_type: &str) -> Result<Vec<DevicePreference>> {
        let results = sqlx::query_as::<_, (String, String, String, i32)>(
            "SELECT device_type, device_id, device_name, is_default 
             FROM device_preferences 
             WHERE device_type = ?
             ORDER BY is_default DESC, device_name ASC"
        )
        .bind(device_type)
        .fetch_all(&self.pool)
        .await?;
        
        Ok(results
            .into_iter()
            .map(|(device_type, device_id, device_name, is_default)| DevicePreference {
                device_type,
                device_id,
                device_name,
                is_default: is_default != 0,
            })
            .collect())
    }
    
    /// Get default device for a type
    pub async fn get_default_device(&self, device_type: &str) -> Result<Option<DevicePreference>> {
        let result = sqlx::query_as::<_, (String, String, String, i32)>(
            "SELECT device_type, device_id, device_name, is_default 
             FROM device_preferences 
             WHERE device_type = ? AND is_default = 1
             LIMIT 1"
        )
        .bind(device_type)
        .fetch_optional(&self.pool)
        .await?;
        
        Ok(result.map(|(device_type, device_id, device_name, is_default)| DevicePreference {
            device_type,
            device_id,
            device_name,
            is_default: is_default != 0,
        }))
    }
    
    /// Delete a device preference
    pub async fn delete_device_preference(&self, device_type: &str, device_id: &str) -> Result<()> {
        sqlx::query(
            "DELETE FROM device_preferences WHERE device_type = ? AND device_id = ?"
        )
        .bind(device_type)
        .bind(device_id)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    /// Clear all settings (for testing or reset)
    pub async fn clear_all(&self) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        
        sqlx::query("DELETE FROM settings")
            .execute(&mut *tx)
            .await?;
        
        sqlx::query("DELETE FROM device_preferences")
            .execute(&mut *tx)
            .await?;
        
        sqlx::query("DELETE FROM model_preferences")
            .execute(&mut *tx)
            .await?;
        
        tx.commit().await?;
        Ok(())
    }
    
    /// Save model preference for an agent type
    pub async fn save_model_preference(
        &self,
        agent_type: &str,
        family: &str,
        allow_fallback: bool,
    ) -> Result<()> {
        debug!("[DB] Saving model preference: {} -> {} (fallback: {})", agent_type, family, allow_fallback);
        let timestamp = Self::now();
        
        let result = sqlx::query(
            r#"
            INSERT INTO model_preferences (agent_type, preferred_family, allow_fallback, updated_at)
            VALUES (?, ?, ?, ?)
            ON CONFLICT(agent_type) DO UPDATE SET
                preferred_family = excluded.preferred_family,
                allow_fallback = excluded.allow_fallback,
                updated_at = excluded.updated_at
            "#
        )
        .bind(agent_type)
        .bind(family)
        .bind(allow_fallback as i32)
        .bind(timestamp)
        .execute(&self.pool)
        .await?;
        
        debug!("[DB] Save result: rows_affected={}", result.rows_affected());
        Ok(())
    }
    
    /// Get model preference for a specific agent type
    pub async fn get_model_preference(&self, agent_type: &str) -> Result<Option<ModelPreference>> {
        let result = sqlx::query_as::<_, (String, String, i32)>(
            "SELECT agent_type, preferred_family, allow_fallback 
             FROM model_preferences 
             WHERE agent_type = ?"
        )
        .bind(agent_type)
        .fetch_optional(&self.pool)
        .await?;
        
        Ok(result.map(|(agent_type, preferred_family, allow_fallback)| ModelPreference {
            agent_type,
            preferred_family,
            allow_fallback: allow_fallback != 0,
        }))
    }
    
    /// Get all model preferences
    pub async fn get_all_model_preferences(&self) -> Result<Vec<ModelPreference>> {
        debug!("[DB] Fetching all model preferences...");
        let results = sqlx::query_as::<_, (String, String, i32)>(
            "SELECT agent_type, preferred_family, allow_fallback 
             FROM model_preferences 
             ORDER BY agent_type"
        )
        .fetch_all(&self.pool)
        .await?;
        
        debug!("[DB] Found {} model preferences in database", results.len());
        for (agent_type, family, fallback) in &results {
            debug!("[DB]   - {}: {} (fallback: {})", agent_type, family, fallback);
        }
        
        Ok(results
            .into_iter()
            .map(|(agent_type, preferred_family, allow_fallback)| ModelPreference {
                agent_type,
                preferred_family,
                allow_fallback: allow_fallback != 0,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    async fn create_test_storage() -> SettingsStorage {
        SettingsStorage::new("sqlite::memory:")
            .await
            .expect("Failed to create test storage")
    }
    
    #[tokio::test]
    async fn test_save_and_get_setting() {
        let storage = create_test_storage().await;
        
        storage.save_setting("theme", "dark").await.unwrap();
        let value = storage.get_setting("theme").await.unwrap();
        
        assert_eq!(value, Some("dark".to_string()));
    }
    
    #[tokio::test]
    async fn test_save_settings_batch() {
        let storage = create_test_storage().await;
        
        storage.save_settings_batch(vec![
            ("privacy.pii_detection", "true"),
            ("privacy.bias_detection", "true"),
            ("appearance.theme", "dark"),
        ]).await.unwrap();
        
        let settings = storage.load_all_settings().await.unwrap();
        assert_eq!(settings.len(), 3);
    }
    
    #[tokio::test]
    async fn test_device_preferences() {
        let storage = create_test_storage().await;
        
        let mic_pref = DevicePreference {
            device_type: "microphone".to_string(),
            device_id: "mic-001".to_string(),
            device_name: "Built-in Microphone".to_string(),
            is_default: true,
        };
        
        storage.save_device_preference(&mic_pref).await.unwrap();
        
        let prefs = storage.get_device_preferences("microphone").await.unwrap();
        assert_eq!(prefs.len(), 1);
        assert_eq!(prefs[0].device_id, "mic-001");
        assert!(prefs[0].is_default);
    }
    
    #[tokio::test]
    async fn test_default_device_switch() {
        let storage = create_test_storage().await;
        
        // Add first mic as default
        let mic1 = DevicePreference {
            device_type: "microphone".to_string(),
            device_id: "mic-001".to_string(),
            device_name: "Mic 1".to_string(),
            is_default: true,
        };
        storage.save_device_preference(&mic1).await.unwrap();
        
        // Add second mic as default (should unset first)
        let mic2 = DevicePreference {
            device_type: "microphone".to_string(),
            device_id: "mic-002".to_string(),
            device_name: "Mic 2".to_string(),
            is_default: true,
        };
        storage.save_device_preference(&mic2).await.unwrap();
        
        // Check only mic2 is default
        let default = storage.get_default_device("microphone").await.unwrap();
        assert!(default.is_some());
        assert_eq!(default.unwrap().device_id, "mic-002");
    }
}
