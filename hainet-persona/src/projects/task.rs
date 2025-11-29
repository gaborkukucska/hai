//! # START OF FILE hainet-persona/src/projects/task.rs
//! Task Management for Projects
//! 
//! Defines the Task structure and its lifecycle. Tasks are assigned to worker agents
//! and tracked through their execution lifecycle.

use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use uuid::Uuid;
use anyhow::Result;

use super::project::ProjectId;
use crate::messaging::AgentId;

/// Unique identifier for a task
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(Uuid);

impl TaskId {
    /// Create a new random task ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create from existing UUID
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Get the inner UUID
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }

    /// Convert to string representation
    pub fn to_string(&self) -> String {
        self.0.to_string()
    }

    /// Parse from string
    pub fn from_string(s: &str) -> Result<Self> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Task lifecycle status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    /// Task created but not yet assigned to a worker
    Unassigned,
    /// Task assigned to a worker agent
    Assigned,
    /// Worker is actively working on the task
    InProgress,
    /// Task is blocked by dependencies or issues
    Blocked,
    /// Task completed, awaiting PM validation
    UnderReview,
    /// PM requested revisions
    NeedsRevision,
    /// Task validated and complete
    Complete,
    /// Task failed with errors
    Failed,
    /// Task stuck in InProgress, needs manual intervention
    Stuck,
}

impl TaskStatus {
    /// Check if task is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(self, TaskStatus::Complete | TaskStatus::Failed | TaskStatus::Stuck)
    }

    /// Check if task is active (assigned or in progress)
    pub fn is_active(&self) -> bool {
        matches!(self, 
            TaskStatus::Assigned | 
            TaskStatus::InProgress | 
            TaskStatus::UnderReview |
            TaskStatus::NeedsRevision
        )
    }
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Unassigned => write!(f, "Unassigned"),
            TaskStatus::Assigned => write!(f, "Assigned"),
            TaskStatus::InProgress => write!(f, "In Progress"),
            TaskStatus::Blocked => write!(f, "Blocked"),
            TaskStatus::UnderReview => write!(f, "Under Review"),
            TaskStatus::NeedsRevision => write!(f, "Needs Revision"),
            TaskStatus::Complete => write!(f, "Complete"),
            TaskStatus::Failed => write!(f, "Failed"),
            TaskStatus::Stuck => write!(f, "Stuck"),
        }
    }
}

/// Task entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Unique task identifier
    pub id: TaskId,
    
    /// Parent project ID
    pub project_id: ProjectId,
    
    /// Human-readable task title
    pub title: String,
    
    /// Detailed task description
    pub description: String,
    
    /// Assigned worker agent (if any)
    pub assigned_worker: Option<AgentId>,
    
    /// Dependencies (task IDs that must complete before this task can start)
    pub dependencies: Vec<TaskId>,
    
    /// Current status
    pub status: TaskStatus,
    
    /// Deliverables produced by worker
    pub deliverables: Vec<String>,
    
    /// PM's validation notes
    pub validation_notes: Option<String>,
    
    /// PM's feedback for revisions
    pub pm_feedback: Option<String>,
    
    /// Number of revision attempts
    pub revision_count: u32,
    
    /// Maximum allowed revisions before task fails
    pub max_revisions: u32,
    
    /// Number of times task has been retried after being stuck
    pub stuck_retry_count: u32,
    
    /// Maximum allowed stuck retries before task permanently fails
    pub max_stuck_retries: u32,
    
    /// Blocking reason (if status is Blocked)
    pub blocking_reason: Option<String>,
    
    /// Failure reason (if status is Failed)
    pub failure_reason: Option<String>,
    
    /// Timestamp when task was created
    pub created_at: SystemTime,
    
    /// Timestamp when task was assigned
    pub assigned_at: Option<SystemTime>,
    
    /// Timestamp when worker started working
    pub started_at: Option<SystemTime>,
    
    /// Timestamp when task completed
    pub completed_at: Option<SystemTime>,
    
    /// Timestamp when task status last changed
    pub last_status_change: SystemTime,
}

impl Task {
    /// Create a new task
    pub fn new(
        project_id: ProjectId,
        title: String,
        description: String,
    ) -> Self {
        let now = SystemTime::now();
        Self {
            id: TaskId::new(),
            project_id,
            title,
            description,
            assigned_worker: None,
            dependencies: Vec::new(),
            status: TaskStatus::Unassigned,
            deliverables: Vec::new(),
            validation_notes: None,
            pm_feedback: None,
            revision_count: 0,
            max_revisions: 2,
            stuck_retry_count: 0,
            max_stuck_retries: 2,
            blocking_reason: None,
            failure_reason: None,
            created_at: now,
            assigned_at: None,
            started_at: None,
            completed_at: None,
            last_status_change: now,
        }
    }

    /// Add a dependency to this task
    pub fn add_dependency(&mut self, task_id: TaskId) {
        if !self.dependencies.contains(&task_id) {
            self.dependencies.push(task_id);
        }
    }

    /// Check if all dependencies are met (all dependent tasks are complete)
    pub fn dependencies_met(&self, completed_task_ids: &[TaskId]) -> bool {
        self.dependencies.iter()
            .all(|dep_id| completed_task_ids.contains(dep_id))
    }

    /// Assign task to a worker agent
    pub fn assign_to(&mut self, worker_id: AgentId) -> Result<()> {
        if self.status != TaskStatus::Unassigned {
            anyhow::bail!("Can only assign unassigned tasks");
        }

        self.assigned_worker = Some(worker_id);
        self.status = TaskStatus::Assigned;
        self.assigned_at = Some(SystemTime::now());
        self.last_status_change = SystemTime::now();
        
        Ok(())
    }

    /// Worker starts working on the task
    pub fn start(&mut self) -> Result<()> {
        if self.status != TaskStatus::Assigned {
            anyhow::bail!("Can only start assigned tasks");
        }

        self.status = TaskStatus::InProgress;
        self.started_at = Some(SystemTime::now());
        self.last_status_change = SystemTime::now();
        
        Ok(())
    }

    /// Block the task with a reason
    pub fn block(&mut self, reason: String) -> Result<()> {
        if !self.status.is_active() {
            anyhow::bail!("Can only block active tasks");
        }

        self.status = TaskStatus::Blocked;
        self.blocking_reason = Some(reason);
        self.last_status_change = SystemTime::now();
        
        Ok(())
    }

    /// Unblock a blocked task
    pub fn unblock(&mut self) -> Result<()> {
        if self.status != TaskStatus::Blocked {
            anyhow::bail!("Can only unblock blocked tasks");
        }

        self.status = TaskStatus::InProgress;
        self.blocking_reason = None;
        self.last_status_change = SystemTime::now();
        
        Ok(())
    }

    /// Submit task for PM review
    pub fn submit_for_review(&mut self, deliverables: Vec<String>) -> Result<()> {
        if self.status != TaskStatus::InProgress {
            anyhow::bail!("Can only submit in-progress tasks for review");
        }

        self.deliverables = deliverables;
        self.status = TaskStatus::UnderReview;
        self.last_status_change = SystemTime::now();
        
        Ok(())
    }

    /// PM approves the task
    pub fn approve(&mut self, notes: String) -> Result<()> {
        if self.status != TaskStatus::UnderReview {
            anyhow::bail!("Can only approve tasks under review");
        }

        self.status = TaskStatus::Complete;
        self.validation_notes = Some(notes);
        self.completed_at = Some(SystemTime::now());
        self.last_status_change = SystemTime::now();
        
        Ok(())
    }

    /// PM rejects the task (sends back to InProgress)
    pub fn reject(&mut self, reason: String) -> Result<()> {
        if self.status != TaskStatus::UnderReview {
            anyhow::bail!("Can only reject tasks under review");
        }

        self.status = TaskStatus::InProgress;
        self.validation_notes = Some(format!("Rejected: {}", reason));
        self.deliverables.clear();
        self.last_status_change = SystemTime::now();
        
        Ok(())
    }

    /// Mark task as failed
    pub fn fail(&mut self, reason: String) -> Result<()> {
        if self.status.is_terminal() {
            anyhow::bail!("Task already in terminal state");
        }

        self.status = TaskStatus::Failed;
        self.failure_reason = Some(reason);
        self.completed_at = Some(SystemTime::now());
        self.last_status_change = SystemTime::now();
        
        Ok(())
    }

    /// Calculate task duration (if completed)
    pub fn duration(&self) -> Option<std::time::Duration> {
        match (self.started_at, self.completed_at) {
            (Some(start), Some(end)) => end.duration_since(start).ok(),
            _ => None,
        }
    }

    /// Request revision from PM
    pub fn request_revision(&mut self, feedback: String) -> Result<()> {
        if self.status != TaskStatus::UnderReview {
            anyhow::bail!("Can only request revision for tasks under review");
        }

        self.revision_count += 1;
        self.pm_feedback = Some(feedback);
        self.status = TaskStatus::NeedsRevision;
        self.last_status_change = SystemTime::now();
        
        Ok(())
    }

    /// Check if task can be retried for revision
    pub fn can_retry_revision(&self) -> bool {
        self.revision_count < self.max_revisions
    }

    /// Clear PM feedback (when worker starts revision)
    pub fn clear_feedback(&mut self) {
        self.pm_feedback = None;
    }

    /// Reset task for revision attempt
    pub fn reset_for_revision(&mut self) -> Result<()> {
        if self.status != TaskStatus::NeedsRevision {
            anyhow::bail!("Can only reset tasks that need revision");
        }

        self.status = TaskStatus::InProgress;
        self.deliverables.clear();
        self.last_status_change = SystemTime::now();
        
        Ok(())
    }

    /// Check if task is stuck in InProgress state
    /// A task is considered stuck if it's been in InProgress for more than the timeout duration
    pub fn is_stuck(&self, timeout_duration: std::time::Duration) -> bool {
        if self.status != TaskStatus::InProgress {
            return false;
        }

        if let Ok(elapsed) = SystemTime::now().duration_since(self.last_status_change) {
            elapsed > timeout_duration
        } else {
            false
        }
    }

    /// Mark task as stuck (requires manual intervention)
    pub fn mark_as_stuck(&mut self, reason: String) -> Result<()> {
        if self.status != TaskStatus::InProgress {
            anyhow::bail!("Can only mark InProgress tasks as stuck");
        }

        self.status = TaskStatus::Stuck;
        self.failure_reason = Some(reason);
        self.last_status_change = SystemTime::now();
        
        Ok(())
    }

    /// Check if task can be retried after being stuck
    pub fn can_retry_stuck(&self) -> bool {
        self.stuck_retry_count < self.max_stuck_retries
    }

    /// Reset stuck task for retry (unassign and reset to Unassigned)
    pub fn reset_stuck_for_retry(&mut self) -> Result<()> {
        if self.status != TaskStatus::Stuck {
            anyhow::bail!("Can only reset stuck tasks");
        }

        self.stuck_retry_count += 1;
        self.assigned_worker = None;
        self.status = TaskStatus::Unassigned;
        self.deliverables.clear();
        self.assigned_at = None;
        self.started_at = None;
        self.last_status_change = SystemTime::now();
        
        Ok(())
    }
}
