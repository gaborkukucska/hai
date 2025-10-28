//! # START OF FILE hainet-persona/src/projects/migrations.rs
//! Database Migration System
//! 
//! Provides schema versioning and automatic migration for the project storage layer.

use sqlx::{SqlitePool, Row};
use anyhow::Result;
use tracing::{info, debug};

/// Represents a single database migration
pub struct Migration {
    pub version: u32,
    pub name: &'static str,
    pub up_sql: &'static str,
}

/// All migrations in order
const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "add_task_revision_fields",
        up_sql: r#"
            ALTER TABLE tasks ADD COLUMN pm_feedback TEXT;
            ALTER TABLE tasks ADD COLUMN revision_count INTEGER NOT NULL DEFAULT 0;
            ALTER TABLE tasks ADD COLUMN max_revisions INTEGER NOT NULL DEFAULT 2;
        "#,
    },
];

/// Migration runner for project storage database
pub struct MigrationRunner {
    pool: SqlitePool,
}

impl MigrationRunner {
    /// Create a new migration runner
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Initialize the schema_migrations table
    async fn init_migrations_table(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                applied_at TEXT NOT NULL
            )
            "#
        )
        .execute(&self.pool)
        .await?;
        
        debug!("Schema migrations table initialized");
        Ok(())
    }

    /// Get the current schema version
    pub async fn current_version(&self) -> Result<u32> {
        // Ensure migrations table exists
        self.init_migrations_table().await?;

        let result = sqlx::query("SELECT MAX(version) as version FROM schema_migrations")
            .fetch_one(&self.pool)
            .await?;

        let version: Option<i64> = result.try_get("version")?;
        Ok(version.unwrap_or(0) as u32)
    }

    /// Check if migrations are needed
    pub async fn needs_migration(&self) -> Result<bool> {
        let current = self.current_version().await?;
        let latest = MIGRATIONS.last().map(|m| m.version).unwrap_or(0);
        Ok(current < latest)
    }

    /// Run all pending migrations
    pub async fn run_migrations(&self) -> Result<()> {
        self.init_migrations_table().await?;

        let current_version = self.current_version().await?;
        debug!("Current schema version: {}", current_version);

        let pending: Vec<&Migration> = MIGRATIONS
            .iter()
            .filter(|m| m.version > current_version)
            .collect();

        if pending.is_empty() {
            debug!("No pending migrations");
            return Ok(());
        }

        info!("Running {} pending migration(s)", pending.len());

        for migration in pending {
            self.apply_migration(migration).await?;
        }

        info!("All migrations completed successfully");
        Ok(())
    }

    /// Apply a single migration
    async fn apply_migration(&self, migration: &Migration) -> Result<()> {
        info!("Applying migration {}: {}", migration.version, migration.name);

        // Start transaction
        let mut tx = self.pool.begin().await?;

        // Execute migration SQL
        // Split on semicolons to handle multiple statements
        for statement in migration.up_sql.split(';') {
            let statement = statement.trim();
            if !statement.is_empty() {
                sqlx::query(statement).execute(&mut *tx).await?;
            }
        }

        // Record migration in schema_migrations table
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO schema_migrations (version, name, applied_at) VALUES (?, ?, ?)"
        )
        .bind(migration.version as i64)
        .bind(migration.name)
        .bind(&now)
        .execute(&mut *tx)
        .await?;

        // Commit transaction
        tx.commit().await?;

        info!("Migration {} applied successfully", migration.version);
        Ok(())
    }

    /// Get list of applied migrations
    pub async fn applied_migrations(&self) -> Result<Vec<(u32, String, String)>> {
        self.init_migrations_table().await?;

        let rows = sqlx::query(
            "SELECT version, name, applied_at FROM schema_migrations ORDER BY version ASC"
        )
        .fetch_all(&self.pool)
        .await?;

        let mut migrations = Vec::new();
        for row in rows {
            let version: i64 = row.try_get("version")?;
            let name: String = row.try_get("name")?;
            let applied_at: String = row.try_get("applied_at")?;
            migrations.push((version as u32, name, applied_at));
        }

        Ok(migrations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn create_test_pool() -> Result<SqlitePool> {
        // Use in-memory database for tests
        let pool = SqlitePool::connect("sqlite::memory:").await?;
        Ok(pool)
    }

    #[tokio::test]
    async fn test_init_migrations_table() {
        let pool = create_test_pool().await.unwrap();
        let runner = MigrationRunner::new(pool.clone());
        
        runner.init_migrations_table().await.unwrap();
        
        // Verify table exists
        let result = sqlx::query("SELECT name FROM sqlite_master WHERE type='table' AND name='schema_migrations'")
            .fetch_one(&pool)
            .await;
        
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_current_version_empty_db() {
        let pool = create_test_pool().await.unwrap();
        let runner = MigrationRunner::new(pool);
        
        let version = runner.current_version().await.unwrap();
        assert_eq!(version, 0);
    }

    #[tokio::test]
    async fn test_needs_migration_new_db() {
        let pool = create_test_pool().await.unwrap();
        let runner = MigrationRunner::new(pool);
        
        let needs = runner.needs_migration().await.unwrap();
        assert!(needs);
    }

    #[tokio::test]
    async fn test_apply_migrations() {
        let pool = create_test_pool().await.unwrap();
        
        // Create tables first (simulating existing database)
        sqlx::query(
            r#"
            CREATE TABLE tasks (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                title TEXT NOT NULL,
                description TEXT NOT NULL,
                status TEXT NOT NULL
            )
            "#
        )
        .execute(&pool)
        .await
        .unwrap();

        let runner = MigrationRunner::new(pool.clone());
        
        // Run migrations
        runner.run_migrations().await.unwrap();
        
        // Verify version updated
        let version = runner.current_version().await.unwrap();
        assert_eq!(version, 1);
        
        // Verify new columns exist
        let result = sqlx::query("SELECT pm_feedback, revision_count, max_revisions FROM tasks")
            .fetch_optional(&pool)
            .await;
        
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_idempotent_migrations() {
        let pool = create_test_pool().await.unwrap();
        
        // Create tasks table
        sqlx::query(
            r#"
            CREATE TABLE tasks (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                title TEXT NOT NULL,
                description TEXT NOT NULL,
                status TEXT NOT NULL
            )
            "#
        )
        .execute(&pool)
        .await
        .unwrap();

        let runner = MigrationRunner::new(pool.clone());
        
        // Run migrations twice
        runner.run_migrations().await.unwrap();
        let result = runner.run_migrations().await;
        
        // Should not error
        assert!(result.is_ok());
        
        // Version should still be 1
        let version = runner.current_version().await.unwrap();
        assert_eq!(version, 1);
    }

    #[tokio::test]
    async fn test_applied_migrations_list() {
        let pool = create_test_pool().await.unwrap();
        
        // Create tasks table
        sqlx::query(
            r#"
            CREATE TABLE tasks (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                title TEXT NOT NULL,
                description TEXT NOT NULL,
                status TEXT NOT NULL
            )
            "#
        )
        .execute(&pool)
        .await
        .unwrap();

        let runner = MigrationRunner::new(pool);
        
        runner.run_migrations().await.unwrap();
        
        let applied = runner.applied_migrations().await.unwrap();
        assert_eq!(applied.len(), 1);
        assert_eq!(applied[0].0, 1);
        assert_eq!(applied[0].1, "add_task_revision_fields");
    }
}
