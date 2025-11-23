//! # START OF FILE hainet-persona/src/projects/manager.rs
//! Project Manager - Central orchestration for project lifecycle
//! 
//! The ProjectManager coordinates all project operations including creation,
//! agent hibernation, task management, and SQLite persistence.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use std::fs::File;
use tokio::sync::RwLock;
use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use flate2::{Compression, write::GzEncoder};
use tempfile::TempDir;
use chrono::{DateTime, Utc};

use super::project::{Project, ProjectId, ProjectStatus, ProjectExportData, TaskExportData, MilestoneExportData, ExportMetadata, ImportResult};
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

/// Task info for UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskInfo {
    pub id: TaskId,
    pub title: String,
    pub status: TaskStatus,
}

/// Project info for UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub id: ProjectId,
    pub title: String,
    pub status: ProjectStatus,
    pub tasks: Vec<TaskInfo>,
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

    // ========== File Management ==========

    /// Get the sandbox directory path for a project
    /// Matches the MCP server's path sanitization logic
    fn get_project_sandbox_path(project_title: &str) -> PathBuf {
        // Sanitize project name (match MCP server logic in hainet-files)
        let sanitized = project_title
            .replace(' ', "_")
            .replace('/', "_")
            .replace('\\', "_");
        
        let current = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        tracing::info!("DEBUG: Current dir: {}", current.display());

        let base_path = if let Ok(path) = std::env::var("HAINET_FILES_BASE_PATH") {
            tracing::info!("DEBUG: Using HAINET_FILES_BASE_PATH: {}", path);
            PathBuf::from(path)
        } else {
            // Walk up the directory tree looking for "sandbox"
            let mut candidate = current.clone();
            let mut found = false;
            
            // Try up to 5 levels up
            for _ in 0..5 {
                if candidate.join("sandbox").exists() {
                    tracing::info!("DEBUG: Found sandbox at: {}", candidate.display());
                    found = true;
                    break;
                }
                
                if let Some(parent) = candidate.parent() {
                    candidate = parent.to_path_buf();
                } else {
                    break;
                }
            }
            
            if found {
                candidate
            } else {
                tracing::warn!("DEBUG: Sandbox not found in any parent, defaulting to current");
                current
            }
        };
        
        let final_path = base_path
            .join("sandbox")
            .join("projects")
            .join(sanitized);
            
        tracing::info!("DEBUG: Calculated sandbox path: {}", final_path.display());
        final_path
    }

    /// Delete sandbox directory for a project
    async fn delete_project_sandbox(project_title: &str) -> Result<()> {
        let sandbox_path = Self::get_project_sandbox_path(project_title);
        tracing::info!("DEBUG: Attempting to delete sandbox at: {}", sandbox_path.display());
        
        if sandbox_path.exists() {
            tokio::fs::remove_dir_all(&sandbox_path).await?;
            tracing::info!("🗑️  Deleted project sandbox: {}", sandbox_path.display());
        } else {
            tracing::warn!("DEBUG: Sandbox directory NOT FOUND at: {}", sandbox_path.display());
            // List contents of parent directory to see what's there
            if let Some(parent) = sandbox_path.parent() {
                if parent.exists() {
                    tracing::info!("DEBUG: Parent directory {} exists. Contents:", parent.display());
                    let mut entries = tokio::fs::read_dir(parent).await?;
                    while let Some(entry) = entries.next_entry().await? {
                        tracing::info!("  - {:?}", entry.file_name());
                    }
                } else {
                    tracing::warn!("DEBUG: Parent directory {} does NOT exist", parent.display());
                }
            }
        }
        
        Ok(())
    }

    /// Delete a project (soft delete, triggers agent cleanup and file removal)
    pub async fn delete_project(&self, project_id: &ProjectId) -> Result<()> {
        // Get project title before deletion (needed for sandbox path)
        let project_title = {
            let active = self.active_projects.read().await;
            active.get(project_id)
                .map(|p| p.title.clone())
                .ok_or_else(|| anyhow::anyhow!("Project not found: {}", project_id))?
        };
        
        // Delete sandbox files FIRST (before database deletion)
        Self::delete_project_sandbox(&project_title).await?;
        
        // Remove from active projects
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

    /// Delete all active projects (for bulk cleanup)
    pub async fn delete_all_active_projects(&self) -> Result<usize> {
        let project_ids: Vec<ProjectId> = {
            let active = self.active_projects.read().await;
            active.keys().cloned().collect()
        };
        
        let count = project_ids.len();
        for project_id in project_ids {
            self.delete_project(&project_id).await?;
        }
        
        tracing::info!("Deleted {} projects", count);
        Ok(count)
    }

    // ========== Export/Import Functionality ==========

    /// Count files recursively in a directory
    async fn count_files_recursive(dir: &Path) -> Result<usize> {
        if !dir.exists() {
            return Ok(0);
        }

        let mut count = 0;
        let mut entries = tokio::fs::read_dir(dir).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                count += Box::pin(Self::count_files_recursive(&entry.path())).await?;
            } else {
                count += 1;
            }
        }
        
        Ok(count)
    }

    /// Get full project data including all tasks and milestones
    async fn get_project_full_data(&self, project_id: &ProjectId) -> Result<ProjectExportData> {
        let active = self.active_projects.read().await;
        let project = active.get(project_id)
            .ok_or_else(|| anyhow::anyhow!("Project not found: {}", project_id))?;
        
        // Get all tasks
        let mut task_exports = Vec::new();
        for task_id in &project.task_ids {
            match self.get_task(task_id).await {
                Ok(task) => {
                    task_exports.push(TaskExportData {
                        id: task.id.to_string(),
                        title: task.title.clone(),
                        description: task.description.clone(),
                        status: format!("{:?}", task.status),
                        dependencies: task.dependencies.iter().map(|id| id.to_string()).collect(),
                        assigned_to: task.assigned_worker.map(|id| id.to_string()),
                        deliverables: task.deliverables.clone(),
                        pm_feedback: task.pm_feedback.clone(),
                    });
                }
                Err(e) => {
                    tracing::warn!("Failed to load task {} for export: {}", task_id, e);
                }
            }
        }
        
        // Get all milestones
        let mut milestone_exports = Vec::new();
        for milestone_id in &project.milestone_ids {
            match self.storage.get_milestone(milestone_id).await {
                Ok(Some(milestone)) => {
                    milestone_exports.push(MilestoneExportData {
                        id: milestone.id.to_string(),
                        title: milestone.title.clone(),
                        description: milestone.description.clone(),
                        deadline: milestone.deadline.and_then(|st| {
                            st.duration_since(SystemTime::UNIX_EPOCH).ok().map(|d| d.as_secs() as i64)
                        }),
                        completed: milestone.completed_at.is_some(),
                    });
                }
                Ok(None) => {
                    tracing::warn!("Milestone {} not found in database", milestone_id);
                }
                Err(e) => {
                    tracing::warn!("Failed to load milestone {} for export: {}", milestone_id, e);
                }
            }
        }
        
        Ok(ProjectExportData {
            id: project.id.to_string(),
            title: project.title.clone(),
            overview: project.overview.clone(),
            status: project.status.clone(),
            created_at: project.created_at.duration_since(SystemTime::UNIX_EPOCH)?.as_secs() as i64,
            tasks: task_exports,
            milestones: milestone_exports,
        })
    }

    /// Export a project to a tar.gz archive
    pub async fn export_project(&self, project_id: &ProjectId, export_path: &Path) -> Result<ExportMetadata> {
        tracing::info!("📦 Exporting project {} to {}", project_id, export_path.display());
        
        // 1. Get project data
        let project_data = self.get_project_full_data(project_id).await?;
        let project_title = project_data.title.clone();
        
        // 2. Get sandbox path
        let sandbox_path = Self::get_project_sandbox_path(&project_title);
        let file_count = Self::count_files_recursive(&sandbox_path).await?;
        
        // 3. Create tar.gz archive
        let tar_file = File::create(export_path)?;
        let enc = GzEncoder::new(tar_file, Compression::default());
        let mut tar = tar::Builder::new(enc);
        
        // 4. Add project metadata JSON
        let metadata_json = serde_json::to_string_pretty(&project_data)?;
        let metadata_bytes = metadata_json.as_bytes();
        let mut header = tar::Header::new_gnu();
        header.set_path("project.json")?;
        header.set_size(metadata_bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append(&header, metadata_bytes)?;
        
        // 5. Add sandbox files (if exists)
        if sandbox_path.exists() {
            tar.append_dir_all("files", &sandbox_path)?;
        }
        
        // 6. Finalize archive
        tar.finish()?;
        
        // 7. Get file size
        let total_size = std::fs::metadata(export_path)?.len();
        
        tracing::info!("✅ Exported project: {} files, {} bytes", file_count, total_size);
        
        Ok(ExportMetadata {
            project_id: project_id.to_string(),
            project_title,
            export_date: Utc::now().to_rfc3339(),
            file_count,
            total_size,
        })
    }

    /// Recursively copy directory contents
    async fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
        tokio::fs::create_dir_all(dst).await?;
        
        let mut entries = tokio::fs::read_dir(src).await?;
        while let Some(entry) = entries.next_entry().await? {
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());
            
            if entry.file_type().await?.is_dir() {
                Box::pin(Self::copy_dir_recursive(&src_path, &dst_path)).await?;
            } else {
                tokio::fs::copy(&src_path, &dst_path).await?;
            }
        }
        
        Ok(())
    }

    /// Find a project by title (for conflict detection during import)
    async fn find_project_by_title(&self, title: &str) -> Result<Option<ProjectId>> {
        let active = self.active_projects.read().await;
        for (id, project) in active.iter() {
            if project.title == title {
                return Ok(Some(id.clone()));
            }
        }
        Ok(None)
    }

    /// Import a project from a tar.gz archive
    pub async fn import_project(&self, import_path: &Path) -> Result<ImportResult> {
        tracing::info!("📥 Importing project from {}", import_path.display());
        
        // 1. Extract archive to temp directory
        let temp_dir = TempDir::new()?;
        let tar_gz = File::open(import_path)?;
        let tar = flate2::read::GzDecoder::new(tar_gz);
        let mut archive = tar::Archive::new(tar);
        archive.unpack(temp_dir.path())?;
        
        // 2. Read project metadata
        let metadata_path = temp_dir.path().join("project.json");
        let metadata_json = tokio::fs::read_to_string(&metadata_path).await?;
        let project_data: ProjectExportData = serde_json::from_str(&metadata_json)?;
        
        tracing::info!("📄 Importing project: {}", project_data.title);
        
        // 3. Check for title conflicts and auto-rename if needed
        let original_title = project_data.title.clone();
        let final_title = if self.find_project_by_title(&original_title).await?.is_some() {
            let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
            let new_title = format!("{} (imported {})", original_title, timestamp);
            tracing::info!("⚠️  Title conflict detected, renaming to: {}", new_title);
            new_title
        } else {
            original_title.clone()
        };
        
        // 4. Create new project in database
        let new_project_id = ProjectId::new();
        let mut project = Project::new(final_title.clone(), project_data.overview);
        project.id = new_project_id.clone();
        project.status = project_data.status;
        
        self.storage.create_project(&project).await?;
        
        // 5. Import tasks
        for task_data in &project_data.tasks {
            let task_status = match task_data.status.as_str() {
                "Unassigned" => TaskStatus::Unassigned,
                "Assigned" => TaskStatus::Assigned,
                "InProgress" => TaskStatus::InProgress,
                "Blocked" => TaskStatus::Blocked,
                "UnderReview" => TaskStatus::UnderReview,
                "NeedsRevision" => TaskStatus::NeedsRevision,
                "Complete" => TaskStatus::Complete,
                "Failed" => TaskStatus::Failed,
                _ => TaskStatus::Unassigned, // Default fallback
            };
            
            let mut task = Task::new(
                new_project_id.clone(),
                task_data.title.clone(),
                task_data.description.clone(),
            );
            task.status = task_status;
            task.deliverables = task_data.deliverables.clone();
            task.pm_feedback = task_data.pm_feedback.clone();
            
            self.storage.create_task(&task).await?;
            project.task_ids.push(task.id.clone());
        }
        
        // 6. Import milestones
        for milestone_data in &project_data.milestones {
            let deadline = milestone_data.deadline.map(|timestamp| {
                SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(timestamp as u64)
            });
            
            let mut milestone = Milestone::new(
                new_project_id.clone(),
                milestone_data.title.clone(),
                milestone_data.description.clone(),
                deadline,
            );
            
            if milestone_data.completed {
                milestone.completed_at = Some(SystemTime::now());
            }
            
            self.storage.create_milestone(&milestone).await?;
            project.milestone_ids.push(milestone.id.clone());
        }
        
        // Update project with task and milestone IDs
        self.storage.update_project(&project).await?;
        
        // 7. Restore sandbox files
        let files_dir = temp_dir.path().join("files");
        let file_count = if files_dir.exists() {
            let target_sandbox = Self::get_project_sandbox_path(&final_title);
            tokio::fs::create_dir_all(&target_sandbox).await?;
            Self::copy_dir_recursive(&files_dir, &target_sandbox).await?;
            Self::count_files_recursive(&target_sandbox).await?
        } else {
            0
        };
        
        // 8. Add to active projects
        self.active_projects.write().await.insert(new_project_id.clone(), project);
        
        tracing::info!("✅ Imported project: {} tasks, {} files", project_data.tasks.len(), file_count);
        
        Ok(ImportResult {
            project_id: new_project_id.to_string(),
            original_title,
            imported_title: final_title,
            task_count: project_data.tasks.len(),
            file_count,
        })
    }

    /// Pause a project
    pub async fn pause_project(&self, project_id: &ProjectId) -> Result<()> {
        let mut active = self.active_projects.write().await;
        
        if let Some(project) = active.get_mut(project_id) {
            project.pause()?;
            self.storage.update_project(project).await?;
            tracing::info!("Paused project: {}", project_id);
            Ok(())
        } else {
            anyhow::bail!("Project not found: {}", project_id)
        }
    }

    /// Resume a paused project
    pub async fn resume_project(&self, project_id: &ProjectId) -> Result<()> {
        let mut active = self.active_projects.write().await;
        
        if let Some(project) = active.get_mut(project_id) {
            project.resume()?;
            self.storage.update_project(project).await?;
            tracing::info!("Resumed project: {}", project_id);
            Ok(())
        } else {
            anyhow::bail!("Project not found: {}", project_id)
        }
    }

    /// Stop/cancel a project
    pub async fn stop_project(&self, project_id: &ProjectId) -> Result<()> {
        let mut active = self.active_projects.write().await;
        
        if let Some(project) = active.get_mut(project_id) {
            project.cancel()?;
            self.storage.update_project(project).await?;
            
            // Cleanup hibernated agents
            drop(active); // Release lock before calling cleanup
            self.cleanup_hibernated_agents(project_id).await?;
            
            tracing::info!("Stopped/cancelled project: {}", project_id);
            Ok(())
        } else {
            anyhow::bail!("Project not found: {}", project_id)
        }
    }

    /// Rename a project
    pub async fn rename_project(&self, project_id: &ProjectId, new_title: String) -> Result<()> {
        let mut active = self.active_projects.write().await;
        
        if let Some(project) = active.get_mut(project_id) {
            let old_title = project.title.clone();
            project.title = new_title.clone();
            self.storage.update_project(project).await?;
            tracing::info!("Renamed project {} from '{}' to '{}'", project_id, old_title, new_title);
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

    /// Start a task (transition to InProgress)
    pub async fn start_task(&self, task_id: &TaskId) -> Result<()> {
        if let Some(mut task) = self.storage.get_task(task_id).await? {
            task.start()?;
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

    /// Reset task for revision (transition back to InProgress)
    pub async fn reset_task_for_revision(&self, task_id: &TaskId) -> Result<()> {
        if let Some(mut task) = self.storage.get_task(task_id).await? {
            task.reset_for_revision()?;
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
        let mut task = self.get_task(task_id).await?;
        task.fail(reason)?;
        self.storage.update_task(&task).await?;
        
        tracing::info!(
            "Task {} failed: {}",
            task_id,
            task.failure_reason.as_ref().unwrap_or(&"Unknown reason".to_string())
        );
        
        Ok(())
    }
    
    /// Mark a task as stuck (requires manual intervention)
    pub async fn mark_task_stuck(&self, task_id: &TaskId, reason: String) -> Result<()> {
        let mut task = self.get_task(task_id).await?;
        task.mark_as_stuck(reason)?;
        self.storage.update_task(&task).await?;
        
        tracing::warn!(
            "Task {} marked as stuck: {}",
            task_id,
            task.failure_reason.as_ref().unwrap_or(&"Unknown reason".to_string())
        );
        
        Ok(())
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

    /// Get recent projects (including completed ones) from storage
    pub async fn get_recent_projects(&self, limit: usize) -> Result<Vec<Project>> {
        // storage.list_active_projects returns all non-deleted projects ordered by created_at DESC
        let all_projects = self.storage.list_active_projects().await?;
        Ok(all_projects.into_iter().take(limit).collect())
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

    /// Get all active projects with their unfinished tasks
    pub async fn get_active_projects_with_tasks(&self) -> Result<Vec<ProjectInfo>> {
        let projects = self.list_active_projects().await?;
        let mut result = Vec::new();
        
        for project in projects {
            let tasks = self.get_project_tasks(&project.id).await?;
            let unfinished_tasks = tasks.into_iter()
                .filter(|t| !t.status.is_terminal())
                .map(|t| TaskInfo {
                    id: t.id,
                    title: t.title,
                    status: t.status,
                })
                .collect();
                
            result.push(ProjectInfo {
                id: project.id,
                title: project.title,
                status: project.status,
                tasks: unfinished_tasks,
            });
        }
        Ok(result)
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
