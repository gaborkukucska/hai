use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePool, FromRow};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ProfileEntry {
    pub key: String,
    pub value: serde_json::Value,
    pub updated_at: i64,
}

pub struct UserProfile {
    pool: SqlitePool,
}

impl UserProfile {
    pub async fn new(db_url: &str) -> Result<Self> {
        let pool = SqlitePool::connect(db_url).await
            .context("Failed to connect to user profile database")?;
        
        // Create table if not exists
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS user_profile (
                key TEXT PRIMARY KEY,
                value JSON NOT NULL,
                updated_at INTEGER NOT NULL
            );
            "#
        )
        .execute(&pool)
        .await
        .context("Failed to initialize user profile table")?;
        
        Ok(Self { pool })
    }
    
    pub async fn set(&self, key: &str, value: serde_json::Value) -> Result<()> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
            
        sqlx::query(
            r#"
            INSERT INTO user_profile (key, value, updated_at)
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
        .await
        .context("Failed to set user profile key")?;
        
        Ok(())
    }
    
    pub async fn get(&self, key: &str) -> Result<Option<serde_json::Value>> {
        let result: Option<(serde_json::Value,)> = sqlx::query_as(
            "SELECT value FROM user_profile WHERE key = ?"
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get user profile key")?;
        
        Ok(result.map(|r| r.0))
    }
    
    pub async fn get_all(&self) -> Result<Vec<ProfileEntry>> {
        let entries = sqlx::query_as::<_, ProfileEntry>(
            "SELECT * FROM user_profile"
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch all profile entries")?;
        
        Ok(entries)
    }
    
    // Helper methods for common profile sections
    
    pub async fn get_goals(&self) -> Result<Vec<String>> {
        match self.get("goals").await? {
            Some(val) => serde_json::from_value(val).context("Failed to parse goals"),
            None => Ok(Vec::new()),
        }
    }
    
    pub async fn add_goal(&self, goal: String) -> Result<()> {
        let mut goals = self.get_goals().await?;
        if !goals.contains(&goal) {
            goals.push(goal);
            self.set("goals", serde_json::to_value(goals)?).await?;
        }
        Ok(())
    }
    
    pub async fn get_preferences(&self) -> Result<serde_json::Map<String, serde_json::Value>> {
        match self.get("preferences").await? {
            Some(val) => serde_json::from_value(val).context("Failed to parse preferences"),
            None => Ok(serde_json::Map::new()),
        }
    }
}
