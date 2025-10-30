//! # START OF FILE hainet-persona/src/agents/metrics.rs
//! Agent Performance Metrics System
//! 
//! Tracks operation success rates, response times, token usage, and other
//! performance indicators for agent optimization.

use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};
use anyhow::Result;
use sqlx::{SqlitePool, Row};
use crate::agents::{AgentType, AgentId};
use crate::agents::llm_config::AgentLLMConfig;
use sha2::{Sha256, Digest};

/// Metrics for a specific agent instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMetrics {
    pub agent_type: AgentType,
    pub agent_id: AgentId,
    pub config_hash: String,
    
    // Success metrics
    pub total_operations: u64,
    pub successful_operations: u64,
    pub failed_operations: u64,
    pub success_rate: f32,
    
    // Performance metrics
    pub avg_response_time_ms: f32,
    pub avg_tokens_used: f32,
    pub json_parse_success_rate: f32,
    pub retry_count: u64,
    
    // Quality metrics
    pub task_completion_rate: f32,
    pub validation_pass_rate: f32,
    pub syntax_error_rate: f32,
    
    // Timestamps
    pub first_operation: SystemTime,
    pub last_operation: SystemTime,
}

/// Result of a single agent operation
#[derive(Debug, Clone)]
pub struct OperationResult {
    pub agent_type: AgentType,
    pub agent_id: AgentId,
    pub config_hash: String,
    pub operation_type: String,
    pub success: bool,
    pub response_time: Duration,
    pub tokens_used: Option<u32>,
    pub error_message: Option<String>,
    pub json_parse_success: bool,
    pub had_syntax_errors: bool,
    pub validation_passed: bool,
}

/// Collects and persists agent metrics
pub struct MetricsCollector {
    pool: SqlitePool,
}

impl MetricsCollector {
    /// Create new metrics collector with SQLite backend
    pub async fn new(db_path: &str) -> Result<Self> {
        let pool = SqlitePool::connect(db_path).await?;
        let collector = Self { pool };
        collector.create_tables().await?;
        Ok(collector)
    }
    
    /// Create database schema for metrics
    async fn create_tables(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS agent_metrics (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                agent_type TEXT NOT NULL,
                agent_id TEXT NOT NULL,
                config_hash TEXT NOT NULL,
                operation_type TEXT NOT NULL,
                success BOOLEAN NOT NULL,
                response_time_ms INTEGER NOT NULL,
                tokens_used INTEGER,
                error_message TEXT,
                json_parse_success BOOLEAN NOT NULL DEFAULT 1,
                had_syntax_errors BOOLEAN NOT NULL DEFAULT 0,
                validation_passed BOOLEAN NOT NULL DEFAULT 1,
                timestamp INTEGER NOT NULL
            )
            "#
        )
        .execute(&self.pool)
        .await?;
        
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_agent_type_config ON agent_metrics(agent_type, config_hash)"
        )
        .execute(&self.pool)
        .await?;
        
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_timestamp ON agent_metrics(timestamp)"
        )
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    /// Record a single operation result
    pub async fn record_operation(&self, result: OperationResult) -> Result<()> {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_secs() as i64;
        
        sqlx::query(
            r#"
            INSERT INTO agent_metrics (
                agent_type, agent_id, config_hash, operation_type,
                success, response_time_ms, tokens_used, error_message,
                json_parse_success, had_syntax_errors, validation_passed,
                timestamp
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(result.agent_type.to_string())
        .bind(result.agent_id.to_string())
        .bind(&result.config_hash)
        .bind(&result.operation_type)
        .bind(result.success)
        .bind(result.response_time.as_millis() as i64)
        .bind(result.tokens_used.map(|t| t as i64))
        .bind(&result.error_message)
        .bind(result.json_parse_success)
        .bind(result.had_syntax_errors)
        .bind(result.validation_passed)
        .bind(timestamp)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    /// Get metrics for specific agent type (last N operations)
    pub async fn get_recent(&self, agent_type: AgentType, limit: u32) -> Result<Vec<OperationResult>> {
        let rows = sqlx::query(
            r#"
            SELECT * FROM agent_metrics
            WHERE agent_type = ?
            ORDER BY timestamp DESC
            LIMIT ?
            "#
        )
        .bind(agent_type.to_string())
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;
        
        let mut results = Vec::new();
        for row in rows {
            let agent_id: String = row.get("agent_id");
            let response_time_ms: i64 = row.get("response_time_ms");
            let tokens_used: Option<i64> = row.get("tokens_used");
            
            results.push(OperationResult {
                agent_type,
                agent_id: AgentId::new(agent_type, agent_id),
                config_hash: row.get("config_hash"),
                operation_type: row.get("operation_type"),
                success: row.get("success"),
                response_time: Duration::from_millis(response_time_ms as u64),
                tokens_used: tokens_used.map(|t| t as u32),
                error_message: row.get("error_message"),
                json_parse_success: row.get("json_parse_success"),
                had_syntax_errors: row.get("had_syntax_errors"),
                validation_passed: row.get("validation_passed"),
            });
        }
        
        Ok(results)
    }
    
    /// Get aggregated metrics for agent type
    pub async fn get_aggregate(&self, agent_type: AgentType) -> Result<AgentMetrics> {
        let row = sqlx::query(
            r#"
            SELECT 
                COUNT(*) as total_operations,
                SUM(CASE WHEN success = 1 THEN 1 ELSE 0 END) as successful_operations,
                SUM(CASE WHEN success = 0 THEN 1 ELSE 0 END) as failed_operations,
                AVG(response_time_ms) as avg_response_time_ms,
                AVG(CASE WHEN tokens_used IS NOT NULL THEN tokens_used ELSE 0 END) as avg_tokens_used,
                SUM(CASE WHEN json_parse_success = 1 THEN 1 ELSE 0 END) as json_success_count,
                SUM(CASE WHEN had_syntax_errors = 1 THEN 1 ELSE 0 END) as syntax_error_count,
                SUM(CASE WHEN validation_passed = 1 THEN 1 ELSE 0 END) as validation_pass_count,
                MIN(timestamp) as first_timestamp,
                MAX(timestamp) as last_timestamp,
                config_hash
            FROM agent_metrics
            WHERE agent_type = ?
            "#
        )
        .bind(agent_type.to_string())
        .fetch_one(&self.pool)
        .await?;
        
        let total_ops: i64 = row.get("total_operations");
        let successful: i64 = row.get("successful_operations");
        let failed: i64 = row.get("failed_operations");
        let json_success: i64 = row.get("json_success_count");
        let syntax_errors: i64 = row.get("syntax_error_count");
        let validation_passed: i64 = row.get("validation_pass_count");
        let first_ts: i64 = row.get("first_timestamp");
        let last_ts: i64 = row.get("last_timestamp");
        
        let total_ops_f = total_ops as f32;
        
        Ok(AgentMetrics {
            agent_type,
            agent_id: AgentId::new(agent_type, "aggregate".to_string()),
            config_hash: row.get("config_hash"),
            total_operations: total_ops as u64,
            successful_operations: successful as u64,
            failed_operations: failed as u64,
            success_rate: if total_ops > 0 { successful as f32 / total_ops_f } else { 0.0 },
            avg_response_time_ms: row.get("avg_response_time_ms"),
            avg_tokens_used: row.get("avg_tokens_used"),
            json_parse_success_rate: if total_ops > 0 { json_success as f32 / total_ops_f } else { 0.0 },
            retry_count: 0, // TODO: Track retries separately
            task_completion_rate: if total_ops > 0 { successful as f32 / total_ops_f } else { 0.0 },
            validation_pass_rate: if total_ops > 0 { validation_passed as f32 / total_ops_f } else { 0.0 },
            syntax_error_rate: if total_ops > 0 { syntax_errors as f32 / total_ops_f } else { 0.0 },
            first_operation: SystemTime::UNIX_EPOCH + Duration::from_secs(first_ts as u64),
            last_operation: SystemTime::UNIX_EPOCH + Duration::from_secs(last_ts as u64),
        })
    }
    
    /// Count total operations for agent type
    pub async fn count_operations(&self, agent_type: AgentType) -> Result<u64> {
        let row = sqlx::query(
            "SELECT COUNT(*) as count FROM agent_metrics WHERE agent_type = ?"
        )
        .bind(agent_type.to_string())
        .fetch_one(&self.pool)
        .await?;
        
        let count: i64 = row.get("count");
        Ok(count as u64)
    }
}

/// Generate deterministic hash of config for grouping metrics
pub fn hash_config(config: &AgentLLMConfig) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!("{:?}", config).as_bytes());
    format!("{:x}", hasher.finalize())[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_metrics_collector_creation() {
        let collector = MetricsCollector::new(":memory:").await.unwrap();
        // Should create without error
    }
    
    #[tokio::test]
    async fn test_record_and_retrieve_operation() {
        let collector = MetricsCollector::new(":memory:").await.unwrap();
        
        let result = OperationResult {
            agent_type: AgentType::Admin,
            agent_id: AgentId::new(AgentType::Admin, "test".to_string()),
            config_hash: "test_hash".to_string(),
            operation_type: "test_op".to_string(),
            success: true,
            response_time: Duration::from_millis(100),
            tokens_used: Some(50),
            error_message: None,
            json_parse_success: true,
            had_syntax_errors: false,
            validation_passed: true,
        };
        
        collector.record_operation(result).await.unwrap();
        
        let count = collector.count_operations(AgentType::Admin).await.unwrap();
        assert_eq!(count, 1);
    }
    
    #[tokio::test]
    async fn test_aggregate_metrics() {
        let collector = MetricsCollector::new(":memory:").await.unwrap();
        
        // Record multiple operations
        for i in 0..10 {
            let result = OperationResult {
                agent_type: AgentType::PM,
                agent_id: AgentId::new(AgentType::PM, "test".to_string()),
                config_hash: "test_hash".to_string(),
                operation_type: "test_op".to_string(),
                success: i % 3 != 0, // 2/3 success rate
                response_time: Duration::from_millis(100 + i as u64 * 10),
                tokens_used: Some(50 + i),
                error_message: None,
                json_parse_success: true,
                had_syntax_errors: false,
                validation_passed: i % 3 != 0,
            };
            
            collector.record_operation(result).await.unwrap();
        }
        
        let metrics = collector.get_aggregate(AgentType::PM).await.unwrap();
        assert_eq!(metrics.total_operations, 10);
        assert!(metrics.success_rate > 0.6 && metrics.success_rate < 0.7);
    }
    
    #[test]
    fn test_config_hash() {
        let config = AgentLLMConfig::for_admin();
        let hash1 = hash_config(&config);
        let hash2 = hash_config(&config);
        assert_eq!(hash1, hash2); // Deterministic
        
        let mut config2 = config.clone();
        config2.temperature = 0.5;
        let hash3 = hash_config(&config2);
        assert_ne!(hash1, hash3); // Different configs = different hashes
    }
}
