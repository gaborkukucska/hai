//! # START OF FILE hainet-persona/src/projects/storage.rs
//! SQLite Storage Backend for Projects
//! 
//! Provides persistent storage for projects, tasks, and milestones using SQLite.
//! All entities are stored in separate tables with proper foreign key relationships.

use sqlx::{SqlitePool, Row, sqlite::SqliteRow};
use anyhow::Result;
use std::time::{SystemTime, UNIX_EPOCH};

use super::project::{Project, ProjectId, ProjectStatus};
use super::task::{Task, TaskId, TaskStatus};
use super::milestone::{Milestone, MilestoneId, MilestoneStatus};
use super::migrations::MigrationRunner;

/// SQLite storage backend for project management
pub struct ProjectStorage {
    pool: SqlitePool,
}

impl ProjectStorage {
    /// Create a new storage instance with the given database path
    pub async fn new(db_path: &str) -> Result<Self> {
        let pool = SqlitePool::connect(db_path).await?;
        let storage = Self { pool };
        
        // Create base tables FIRST (migrations expect these to exist)
        storage.create_tables().await?;
        
        // Then run migrations to add new columns
        storage.run_migrations().await?;
        
        Ok(storage)
    }

    /// Run all pending database migrations
    async fn run_migrations(&self) -> Result<()> {
        let runner = MigrationRunner::new(self.pool.clone());
        runner.run_migrations().await
    }

    /// Create all necessary database tables
    pub async fn create_tables(&self) -> Result<()> {
        // Projects table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS projects (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                overview TEXT NOT NULL,
                status TEXT NOT NULL,
                pm_agent_id TEXT,
                worker_agent_ids TEXT NOT NULL,
                milestone_ids TEXT NOT NULL,
                task_ids TEXT NOT NULL,
                failure_reason TEXT,
                created_at INTEGER NOT NULL,
                started_at INTEGER,
                completed_at INTEGER,
                deleted_at INTEGER
            )
            "#
        )
        .execute(&self.pool)
        .await?;

        // Tasks table (migration-added columns will be added via migrations)
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                title TEXT NOT NULL,
                description TEXT NOT NULL,
                assigned_worker TEXT,
                dependencies TEXT NOT NULL,
                status TEXT NOT NULL,
                deliverables TEXT NOT NULL,
                validation_notes TEXT,
                blocking_reason TEXT,
                failure_reason TEXT,
                created_at INTEGER NOT NULL,
                assigned_at INTEGER,
                started_at INTEGER,
                completed_at INTEGER,
                FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
            )
            "#
        )
        .execute(&self.pool)
        .await?;

        // Milestones table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS milestones (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                title TEXT NOT NULL,
                description TEXT NOT NULL,
                deadline INTEGER,
                task_ids TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                completed_at INTEGER,
                FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
            )
            "#
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // ========== Project CRUD ==========

    /// Create a new project in the database
    pub async fn create_project(&self, project: &Project) -> Result<()> {
        let worker_ids = serde_json::to_string(&project.worker_agent_ids)?;
        let milestone_ids = serde_json::to_string(&project.milestone_ids)?;
        let task_ids = serde_json::to_string(&project.task_ids)?;

        sqlx::query(
            r#"
            INSERT INTO projects (
                id, title, overview, status, pm_agent_id, 
                worker_agent_ids, milestone_ids, task_ids, failure_reason,
                created_at, started_at, completed_at, deleted_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(project.id.to_string())
        .bind(&project.title)
        .bind(&project.overview)
        .bind(project.status.to_string())
        .bind(project.pm_agent_id.as_ref().map(|id| serde_json::to_string(id).unwrap()))
        .bind(worker_ids)
        .bind(milestone_ids)
        .bind(task_ids)
        .bind(&project.failure_reason)
        .bind(system_time_to_i64(project.created_at))
        .bind(project.started_at.map(system_time_to_i64))
        .bind(project.completed_at.map(system_time_to_i64))
        .bind(project.deleted_at.map(system_time_to_i64))
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get a project by ID
    pub async fn get_project(&self, id: &ProjectId) -> Result<Option<Project>> {
        let row = sqlx::query(
            "SELECT * FROM projects WHERE id = ? AND deleted_at IS NULL"
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => Ok(Some(row_to_project(&row)?)),
            None => Ok(None),
        }
    }

    /// Update an existing project
    pub async fn update_project(&self, project: &Project) -> Result<()> {
        let worker_ids = serde_json::to_string(&project.worker_agent_ids)?;
        let milestone_ids = serde_json::to_string(&project.milestone_ids)?;
        let task_ids = serde_json::to_string(&project.task_ids)?;

        sqlx::query(
            r#"
            UPDATE projects SET
                title = ?, overview = ?, status = ?, pm_agent_id = ?,
                worker_agent_ids = ?, milestone_ids = ?, task_ids = ?,
                failure_reason = ?, started_at = ?, completed_at = ?, deleted_at = ?
            WHERE id = ?
            "#
        )
        .bind(&project.title)
        .bind(&project.overview)
        .bind(project.status.to_string())
        .bind(project.pm_agent_id.as_ref().map(|id| serde_json::to_string(id).unwrap()))
        .bind(worker_ids)
        .bind(milestone_ids)
        .bind(task_ids)
        .bind(&project.failure_reason)
        .bind(project.started_at.map(system_time_to_i64))
        .bind(project.completed_at.map(system_time_to_i64))
        .bind(project.deleted_at.map(system_time_to_i64))
        .bind(project.id.to_string())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Soft delete a project (sets deleted_at timestamp)
    pub async fn delete_project(&self, id: &ProjectId) -> Result<()> {
        sqlx::query(
            "UPDATE projects SET deleted_at = ? WHERE id = ?"
        )
        .bind(system_time_to_i64(SystemTime::now()))
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// List all active projects (not deleted)
    pub async fn list_active_projects(&self) -> Result<Vec<Project>> {
        let rows = sqlx::query(
            "SELECT * FROM projects WHERE deleted_at IS NULL ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(row_to_project).collect()
    }

    // ========== Task CRUD ==========

    /// Create a new task in the database
    pub async fn create_task(&self, task: &Task) -> Result<()> {
        let dependencies = serde_json::to_string(&task.dependencies)?;
        let deliverables = serde_json::to_string(&task.deliverables)?;

        sqlx::query(
            r#"
            INSERT INTO tasks (
                id, project_id, title, description, assigned_worker,
                dependencies, status, deliverables, validation_notes,
                pm_feedback, revision_count, max_revisions,
                blocking_reason, failure_reason, created_at, assigned_at,
                started_at, completed_at, last_status_change
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(task.id.to_string())
        .bind(task.project_id.to_string())
        .bind(&task.title)
        .bind(&task.description)
        .bind(task.assigned_worker.as_ref().map(|id| serde_json::to_string(id).unwrap()))
        .bind(dependencies)
        .bind(task.status.to_string())
        .bind(deliverables)
        .bind(&task.validation_notes)
        .bind(&task.pm_feedback)
        .bind(task.revision_count as i64)
        .bind(task.max_revisions as i64)
        .bind(&task.blocking_reason)
        .bind(&task.failure_reason)
        .bind(system_time_to_i64(task.created_at))
        .bind(task.assigned_at.map(system_time_to_i64))
        .bind(task.started_at.map(system_time_to_i64))
        .bind(task.completed_at.map(system_time_to_i64))
        .bind(system_time_to_i64(task.last_status_change))
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get a task by ID
    pub async fn get_task(&self, id: &TaskId) -> Result<Option<Task>> {
        let row = sqlx::query("SELECT * FROM tasks WHERE id = ?")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(row) => Ok(Some(row_to_task(&row)?)),
            None => Ok(None),
        }
    }

    /// Update an existing task
    pub async fn update_task(&self, task: &Task) -> Result<()> {
        let dependencies = serde_json::to_string(&task.dependencies)?;
        let deliverables = serde_json::to_string(&task.deliverables)?;

        sqlx::query(
            r#"
            UPDATE tasks SET
                title = ?, description = ?, assigned_worker = ?,
                dependencies = ?, status = ?, deliverables = ?,
                validation_notes = ?, pm_feedback = ?, revision_count = ?,
                max_revisions = ?, blocking_reason = ?, failure_reason = ?,
                assigned_at = ?, started_at = ?, completed_at = ?,
                last_status_change = ?
            WHERE id = ?
            "#
        )
        .bind(&task.title)
        .bind(&task.description)
        .bind(task.assigned_worker.as_ref().map(|id| serde_json::to_string(id).unwrap()))
        .bind(dependencies)
        .bind(task.status.to_string())
        .bind(deliverables)
        .bind(&task.validation_notes)
        .bind(&task.pm_feedback)
        .bind(task.revision_count as i64)
        .bind(task.max_revisions as i64)
        .bind(&task.blocking_reason)
        .bind(&task.failure_reason)
        .bind(task.assigned_at.map(system_time_to_i64))
        .bind(task.started_at.map(system_time_to_i64))
        .bind(task.completed_at.map(system_time_to_i64))
        .bind(system_time_to_i64(task.last_status_change))
        .bind(task.id.to_string())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// List all tasks for a project
    pub async fn list_project_tasks(&self, project_id: &ProjectId) -> Result<Vec<Task>> {
        let rows = sqlx::query(
            "SELECT * FROM tasks WHERE project_id = ? ORDER BY created_at ASC"
        )
        .bind(project_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(row_to_task).collect()
    }

    // ========== Milestone CRUD ==========

    /// Create a new milestone in the database
    pub async fn create_milestone(&self, milestone: &Milestone) -> Result<()> {
        let task_ids = serde_json::to_string(&milestone.task_ids)?;

        sqlx::query(
            r#"
            INSERT INTO milestones (
                id, project_id, title, description, deadline,
                task_ids, status, created_at, completed_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(milestone.id.to_string())
        .bind(milestone.project_id.to_string())
        .bind(&milestone.title)
        .bind(&milestone.description)
        .bind(milestone.deadline.map(system_time_to_i64))
        .bind(task_ids)
        .bind(milestone.status.to_string())
        .bind(system_time_to_i64(milestone.created_at))
        .bind(milestone.completed_at.map(system_time_to_i64))
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get a milestone by ID
    pub async fn get_milestone(&self, id: &MilestoneId) -> Result<Option<Milestone>> {
        let row = sqlx::query("SELECT * FROM milestones WHERE id = ?")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(row) => Ok(Some(row_to_milestone(&row)?)),
            None => Ok(None),
        }
    }

    /// Update an existing milestone
    pub async fn update_milestone(&self, milestone: &Milestone) -> Result<()> {
        let task_ids = serde_json::to_string(&milestone.task_ids)?;

        sqlx::query(
            r#"
            UPDATE milestones SET
                title = ?, description = ?, deadline = ?,
                task_ids = ?, status = ?, completed_at = ?
            WHERE id = ?
            "#
        )
        .bind(&milestone.title)
        .bind(&milestone.description)
        .bind(milestone.deadline.map(system_time_to_i64))
        .bind(task_ids)
        .bind(milestone.status.to_string())
        .bind(milestone.completed_at.map(system_time_to_i64))
        .bind(milestone.id.to_string())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// List all milestones for a project
    pub async fn list_project_milestones(&self, project_id: &ProjectId) -> Result<Vec<Milestone>> {
        let rows = sqlx::query(
            "SELECT * FROM milestones WHERE project_id = ? ORDER BY created_at ASC"
        )
        .bind(project_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(row_to_milestone).collect()
    }
}

// ========== Helper Functions ==========

/// Convert SystemTime to Unix timestamp (i64)
fn system_time_to_i64(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs() as i64
}

/// Convert Unix timestamp (i64) to SystemTime
fn i64_to_system_time(timestamp: i64) -> SystemTime {
    UNIX_EPOCH + std::time::Duration::from_secs(timestamp as u64)
}

/// Convert SQLite row to Project
fn row_to_project(row: &SqliteRow) -> Result<Project> {
    let status_str: String = row.try_get("status")?;
    let status = match status_str.as_str() {
        "Created" => ProjectStatus::Created,
        "Active" => ProjectStatus::Active,
        "Paused" => ProjectStatus::Paused,
        "Completed" => ProjectStatus::Completed,
        "Failed" => ProjectStatus::Failed,
        "Cancelled" => ProjectStatus::Cancelled,
        _ => anyhow::bail!("Unknown project status: {}", status_str),
    };

    Ok(Project {
        id: ProjectId::from_string(&row.try_get::<String, _>("id")?)?,
        title: row.try_get("title")?,
        overview: row.try_get("overview")?,
        status,
        pm_agent_id: row.try_get::<Option<String>, _>("pm_agent_id")?
            .and_then(|s| serde_json::from_str(&s).ok()),
        worker_agent_ids: serde_json::from_str(&row.try_get::<String, _>("worker_agent_ids")?)?,
        milestone_ids: serde_json::from_str(&row.try_get::<String, _>("milestone_ids")?)?,
        task_ids: serde_json::from_str(&row.try_get::<String, _>("task_ids")?)?,
        created_at: i64_to_system_time(row.try_get("created_at")?),
        started_at: row.try_get::<Option<i64>, _>("started_at")?.map(i64_to_system_time),
        completed_at: row.try_get::<Option<i64>, _>("completed_at")?.map(i64_to_system_time),
        deleted_at: row.try_get::<Option<i64>, _>("deleted_at")?.map(i64_to_system_time),
        failure_reason: row.try_get("failure_reason")?,
    })
}

/// Convert SQLite row to Task
fn row_to_task(row: &SqliteRow) -> Result<Task> {
    let status_str: String = row.try_get("status")?;
    let status = match status_str.as_str() {
        "Unassigned" => TaskStatus::Unassigned,
        "Assigned" => TaskStatus::Assigned,
        "InProgress" | "In Progress" => TaskStatus::InProgress,
        "Blocked" => TaskStatus::Blocked,
        "UnderReview" | "Under Review" => TaskStatus::UnderReview,
        "NeedsRevision" | "Needs Revision" => TaskStatus::NeedsRevision,
        "Complete" => TaskStatus::Complete,
        "Failed" => TaskStatus::Failed,
        "Stuck" => TaskStatus::Stuck,
        _ => anyhow::bail!("Unknown task status: {}", status_str),
    };

    Ok(Task {
        id: TaskId::from_string(&row.try_get::<String, _>("id")?)?,
        project_id: ProjectId::from_string(&row.try_get::<String, _>("project_id")?)?,
        title: row.try_get("title")?,
        description: row.try_get("description")?,
        assigned_worker: row.try_get::<Option<String>, _>("assigned_worker")?
            .and_then(|s| serde_json::from_str(&s).ok()),
        dependencies: serde_json::from_str(&row.try_get::<String, _>("dependencies")?)?,
        status,
        deliverables: serde_json::from_str(&row.try_get::<String, _>("deliverables")?)?,
        validation_notes: row.try_get("validation_notes")?,
        pm_feedback: row.try_get("pm_feedback")?,
        revision_count: row.try_get::<i64, _>("revision_count")? as u32,
        max_revisions: row.try_get::<i64, _>("max_revisions")? as u32,
        stuck_retry_count: 0, // Default for existing tasks
        max_stuck_retries: 2, // Default for existing tasks
        blocking_reason: row.try_get("blocking_reason")?,
        failure_reason: row.try_get("failure_reason")?,
        created_at: i64_to_system_time(row.try_get("created_at")?),
        assigned_at: row.try_get::<Option<i64>, _>("assigned_at")?.map(i64_to_system_time),
        started_at: row.try_get::<Option<i64>, _>("started_at")?.map(i64_to_system_time),
        completed_at: row.try_get::<Option<i64>, _>("completed_at")?.map(i64_to_system_time),
        last_status_change: i64_to_system_time(row.try_get("last_status_change")?),
    })
}

/// Convert SQLite row to Milestone
fn row_to_milestone(row: &SqliteRow) -> Result<Milestone> {
    let status_str: String = row.try_get("status")?;
    let status = match status_str.as_str() {
        "NotStarted" | "Not Started" => MilestoneStatus::NotStarted,
        "InProgress" | "In Progress" => MilestoneStatus::InProgress,
        "Complete" => MilestoneStatus::Complete,
        "Delayed" => MilestoneStatus::Delayed,
        _ => anyhow::bail!("Unknown milestone status: {}", status_str),
    };

    Ok(Milestone {
        id: MilestoneId::from_string(&row.try_get::<String, _>("id")?)?,
        project_id: ProjectId::from_string(&row.try_get::<String, _>("project_id")?)?,
        title: row.try_get("title")?,
        description: row.try_get("description")?,
        deadline: row.try_get::<Option<i64>, _>("deadline")?.map(i64_to_system_time),
        task_ids: serde_json::from_str(&row.try_get::<String, _>("task_ids")?)?,
        status,
        created_at: i64_to_system_time(row.try_get("created_at")?),
        completed_at: row.try_get::<Option<i64>, _>("completed_at")?.map(i64_to_system_time),
    })
}
