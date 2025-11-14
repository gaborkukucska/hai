//! User Settings Storage
//!
//! Stores user preferences for model selection in hainet-persona's database.
//! This allows the agent system to access user preferences without depending
//! on the Portal's database.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::{SqlitePool, Row};
use std::sync::Arc;
use tokio::sync::RwLock;

/// User model preference for a specific agent type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPreference {
    pub agent_type: String,
    pub model_family: String,
}

/// User settings manager
pub struct UserSettingsManager {
    pool: SqlitePool,
}

impl UserSettingsManager {
    /// Create a new settings manager with the given database connection
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = SqlitePool::connect(database_url).await?;
        
        // Create settings table if it doesn't exist
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS user_model_preferences (
                agent_type TEXT PRIMARY KEY,
                model_family TEXT NOT NULL,
                updated_at INTEGER NOT NULL
            )
            "#
        )
        .execute(&pool)
        .await?;
        
        Ok(Self { pool })
    }
    
    /// Set model preference for an agent type
    pub async fn set_model_preference(&self, agent_type: &str, model_family: &str) -> Result<()> {
        let updated_at = chrono::Utc::now().timestamp();
        
        sqlx::query(
            r#"
            INSERT INTO user_model_preferences (agent_type, model_family, updated_at)
            VALUES (?, ?, ?)
            ON CONFLICT(agent_type) DO UPDATE SET
                model_family = excluded.model_family,
                updated_at = excluded.updated_at
            "#
        )
        .bind(agent_type)
        .bind(model_family)
        .bind(updated_at)
        .execute(&self.pool)
        .await?;
        
        tracing::info!("Set model preference for {} to {}", agent_type, model_family);
        
        Ok(())
    }
    
    /// Get model preference for an agent type
    pub async fn get_model_preference(&self, agent_type: &str) -> Result<Option<String>> {
        let result = sqlx::query(
            r#"
            SELECT model_family FROM user_model_preferences
            WHERE agent_type = ?
            "#
        )
        .bind(agent_type)
        .fetch_optional(&self.pool)
        .await?;
        
        Ok(result.map(|row| row.get::<String, _>("model_family")))
    }
    
    /// Get all model preferences
    pub async fn get_all_preferences(&self) -> Result<Vec<ModelPreference>> {
        let rows = sqlx::query(
            r#"
            SELECT agent_type, model_family FROM user_model_preferences
            ORDER BY agent_type
            "#
        )
        .fetch_all(&self.pool)
        .await?;
        
        let preferences = rows.into_iter()
            .map(|row| ModelPreference {
                agent_type: row.get("agent_type"),
                model_family: row.get("model_family"),
            })
            .collect();
        
        Ok(preferences)
    }
    
    /// Clear all preferences
    pub async fn clear_all_preferences(&self) -> Result<()> {
        sqlx::query("DELETE FROM user_model_preferences")
            .execute(&self.pool)
            .await?;
        
        tracing::info!("Cleared all model preferences");
        
        Ok(())
    }
}

/// Shared user settings manager
pub type SharedUserSettings = Arc<RwLock<UserSettingsManager>>;
