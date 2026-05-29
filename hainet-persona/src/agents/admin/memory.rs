use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePool, FromRow};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ConversationEntry {
    pub id: String,
    pub session_id: String,
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
                session_id TEXT DEFAULT 'default',
                user_message TEXT NOT NULL,
                admin_response TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                context_snapshot JSON,
                sentiment TEXT,
                intent TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_conversations_timestamp ON conversations(timestamp);
            CREATE INDEX IF NOT EXISTS idx_conversations_session ON conversations(session_id);
            "#
        )
        .execute(&pool)
        .await
        .context("Failed to initialize conversation table")?;

        // Add session_id column if it doesn't exist (migration for existing DBs)
        let _ = sqlx::query("ALTER TABLE conversations ADD COLUMN session_id TEXT DEFAULT 'default'")
            .execute(&pool)
            .await;

        
        Ok(Self { pool })
    }
    
    pub async fn add_entry(&self, entry: ConversationEntry) -> Result<()> {
        // Check if an identical entry already exists (same user_message and admin_response in same session)
        let existing = sqlx::query_as::<_, ConversationEntry>(
            r#"
            SELECT * FROM conversations 
            WHERE user_message = ? AND admin_response = ? AND session_id = ?
            ORDER BY timestamp DESC
            LIMIT 1
            "#
        )
        .bind(&entry.user_message)
        .bind(&entry.admin_response)
        .bind(&entry.session_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to check for existing conversation entry")?;
        
        if let Some(existing_entry) = existing {
            // Update the existing entry with new timestamp and context
            tracing::debug!("Updating existing conversation entry {} instead of creating duplicate", existing_entry.id);
            sqlx::query(
                r#"
                UPDATE conversations 
                SET timestamp = ?, context_snapshot = ?, sentiment = ?, intent = ?
                WHERE id = ?
                "#
            )
            .bind(entry.timestamp)
            .bind(entry.context_snapshot)
            .bind(entry.sentiment)
            .bind(entry.intent)
            .bind(existing_entry.id)
            .execute(&self.pool)
            .await
            .context("Failed to update existing conversation entry")?;
        } else {
            // Insert new entry
            sqlx::query(
                r#"
                INSERT INTO conversations (id, session_id, user_message, admin_response, timestamp, context_snapshot, sentiment, intent)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                "#
            )
            .bind(entry.id)
            .bind(entry.session_id)
            .bind(entry.user_message)
            .bind(entry.admin_response)
            .bind(entry.timestamp)
            .bind(entry.context_snapshot)
            .bind(entry.sentiment)
            .bind(entry.intent)
            .execute(&self.pool)
            .await
            .context("Failed to insert conversation entry")?;
        }
        
        Ok(())
    }
    
    pub async fn get_recent_context(&self, session_id: &str, limit: usize) -> Result<Vec<ConversationEntry>> {
        let entries = sqlx::query_as::<_, ConversationEntry>(
            r#"
            SELECT * FROM conversations 
            WHERE session_id = ?
            ORDER BY timestamp DESC 
            LIMIT ?
            "#
        )
        .bind(session_id)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch recent context")?;
        
        // Reverse to get chronological order
        Ok(entries.into_iter().rev().collect())
    }
    
    pub async fn search_history(&self, session_id: Option<&str>, query: &str, limit: usize) -> Result<Vec<ConversationEntry>> {
        let pattern = format!("%{}%", query);
        
        let entries = if let Some(sid) = session_id {
            sqlx::query_as::<_, ConversationEntry>(
                r#"
                SELECT * FROM conversations 
                WHERE session_id = ? AND (user_message LIKE ? OR admin_response LIKE ?)
                ORDER BY timestamp DESC 
                LIMIT ?
                "#
            )
            .bind(sid)
            .bind(&pattern)
            .bind(&pattern)
            .bind(limit as i64)
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query_as::<_, ConversationEntry>(
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
        }.context("Failed to search conversation history")?;
        
        Ok(entries)
    }
}
