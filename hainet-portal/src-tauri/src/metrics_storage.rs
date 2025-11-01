//! # START OF FILE hainet-portal/src-tauri/src/metrics_storage.rs
//! Metrics historical storage and analytics system
//! 
//! Provides persistent storage for agent metrics snapshots, enabling historical
//! trend analysis and performance monitoring over time.

use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use sqlx::{Row, SqlitePool as Pool};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

/// Time range for querying historical metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    /// Start timestamp (Unix seconds), None = unlimited
    pub start: Option<i64>,
    /// End timestamp (Unix seconds), None = now
    pub end: Option<i64>,
}

impl Default for TimeRange {
    fn default() -> Self {
        Self {
            start: None,
            end: Some(chrono::Utc::now().timestamp()),
        }
    }
}

/// Aggregation interval for trend analysis
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrendInterval {
    Hourly,
    Daily,
    Weekly,
}

impl TrendInterval {
    /// Get interval duration in seconds
    pub fn duration_secs(&self) -> i64 {
        match self {
            TrendInterval::Hourly => 3600,      // 1 hour
            TrendInterval::Daily => 86400,      // 24 hours
            TrendInterval::Weekly => 604800,    // 7 days
        }
    }
}

/// Single data point in a trend analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendDataPoint {
    /// Timestamp of the data point (Unix seconds)
    pub timestamp: i64,
    /// Success rate (0.0-1.0)
    pub success_rate: f64,
    /// Average latency in milliseconds
    pub avg_latency_ms: f64,
    /// Total operations in this interval
    pub total_operations: u32,
    /// Total tokens used
    pub tokens_used: u64,
    /// Estimated cost in USD
    pub estimated_cost_usd: f64,
}

/// Metrics snapshot for historical storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub agent_type: String,
    pub timestamp: i64,
    pub success_rate: f64,
    pub avg_latency_ms: f64,
    pub total_operations: u32,
    pub successful_operations: u32,
    pub failed_operations: u32,
    pub tokens_used: u64,
    pub estimated_cost_usd: f64,
}

/// Historical metrics storage manager
pub struct MetricsStorage {
    pool: SqlitePool,
}

impl MetricsStorage {
    /// Create new metrics storage with SQLite backend
    pub async fn new(db_path: PathBuf) -> Result<Self, String> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create metrics directory: {}", e))?;
        }

        let db_url = format!("sqlite:{}", db_path.display());
        info!("Initializing metrics storage: {}", db_url);

        let pool = SqlitePool::connect(&db_url)
            .await
            .map_err(|e| format!("Failed to connect to metrics database: {}", e))?;

        let storage = Self { pool };
        storage.create_tables().await?;
        storage.run_migrations().await?;

        info!("Metrics storage initialized successfully");
        Ok(storage)
    }

    /// Create database schema
    async fn create_tables(&self) -> Result<(), String> {
        debug!("Creating metrics_snapshots table");

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS metrics_snapshots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                agent_type TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                success_rate REAL NOT NULL,
                avg_latency_ms REAL NOT NULL,
                total_operations INTEGER NOT NULL,
                successful_operations INTEGER NOT NULL,
                failed_operations INTEGER NOT NULL,
                tokens_used INTEGER NOT NULL,
                estimated_cost_usd REAL NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Failed to create metrics_snapshots table: {}", e))?;

        // Create indexes for efficient queries
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_timestamp ON metrics_snapshots(timestamp)")
            .execute(&self.pool)
            .await
            .map_err(|e| format!("Failed to create timestamp index: {}", e))?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_agent_type ON metrics_snapshots(agent_type)")
            .execute(&self.pool)
            .await
            .map_err(|e| format!("Failed to create agent_type index: {}", e))?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_agent_timestamp ON metrics_snapshots(agent_type, timestamp)")
            .execute(&self.pool)
            .await
            .map_err(|e| format!("Failed to create composite index: {}", e))?;

        debug!("Metrics tables created successfully");
        Ok(())
    }

    /// Run database migrations
    async fn run_migrations(&self) -> Result<(), String> {
        // Migration table for version tracking
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS metrics_migrations (
                version INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                applied_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Failed to create migrations table: {}", e))?;

        // Future migrations will be added here
        debug!("Metrics migrations complete");
        Ok(())
    }

    /// Record a metrics snapshot
    pub async fn record_snapshot(&self, snapshot: MetricsSnapshot) -> Result<(), String> {
        sqlx::query(
            r#"
            INSERT INTO metrics_snapshots (
                agent_type, timestamp, success_rate, avg_latency_ms,
                total_operations, successful_operations, failed_operations,
                tokens_used, estimated_cost_usd
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&snapshot.agent_type)
        .bind(snapshot.timestamp)
        .bind(snapshot.success_rate)
        .bind(snapshot.avg_latency_ms)
        .bind(snapshot.total_operations as i64)
        .bind(snapshot.successful_operations as i64)
        .bind(snapshot.failed_operations as i64)
        .bind(snapshot.tokens_used as i64)
        .bind(snapshot.estimated_cost_usd)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Failed to record metrics snapshot: {}", e))?;

        debug!("Recorded metrics snapshot for agent_type={}", snapshot.agent_type);
        Ok(())
    }

    /// Get historical metrics within a time range
    pub async fn get_historical_metrics(
        &self,
        agent_type: Option<String>,
        time_range: TimeRange,
    ) -> Result<Vec<MetricsSnapshot>, String> {
        let end_time = time_range.end.unwrap_or_else(|| chrono::Utc::now().timestamp());
        
        let snapshots = if let Some(agent_type) = agent_type {
            if let Some(start_time) = time_range.start {
                // Agent type + time range
                sqlx::query_as::<_, (String, i64, f64, f64, i32, i32, i32, i64, f64)>(
                    r#"
                    SELECT agent_type, timestamp, success_rate, avg_latency_ms,
                           total_operations, successful_operations, failed_operations,
                           tokens_used, estimated_cost_usd
                    FROM metrics_snapshots
                    WHERE agent_type = ? AND timestamp >= ? AND timestamp <= ?
                    ORDER BY timestamp ASC
                    "#,
                )
                .bind(&agent_type)
                .bind(start_time)
                .bind(end_time)
                .fetch_all(&self.pool)
                .await
            } else {
                // Agent type only
                sqlx::query_as::<_, (String, i64, f64, f64, i32, i32, i32, i64, f64)>(
                    r#"
                    SELECT agent_type, timestamp, success_rate, avg_latency_ms,
                           total_operations, successful_operations, failed_operations,
                           tokens_used, estimated_cost_usd
                    FROM metrics_snapshots
                    WHERE agent_type = ? AND timestamp <= ?
                    ORDER BY timestamp ASC
                    "#,
                )
                .bind(&agent_type)
                .bind(end_time)
                .fetch_all(&self.pool)
                .await
            }
        } else {
            if let Some(start_time) = time_range.start {
                // Time range only
                sqlx::query_as::<_, (String, i64, f64, f64, i32, i32, i32, i64, f64)>(
                    r#"
                    SELECT agent_type, timestamp, success_rate, avg_latency_ms,
                           total_operations, successful_operations, failed_operations,
                           tokens_used, estimated_cost_usd
                    FROM metrics_snapshots
                    WHERE timestamp >= ? AND timestamp <= ?
                    ORDER BY timestamp ASC
                    "#,
                )
                .bind(start_time)
                .bind(end_time)
                .fetch_all(&self.pool)
                .await
            } else {
                // All snapshots
                sqlx::query_as::<_, (String, i64, f64, f64, i32, i32, i32, i64, f64)>(
                    r#"
                    SELECT agent_type, timestamp, success_rate, avg_latency_ms,
                           total_operations, successful_operations, failed_operations,
                           tokens_used, estimated_cost_usd
                    FROM metrics_snapshots
                    WHERE timestamp <= ?
                    ORDER BY timestamp ASC
                    "#,
                )
                .bind(end_time)
                .fetch_all(&self.pool)
                .await
            }
        }
        .map_err(|e| format!("Failed to query historical metrics: {}", e))?;

        Ok(snapshots
            .into_iter()
            .map(|(agent_type, timestamp, success_rate, avg_latency_ms, total_ops, success_ops, failed_ops, tokens, cost)| {
                MetricsSnapshot {
                    agent_type,
                    timestamp,
                    success_rate,
                    avg_latency_ms,
                    total_operations: total_ops as u32,
                    successful_operations: success_ops as u32,
                    failed_operations: failed_ops as u32,
                    tokens_used: tokens as u64,
                    estimated_cost_usd: cost,
                }
            })
            .collect())
    }

    /// Compute trend data for a specific agent type
    pub async fn compute_trend(
        &self,
        agent_type: String,
        interval: TrendInterval,
        time_range: TimeRange,
    ) -> Result<Vec<TrendDataPoint>, String> {
        let interval_secs = interval.duration_secs();
        let end_time = time_range.end.unwrap_or_else(|| chrono::Utc::now().timestamp());
        let start_time = time_range.start.unwrap_or(end_time - (30 * 86400)); // Default: last 30 days

        // Aggregate snapshots into intervals
        let rows = sqlx::query(
            r#"
            SELECT 
                (timestamp / ?) * ? as interval_start,
                AVG(success_rate) as avg_success_rate,
                AVG(avg_latency_ms) as avg_latency,
                SUM(total_operations) as total_ops,
                SUM(tokens_used) as total_tokens,
                SUM(estimated_cost_usd) as total_cost
            FROM metrics_snapshots
            WHERE agent_type = ? AND timestamp >= ? AND timestamp <= ?
            GROUP BY interval_start
            ORDER BY interval_start ASC
            "#,
        )
        .bind(interval_secs)
        .bind(interval_secs)
        .bind(&agent_type)
        .bind(start_time)
        .bind(end_time)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("Failed to compute trend: {}", e))?;

        let mut trend = Vec::new();
        for row in rows {
            trend.push(TrendDataPoint {
                timestamp: row.get::<i64, _>(0),
                success_rate: row.get::<f64, _>(1),
                avg_latency_ms: row.get::<f64, _>(2),
                total_operations: row.get::<i64, _>(3) as u32,
                tokens_used: row.get::<i64, _>(4) as u64,
                estimated_cost_usd: row.get::<f64, _>(5),
            });
        }

        debug!("Computed {} trend data points for agent_type={}", trend.len(), agent_type);
        Ok(trend)
    }

    /// Prune old snapshots beyond retention period (default: 90 days)
    pub async fn prune_old_snapshots(&self, retention_days: u32) -> Result<u64, String> {
        let cutoff_time = chrono::Utc::now().timestamp() - (retention_days as i64 * 86400);

        let result = sqlx::query("DELETE FROM metrics_snapshots WHERE timestamp < ?")
            .bind(cutoff_time)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("Failed to prune old snapshots: {}", e))?;

        info!("Pruned {} old metrics snapshots (retention: {} days)", result.rows_affected(), retention_days);
        Ok(result.rows_affected())
    }

    /// Get total snapshot count
    pub async fn snapshot_count(&self) -> Result<i64, String> {
        let row = sqlx::query("SELECT COUNT(*) FROM metrics_snapshots")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| format!("Failed to get snapshot count: {}", e))?;

        Ok(row.get::<i64, _>(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn create_test_storage() -> MetricsStorage {
        let temp_path = std::env::temp_dir().join(format!("test_metrics_{}.db", uuid::Uuid::new_v4()));
        MetricsStorage::new(temp_path).await.unwrap()
    }

    #[tokio::test]
    async fn test_create_storage() {
        let storage = create_test_storage().await;
        assert_eq!(storage.snapshot_count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_record_snapshot() {
        let storage = create_test_storage().await;

        let snapshot = MetricsSnapshot {
            agent_type: "Admin".to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            success_rate: 0.95,
            avg_latency_ms: 120.5,
            total_operations: 100,
            successful_operations: 95,
            failed_operations: 5,
            tokens_used: 10000,
            estimated_cost_usd: 0.02,
        };

        storage.record_snapshot(snapshot).await.unwrap();
        assert_eq!(storage.snapshot_count().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_get_historical_metrics() {
        let storage = create_test_storage().await;
        let now = chrono::Utc::now().timestamp();

        // Record 3 snapshots
        for i in 0..3 {
            let snapshot = MetricsSnapshot {
                agent_type: "Worker".to_string(),
                timestamp: now - (i * 3600), // 1 hour apart
                success_rate: 0.9,
                avg_latency_ms: 100.0,
                total_operations: 50,
                successful_operations: 45,
                failed_operations: 5,
                tokens_used: 5000,
                estimated_cost_usd: 0.01,
            };
            storage.record_snapshot(snapshot).await.unwrap();
        }

        let time_range = TimeRange {
            start: Some(now - 7200), // Last 2 hours
            end: Some(now),
        };

        let snapshots = storage.get_historical_metrics(Some("Worker".to_string()), time_range).await.unwrap();
        assert_eq!(snapshots.len(), 3);
    }

    #[tokio::test]
    async fn test_compute_trend() {
        let storage = create_test_storage().await;
        let now = chrono::Utc::now().timestamp();

        // Record hourly snapshots
        for i in 0..24 {
            let snapshot = MetricsSnapshot {
                agent_type: "PM".to_string(),
                timestamp: now - (i * 3600),
                success_rate: 0.85 + (i as f64 * 0.005), // Slight trend
                avg_latency_ms: 150.0,
                total_operations: 20,
                successful_operations: 17,
                failed_operations: 3,
                tokens_used: 2000,
                estimated_cost_usd: 0.004,
            };
            storage.record_snapshot(snapshot).await.unwrap();
        }

        let time_range = TimeRange {
            start: Some(now - 86400), // Last 24 hours
            end: Some(now),
        };

        let trend = storage.compute_trend("PM".to_string(), TrendInterval::Hourly, time_range).await.unwrap();
        assert!(!trend.is_empty());
    }

    #[tokio::test]
    async fn test_prune_old_snapshots() {
        let storage = create_test_storage().await;
        let now = chrono::Utc::now().timestamp();

        // Record old and new snapshots
        let old_snapshot = MetricsSnapshot {
            agent_type: "Admin".to_string(),
            timestamp: now - (100 * 86400), // 100 days ago
            success_rate: 0.9,
            avg_latency_ms: 100.0,
            total_operations: 10,
            successful_operations: 9,
            failed_operations: 1,
            tokens_used: 1000,
            estimated_cost_usd: 0.002,
        };

        let new_snapshot = MetricsSnapshot {
            agent_type: "Admin".to_string(),
            timestamp: now,
            success_rate: 0.95,
            avg_latency_ms: 110.0,
            total_operations: 15,
            successful_operations: 14,
            failed_operations: 1,
            tokens_used: 1500,
            estimated_cost_usd: 0.003,
        };

        storage.record_snapshot(old_snapshot).await.unwrap();
        storage.record_snapshot(new_snapshot).await.unwrap();

        let pruned = storage.prune_old_snapshots(90).await.unwrap();
        assert_eq!(pruned, 1); // Old snapshot removed
        assert_eq!(storage.snapshot_count().await.unwrap(), 1); // New snapshot remains
    }
}
