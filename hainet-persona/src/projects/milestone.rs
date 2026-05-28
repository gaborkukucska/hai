//! # START OF FILE hainet-persona/src/projects/milestone.rs
//! Milestone Tracking for Projects
//! 
//! Defines the Milestone structure for tracking project progress. Milestones group
//! related tasks and provide high-level progress indicators.

use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use uuid::Uuid;
use anyhow::Result;

use super::project::ProjectId;
use super::task::{Task, TaskId, TaskStatus};

/// Unique identifier for a milestone
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MilestoneId(Uuid);

impl MilestoneId {
    /// Create a new random milestone ID
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

    /// Parse from string
    pub fn from_string(s: &str) -> Result<Self> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl Default for MilestoneId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for MilestoneId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Milestone status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MilestoneStatus {
    /// No tasks started yet
    NotStarted,
    /// Some tasks are in progress
    InProgress,
    /// All tasks completed
    Complete,
    /// Milestone is past deadline
    Delayed,
}

impl std::fmt::Display for MilestoneStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MilestoneStatus::NotStarted => write!(f, "Not Started"),
            MilestoneStatus::InProgress => write!(f, "In Progress"),
            MilestoneStatus::Complete => write!(f, "Complete"),
            MilestoneStatus::Delayed => write!(f, "Delayed"),
        }
    }
}

/// Milestone entity for grouping related tasks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Milestone {
    /// Unique milestone identifier
    pub id: MilestoneId,
    
    /// Parent project ID
    pub project_id: ProjectId,
    
    /// Human-readable milestone title
    pub title: String,
    
    /// Detailed milestone description
    pub description: String,
    
    /// Optional deadline
    pub deadline: Option<SystemTime>,
    
    /// Task IDs associated with this milestone
    pub task_ids: Vec<TaskId>,
    
    /// Current status
    pub status: MilestoneStatus,
    
    /// Timestamp when milestone was created
    pub created_at: SystemTime,
    
    /// Timestamp when milestone was completed
    pub completed_at: Option<SystemTime>,
}

impl Milestone {
    /// Create a new milestone
    pub fn new(
        project_id: ProjectId,
        title: String,
        description: String,
        deadline: Option<SystemTime>,
    ) -> Self {
        Self {
            id: MilestoneId::new(),
            project_id,
            title,
            description,
            deadline,
            task_ids: Vec::new(),
            status: MilestoneStatus::NotStarted,
            created_at: SystemTime::now(),
            completed_at: None,
        }
    }

    /// Add a task to this milestone
    pub fn add_task(&mut self, task_id: TaskId) {
        if !self.task_ids.contains(&task_id) {
            self.task_ids.push(task_id);
        }
    }

    /// Remove a task from this milestone
    pub fn remove_task(&mut self, task_id: &TaskId) {
        self.task_ids.retain(|id| id != task_id);
    }

    /// Calculate progress percentage (0.0 to 1.0) based on completed tasks
    pub fn progress(&self, tasks: &[Task]) -> f64 {
        if self.task_ids.is_empty() {
            return 0.0;
        }

        let completed_count = tasks.iter()
            .filter(|task| self.task_ids.contains(&task.id))
            .filter(|task| task.status == TaskStatus::Complete)
            .count();

        completed_count as f64 / self.task_ids.len() as f64
    }

    /// Check if all tasks in this milestone are complete
    pub fn is_complete(&self, tasks: &[Task]) -> bool {
        if self.task_ids.is_empty() {
            return false;
        }

        self.task_ids.iter().all(|task_id| {
            tasks.iter()
                .find(|task| &task.id == task_id)
                .map(|task| task.status == TaskStatus::Complete)
                .unwrap_or(false)
        })
    }

    /// Check if any tasks in this milestone are in progress
    pub fn has_active_tasks(&self, tasks: &[Task]) -> bool {
        self.task_ids.iter().any(|task_id| {
            tasks.iter()
                .find(|task| &task.id == task_id)
                .map(|task| task.status.is_active())
                .unwrap_or(false)
        })
    }

    /// Update milestone status based on task states
    pub fn update_status(&mut self, tasks: &[Task]) -> Result<()> {
        if self.is_complete(tasks) {
            self.status = MilestoneStatus::Complete;
            if self.completed_at.is_none() {
                self.completed_at = Some(SystemTime::now());
            }
        } else if self.has_active_tasks(tasks) {
            self.status = MilestoneStatus::InProgress;
        } else {
            self.status = MilestoneStatus::NotStarted;
        }

        // Check if past deadline
        if let Some(deadline) = self.deadline {
            if SystemTime::now() > deadline && self.status != MilestoneStatus::Complete {
                self.status = MilestoneStatus::Delayed;
            }
        }

        Ok(())
    }

    /// Check if milestone is past its deadline
    pub fn is_delayed(&self) -> bool {
        if let Some(deadline) = self.deadline {
            SystemTime::now() > deadline && self.status != MilestoneStatus::Complete
        } else {
            false
        }
    }

    /// Get time remaining until deadline (if any)
    pub fn time_until_deadline(&self) -> Option<std::time::Duration> {
        self.deadline.and_then(|deadline| {
            deadline.duration_since(SystemTime::now()).ok()
        })
    }

    /// Get task count statistics
    pub fn task_stats(&self, tasks: &[Task]) -> MilestoneTaskStats {
        let milestone_tasks: Vec<&Task> = tasks.iter()
            .filter(|task| self.task_ids.contains(&task.id))
            .collect();

        MilestoneTaskStats {
            total: milestone_tasks.len(),
            completed: milestone_tasks.iter()
                .filter(|task| task.status == TaskStatus::Complete)
                .count(),
            in_progress: milestone_tasks.iter()
                .filter(|task| task.status == TaskStatus::InProgress)
                .count(),
            blocked: milestone_tasks.iter()
                .filter(|task| task.status == TaskStatus::Blocked)
                .count(),
            failed: milestone_tasks.iter()
                .filter(|task| task.status == TaskStatus::Failed)
                .count(),
        }
    }
}

/// Task statistics for a milestone
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MilestoneTaskStats {
    pub total: usize,
    pub completed: usize,
    pub in_progress: usize,
    pub blocked: usize,
    pub failed: usize,
}

impl MilestoneTaskStats {
    /// Calculate completion percentage
    pub fn completion_percentage(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        (self.completed as f64 / self.total as f64) * 100.0
    }
}
