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
    /// Task validated and complete
    Complete,
    /// Task failed with errors
    Failed,
}

impl TaskStatus {
    /// Check if task is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(self, TaskStatus::Complete | TaskStatus::Failed)
    }

    /// Check if task is active (assigned or in progress)
    pub fn is_active(&self) -> bool {
        matches!(self, 
            TaskStatus::Assigned | 
            TaskStatus::InProgress | 
            TaskStatus::UnderReview
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
            TaskStatus::Complete => write!(f, "Complete"),
            TaskStatus::Failed => write!(f, "Failed"),
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
}

impl Task {
    /// Create a new task
    pub fn new(
        project_id: ProjectId,
        title: String,
        description: String,
    ) -> Self {
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
            blocking_reason: None,
            failure_reason: None,
            created_at: SystemTime::now(),
            assigned_at: None,
            started_at: None,
            completed_at: None,
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
        
        Ok(())
    }

    /// Worker starts working on the task
    pub fn start(&mut self) -> Result<()> {
        if self.status != TaskStatus::Assigned {
            anyhow::bail!("Can only start assigned tasks");
        }

        self.status = TaskStatus::InProgress;
        self.started_at = Some(SystemTime::now());
        
        Ok(())
    }

    /// Block the task with a reason
    pub fn block(&mut self, reason: String) -> Result<()> {
        if !self.status.is_active() {
            anyhow::bail!("Can only block active tasks");
        }

        self.status = TaskStatus::Blocked;
        self.blocking_reason = Some(reason);
        
        Ok(())
    }

    /// Unblock a blocked task
    pub fn unblock(&mut self) -> Result<()> {
        if self.status != TaskStatus::Blocked {
            anyhow::bail!("Can only unblock blocked tasks");
        }

        self.status = TaskStatus::InProgress;
        self.blocking_reason = None;
        
        Ok(())
    }

    /// Submit task for PM review
    pub fn submit_for_review(&mut self, deliverables: Vec<String>) -> Result<()> {
        if self.status != TaskStatus::InProgress {
            anyhow::bail!("Can only submit in-progress tasks for review");
        }

        self.deliverables = deliverables;
        self.status = TaskStatus::UnderReview;
        
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
        
        Ok(())
    }

    /// Calculate task duration (if completed)
    pub fn duration(&self) -> Option<std::time::Duration> {
        match (self.started_at, self.completed_at) {
            (Some(start), Some(end)) => end.duration_since(start).ok(),
            _ => None,
        }
    }
}
