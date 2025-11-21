use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePool, FromRow};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ConversationEntry {
    pub id: String,
    pub user_message: String,
    pub admin_response: String,
    pub timestamp: i64,
    pub context_snapshot: Option<serde_json::Value>,
    pub sentiment: Option<String>,
    pub intent: Option<String>,
}

pub struct ConversationStore {
    pool: SqlitePool,
}

impl ConversationStore {
    pub async fn new(db_url: &str) -> Result<Self> {
        let pool = SqlitePool::connect(db_url).await
            .context("Failed to connect to conversation database")?;
        
        // Create table if not exists
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS conversations (
                id TEXT PRIMARY KEY,
                user_message TEXT NOT NULL,
                admin_response TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                context_snapshot JSON,
                sentiment TEXT,
                intent TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_conversations_timestamp ON conversations(timestamp);
            "#
        )
        .execute(&pool)
        .await
        .context("Failed to initialize conversation table")?;
        
        Ok(Self { pool })
    }
    
    pub async fn add_entry(&self, entry: ConversationEntry) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO conversations (id, user_message, admin_response, timestamp, context_snapshot, sentiment, intent)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(entry.id)
        .bind(entry.user_message)
        .bind(entry.admin_response)
        .bind(entry.timestamp)
        .bind(entry.context_snapshot)
        .bind(entry.sentiment)
        .bind(entry.intent)
        .execute(&self.pool)
        .await
        .context("Failed to insert conversation entry")?;
        
        Ok(())
    }
    
    pub async fn get_recent_context(&self, limit: usize) -> Result<Vec<ConversationEntry>> {
        let entries = sqlx::query_as::<_, ConversationEntry>(
            r#"
            SELECT * FROM conversations 
            ORDER BY timestamp DESC 
            LIMIT ?
            "#
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch recent context")?;
        
        // Reverse to get chronological order
        Ok(entries.into_iter().rev().collect())
    }
    
    pub async fn search_history(&self, query: &str, limit: usize) -> Result<Vec<ConversationEntry>> {
        let pattern = format!("%{}%", query);
        let entries = sqlx::query_as::<_, ConversationEntry>(
            r#"
            SELECT * FROM conversations 
            WHERE user_message LIKE ? OR admin_response LIKE ?
            ORDER BY timestamp DESC 
            LIMIT ?
            "#
        )
        .bind(&pattern)
        .bind(&pattern)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .context("Failed to search conversation history")?;
        
        Ok(entries)
    }
}
