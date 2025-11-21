//! # START OF FILE hainet-persona/src/projects/mod.rs
//! Project Management Module
//! 
//! This module provides the core infrastructure for managing projects in HAI-Net's
//! project-based agentic system. Each user intent can become a discrete project with
//! dedicated PM and Worker agents.

pub mod project;
pub mod task;
pub mod milestone;
pub mod storage;
pub mod manager;
pub mod migrations;

// Re-export key types for convenience
pub use project::{Project, ProjectId, ProjectStatus, ExportMetadata, ImportResult};
pub use task::{Task, TaskId, TaskStatus};
pub use milestone::{Milestone, MilestoneId, MilestoneStatus};
pub use manager::{ProjectManager, HibernatedAgent, ProjectInfo, TaskInfo};
pub use storage::ProjectStorage;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // Verify all key types are accessible
        let _project_id = ProjectId::new();
        let _task_id = TaskId::new();
        let _milestone_id = MilestoneId::new();
    }
}
