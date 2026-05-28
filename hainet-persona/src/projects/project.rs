//! # START OF FILE hainet-persona/src/projects/project.rs
//! Project Entity and Lifecycle Management
//! 
//! Defines the Project structure and its lifecycle state machine. Projects are first-class
//! entities in HAI-Net's agentic system, each with dedicated PM and Worker agents.

use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use uuid::Uuid;
use anyhow::Result;
use chrono::{DateTime, Utc};

use crate::messaging::AgentId;

/// Unique identifier for a project
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProjectId(Uuid);

impl ProjectId {
    /// Create a new random project ID
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

impl Default for ProjectId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ProjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Project lifecycle status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectStatus {
    /// Project created by Admin AI, PM not yet assigned
    Created,
    /// PM agent assigned and managing, workers executing tasks
    Active,
    /// User paused the project
    Paused,
    /// Project successfully completed, agents hibernated
    Completed,
    /// Project failed with unrecoverable error
    Failed,
    /// User cancelled the project
    Cancelled,
}

impl ProjectStatus {
    /// Check if project is in a terminal state (completed, failed, or cancelled)
    pub fn is_terminal(&self) -> bool {
        matches!(self, 
            ProjectStatus::Completed | 
            ProjectStatus::Failed | 
            ProjectStatus::Cancelled
        )
    }

    /// Check if project is active (Created or Active)
    pub fn is_active(&self) -> bool {
        matches!(self, ProjectStatus::Created | ProjectStatus::Active)
    }
}

impl std::fmt::Display for ProjectStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProjectStatus::Created => write!(f, "Created"),
            ProjectStatus::Active => write!(f, "Active"),
            ProjectStatus::Paused => write!(f, "Paused"),
            ProjectStatus::Completed => write!(f, "Completed"),
            ProjectStatus::Failed => write!(f, "Failed"),
            ProjectStatus::Cancelled => write!(f, "Cancelled"),
        }
    }
}

/// Main project entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    /// Unique project identifier
    pub id: ProjectId,
    
    /// Human-readable project title
    pub title: String,
    
    /// Detailed project overview
    pub overview: String,
    
    /// Current lifecycle status
    pub status: ProjectStatus,
    
    /// PM agent ID (hibernates when project completes)
    pub pm_agent_id: Option<AgentId>,
    
    /// Worker agent IDs (hibernate when project completes)
    pub worker_agent_ids: Vec<AgentId>,
    
    /// Milestone IDs (detailed in milestone module)
    pub milestone_ids: Vec<super::milestone::MilestoneId>,
    
    /// Task IDs (detailed in task module)
    pub task_ids: Vec<super::task::TaskId>,
    
    /// Timestamp when project was created
    pub created_at: SystemTime,
    
    /// Timestamp when project started (PM assigned)
    pub started_at: Option<SystemTime>,
    
    /// Timestamp when project completed
    pub completed_at: Option<SystemTime>,
    
    /// Timestamp when project was deleted (soft delete)
    pub deleted_at: Option<SystemTime>,
    
    /// Failure reason (if status is Failed)
    pub failure_reason: Option<String>,
}

impl Project {
    /// Create a new project
    pub fn new(title: String, overview: String) -> Self {
        Self {
            id: ProjectId::new(),
            title,
            overview,
            status: ProjectStatus::Created,
            pm_agent_id: None,
            worker_agent_ids: Vec::new(),
            milestone_ids: Vec::new(),
            task_ids: Vec::new(),
            created_at: SystemTime::now(),
            started_at: None,
            completed_at: None,
            deleted_at: None,
            failure_reason: None,
        }
    }

    /// Assign a PM agent to this project
    pub fn assign_pm(&mut self, pm_id: AgentId) -> Result<()> {
        if self.status != ProjectStatus::Created {
            anyhow::bail!("Can only assign PM to Created projects");
        }

        self.pm_agent_id = Some(pm_id);
        self.status = ProjectStatus::Active;
        self.started_at = Some(SystemTime::now());
        
        Ok(())
    }

    /// Add a worker agent to this project
    pub fn add_worker(&mut self, worker_id: AgentId) -> Result<()> {
        if !self.status.is_active() {
            anyhow::bail!("Can only add workers to active projects");
        }

        if !self.worker_agent_ids.contains(&worker_id) {
            self.worker_agent_ids.push(worker_id);
        }
        
        Ok(())
    }

    /// Remove a worker agent from this project
    pub fn remove_worker(&mut self, worker_id: &AgentId) -> Result<()> {
        self.worker_agent_ids.retain(|id| id != worker_id);
        Ok(())
    }

    /// Add a milestone to this project
    pub fn add_milestone(&mut self, milestone_id: super::milestone::MilestoneId) {
        if !self.milestone_ids.contains(&milestone_id) {
            self.milestone_ids.push(milestone_id);
        }
    }

    /// Add a task to this project
    pub fn add_task(&mut self, task_id: super::task::TaskId) {
        if !self.task_ids.contains(&task_id) {
            self.task_ids.push(task_id);
        }
    }

    /// Pause the project
    pub fn pause(&mut self) -> Result<()> {
        if self.status != ProjectStatus::Active {
            anyhow::bail!("Can only pause active projects");
        }

        self.status = ProjectStatus::Paused;
        Ok(())
    }

    /// Resume a paused project
    pub fn resume(&mut self) -> Result<()> {
        if self.status != ProjectStatus::Paused {
            anyhow::bail!("Can only resume paused projects");
        }

        self.status = ProjectStatus::Active;
        Ok(())
    }

    /// Mark project as completed (agents will be hibernated)
    pub fn complete(&mut self) -> Result<()> {
        if self.status.is_terminal() {
            anyhow::bail!("Project already in terminal state");
        }

        self.status = ProjectStatus::Completed;
        self.completed_at = Some(SystemTime::now());
        Ok(())
    }

    /// Mark project as failed
    pub fn fail(&mut self, reason: String) -> Result<()> {
        if self.status.is_terminal() {
            anyhow::bail!("Project already in terminal state");
        }

        self.status = ProjectStatus::Failed;
        self.completed_at = Some(SystemTime::now());
        self.failure_reason = Some(reason);
        Ok(())
    }

    /// Cancel the project
    pub fn cancel(&mut self) -> Result<()> {
        if self.status.is_terminal() {
            anyhow::bail!("Project already in terminal state");
        }

        self.status = ProjectStatus::Cancelled;
        self.completed_at = Some(SystemTime::now());
        Ok(())
    }

    /// Soft delete the project (sets deleted_at timestamp)
    pub fn soft_delete(&mut self) {
        self.deleted_at = Some(SystemTime::now());
    }

    /// Check if project is deleted
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }

    /// Get progress percentage (0.0 to 1.0) based on tasks
    pub fn progress(&self, completed_tasks: usize) -> f64 {
        if self.task_ids.is_empty() {
            return 0.0;
        }
        completed_tasks as f64 / self.task_ids.len() as f64
    }
}

// ========== Export/Import Data Structures ==========

/// Task data for export (simplified representation)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskExportData {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub dependencies: Vec<String>,
    pub assigned_to: Option<String>,
    pub deliverables: Vec<String>,
    pub pm_feedback: Option<String>,
}

/// Milestone data for export (simplified representation)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MilestoneExportData {
    pub id: String,
    pub title: String,
    pub description: String,
    pub deadline: Option<i64>, // Unix timestamp
    pub completed: bool,
}

/// Full project data for export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectExportData {
    pub id: String,
    pub title: String,
    pub overview: String,
    pub status: ProjectStatus,
    pub created_at: i64, // Unix timestamp
    pub tasks: Vec<TaskExportData>,
    pub milestones: Vec<MilestoneExportData>,
}

/// Metadata returned after successful export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportMetadata {
    pub project_id: String,
    pub project_title: String,
    pub export_date: String, // ISO 8601 format
    pub file_count: usize,
    pub total_size: u64,
}

/// Result returned after successful import
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub project_id: String,
    pub original_title: String,
    pub imported_title: String,
    pub task_count: usize,
    pub file_count: usize,
}
