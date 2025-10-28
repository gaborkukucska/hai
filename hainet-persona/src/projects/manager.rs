//! # START OF FILE hainet-persona/src/projects/manager.rs
//! Project Manager - Central orchestration for project lifecycle
//! 
//! The ProjectManager coordinates all project operations including creation,
//! agent hibernation, task management, and SQLite persistence.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;
use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::project::{Project, ProjectId, ProjectStatus};
use super::task::{Task, TaskId, TaskStatus};
use super::milestone::{Milestone, MilestoneId};
use super::storage::ProjectStorage;
use crate::messaging::AgentId;
use crate::prompts::types::AgentType;

/// Hibernated agent metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HibernatedAgent {
    pub agent_id: AgentId,
    pub project_id: ProjectId,
    pub agent_type: AgentType,
    pub system_prompt: String,
    pub hibernated_at: SystemTime,
}

/// Central project management coordinator
pub struct ProjectManager {
    storage: Arc<ProjectStorage>,
    active_projects: Arc<RwLock<HashMap<ProjectId, Project>>>,
    hibernated_agents: Arc<RwLock<HashMap<AgentId, HibernatedAgent>>>,
}

impl ProjectManager {
    /// Create a new ProjectManager with SQLite backend
    pub async fn new(db_path: &str) -> Result<Self> {
        // ProjectStorage::new() now handles table creation AND migrations
        let storage = Arc::new(ProjectStorage::new(db_path).await?);

        // Load active projects from database
        let projects = storage.list_active_projects().await?;
        let mut active_projects_map = HashMap::new();
        for project in projects {
            active_projects_map.insert(project.id.clone(), project);
        }

        Ok(Self {
            storage,
            active_projects: Arc::new(RwLock::new(active_projects_map)),
            hibernated_agents: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    // ========== Project Lifecycle ==========

    /// Create a new project
    pub async fn create_project(
        &self,
        title: String,
        overview: String,
        initial_tasks: Vec<String>,
    ) -> Result<ProjectId> {
        let mut project = Project::new(title, overview);

        // Store project FIRST (so foreign key constraint is satisfied)
        self.storage.create_project(&project).await?;

        // Create initial tasks AFTER project exists
        for (idx, task_title) in initial_tasks.iter().enumerate() {
            let task = Task::new(
                project.id.clone(),
                task_title.clone(),
                format!("Task {} description", idx + 1),
            );
            
            // Store task (FK constraint now satisfied)
            self.storage.create_task(&task).await?;
            
            // Add to project
            project.add_task(task.id.clone());
        }

        // Update project with task IDs
        self.storage.update_project(&project).await?;

        // Add to active projects
        let project_id = project.id.clone();
        self.active_projects.write().await.insert(project_id.clone(), project);

        Ok(project_id)
    }

    /// Assign a PM agent to a project
    pub async fn assign_pm(&self, project_id: &ProjectId, pm_id: AgentId) -> Result<()> {
        let mut active = self.active_projects.write().await;
        
        if let Some(project) = active.get_mut(project_id) {
            project.assign_pm(pm_id)?;
            self.storage.update_project(project).await?;
            Ok(())
        } else {
            anyhow::bail!("Project not found: {}", project_id)
        }
    }

    /// Complete a project (triggers agent hibernation)
    pub async fn complete_project(&self, project_id: &ProjectId) -> Result<()> {
        let mut active = self.active_projects.write().await;
        
        if let Some(project) = active.get_mut(project_id) {
            project.complete()?;
            self.storage.update_project(project).await?;
            
            // Hibernate PM and worker agents
            if let Some(pm_id) = &project.pm_agent_id {
                // PM agent hibernation will be triggered externally
                tracing::info!("Project {} complete, PM agent {:?} should hibernate", project_id, pm_id);
            }
            
            for worker_id in &project.worker_agent_ids {
                tracing::info!("Project {} complete, worker agent {:?} should hibernate", project_id, worker_id);
            }
            
            Ok(())
        } else {
            anyhow::bail!("Project not found: {}", project_id)
        }
    }

    /// Delete a project (soft delete, triggers agent cleanup)
    pub async fn delete_project(&self, project_id: &ProjectId) -> Result<()> {
        let mut active = self.active_projects.write().await;
        
        if let Some(_project) = active.remove(project_id) {
            // Soft delete in database
            self.storage.delete_project(project_id).await?;
            
            // Cleanup hibernated agents for this project
            self.cleanup_hibernated_agents(project_id).await?;
            
            Ok(())
        } else {
            anyhow::bail!("Project not found: {}", project_id)
        }
    }

    // ========== Agent Hibernation ==========

    /// Hibernate an agent when project completes
    pub async fn hibernate_agent(
        &self,
        agent_id: AgentId,
        project_id: ProjectId,
        agent_type: AgentType,
        system_prompt: String,
    ) -> Result<()> {
        let hibernated = HibernatedAgent {
            agent_id: agent_id.clone(),
            project_id,
            agent_type,
            system_prompt,
            hibernated_at: SystemTime::now(),
        };

        self.hibernated_agents.write().await.insert(agent_id, hibernated);
        
        Ok(())
    }

    /// Wake a hibernated agent (returns system prompt)
    pub async fn wake_agent(&self, agent_id: &AgentId) -> Result<String> {
        let mut hibernated = self.hibernated_agents.write().await;
        
        if let Some(agent) = hibernated.remove(agent_id) {
            Ok(agent.system_prompt)
        } else {
            anyhow::bail!("Agent not found in hibernation: {:?}", agent_id)
        }
    }

    /// Cleanup all hibernated agents for a deleted project
    pub async fn cleanup_hibernated_agents(&self, project_id: &ProjectId) -> Result<()> {
        let mut hibernated = self.hibernated_agents.write().await;
        
        hibernated.retain(|_, agent| &agent.project_id != project_id);
        
        Ok(())
    }

    /// Get all hibernated agents for a project
    pub async fn get_project_hibernated_agents(&self, project_id: &ProjectId) -> Vec<HibernatedAgent> {
        let hibernated = self.hibernated_agents.read().await;
        
        hibernated.values()
            .filter(|agent| &agent.project_id == project_id)
            .cloned()
            .collect()
    }

    // ========== Task Management ==========

    /// Create a new task for a project
    pub async fn create_task(
        &self,
        project_id: &ProjectId,
        title: String,
        description: String,
    ) -> Result<TaskId> {
        let task = Task::new(project_id.clone(), title, description);
        let task_id = task.id.clone();

        // Store in database
        self.storage.create_task(&task).await?;

        // Add to project
        let mut active = self.active_projects.write().await;
        if let Some(project) = active.get_mut(project_id) {
            project.add_task(task_id.clone());
            self.storage.update_project(project).await?;
        }

        Ok(task_id)
    }

    /// Assign a task to a worker agent
    pub async fn assign_task(&self, task_id: &TaskId, worker_id: AgentId) -> Result<()> {
        if let Some(mut task) = self.storage.get_task(task_id).await? {
            task.assign_to(worker_id)?;
            self.storage.update_task(&task).await?;
            Ok(())
        } else {
            anyhow::bail!("Task not found: {}", task_id)
        }
    }

    /// Complete a task
    pub async fn complete_task(&self, task_id: &TaskId, deliverables: Vec<String>) -> Result<()> {
        if let Some(mut task) = self.storage.get_task(task_id).await? {
            task.submit_for_review(deliverables)?;
            self.storage.update_task(&task).await?;
            Ok(())
        } else {
            anyhow::bail!("Task not found: {}", task_id)
        }
    }

    /// Approve a task (PM validation)
    pub async fn approve_task(&self, task_id: &TaskId, notes: String) -> Result<()> {
        if let Some(mut task) = self.storage.get_task(task_id).await? {
            task.approve(notes)?;
            self.storage.update_task(&task).await?;
            Ok(())
        } else {
            anyhow::bail!("Task not found: {}", task_id)
        }
    }

    /// Reject a task (PM sends back for rework)
    pub async fn reject_task(&self, task_id: &TaskId, reason: String) -> Result<()> {
        if let Some(mut task) = self.storage.get_task(task_id).await? {
            task.reject(reason)?;
            self.storage.update_task(&task).await?;
            Ok(())
        } else {
            anyhow::bail!("Task not found: {}", task_id)
        }
    }

    /// Request task revision with PM feedback
    pub async fn request_revision(&self, task_id: &TaskId, feedback: String) -> Result<()> {
        if let Some(mut task) = self.storage.get_task(task_id).await? {
            task.request_revision(feedback)?;
            self.storage.update_task(&task).await?;
            Ok(())
        } else {
            anyhow::bail!("Task not found: {}", task_id)
        }
    }

    /// Get current task status (for worker polling)
    pub async fn get_task_status(&self, task_id: &TaskId) -> Result<TaskStatus> {
        if let Some(task) = self.storage.get_task(task_id).await? {
            Ok(task.status)
        } else {
            anyhow::bail!("Task not found: {}", task_id)
        }
    }

    /// Get a task by ID
    pub async fn get_task(&self, task_id: &TaskId) -> Result<Task> {
        self.storage.get_task(task_id).await?
            .ok_or_else(|| anyhow::anyhow!("Task not found: {}", task_id))
    }

    /// Fail task with reason
    pub async fn fail_task(&self, task_id: &TaskId, reason: String) -> Result<()> {
        if let Some(mut task) = self.storage.get_task(task_id).await? {
            task.fail(reason)?;
            self.storage.update_task(&task).await?;
            Ok(())
        } else {
            anyhow::bail!("Task not found: {}", task_id)
        }
    }

    // ========== Milestone Management ==========

    /// Create a new milestone for a project
    pub async fn create_milestone(
        &self,
        project_id: &ProjectId,
        title: String,
        description: String,
        deadline: Option<SystemTime>,
    ) -> Result<MilestoneId> {
        let milestone = Milestone::new(project_id.clone(), title, description, deadline);
        let milestone_id = milestone.id.clone();

        // Store in database
        self.storage.create_milestone(&milestone).await?;

        // Add to project
        let mut active = self.active_projects.write().await;
        if let Some(project) = active.get_mut(project_id) {
            project.add_milestone(milestone_id.clone());
            self.storage.update_project(project).await?;
        }

        Ok(milestone_id)
    }

    /// Update milestone status based on task progress
    pub async fn update_milestone_status(&self, milestone_id: &MilestoneId) -> Result<()> {
        if let Some(mut milestone) = self.storage.get_milestone(milestone_id).await? {
            let tasks = self.storage.list_project_tasks(&milestone.project_id).await?;
            milestone.update_status(&tasks)?;
            self.storage.update_milestone(&milestone).await?;
            Ok(())
        } else {
            anyhow::bail!("Milestone not found: {}", milestone_id)
        }
    }

    // ========== Queries ==========

    /// Get a project by ID
    pub async fn get_project(&self, id: &ProjectId) -> Result<Option<Project>> {
        // Check active projects first
        if let Some(project) = self.active_projects.read().await.get(id) {
            return Ok(Some(project.clone()));
        }

        // Fall back to database
        self.storage.get_project(id).await
    }

    /// List all active projects
    pub async fn list_active_projects(&self) -> Result<Vec<Project>> {
        Ok(self.active_projects.read().await.values().cloned().collect())
    }

    /// Get all tasks for a project
    pub async fn get_project_tasks(&self, project_id: &ProjectId) -> Result<Vec<Task>> {
        self.storage.list_project_tasks(project_id).await
    }

    /// Get all milestones for a project
    pub async fn get_project_milestones(&self, project_id: &ProjectId) -> Result<Vec<Milestone>> {
        self.storage.list_project_milestones(project_id).await
    }

    /// Get project progress summary
    pub async fn get_project_progress(&self, project_id: &ProjectId) -> Result<ProjectProgress> {
        let project = self.get_project(project_id).await?
            .ok_or_else(|| anyhow::anyhow!("Project not found"))?;
        
        let tasks = self.get_project_tasks(project_id).await?;
        let milestones = self.get_project_milestones(project_id).await?;

        let completed_tasks = tasks.iter()
            .filter(|t| t.status == TaskStatus::Complete)
            .count();
        
        let completed_milestones = milestones.iter()
            .filter(|m| m.is_complete(&tasks))
            .count();

        let status = project.status.clone();
        let progress_percentage = project.progress(completed_tasks);
        
        Ok(ProjectProgress {
            project_id: project_id.clone(),
            title: project.title,
            status,
            total_tasks: tasks.len(),
            completed_tasks,
            total_milestones: milestones.len(),
            completed_milestones,
            progress_percentage,
        })
    }
}

/// Project progress summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectProgress {
    pub project_id: ProjectId,
    pub title: String,
    pub status: ProjectStatus,
    pub total_tasks: usize,
    pub completed_tasks: usize,
    pub total_milestones: usize,
    pub completed_milestones: usize,
    pub progress_percentage: f64,
}
