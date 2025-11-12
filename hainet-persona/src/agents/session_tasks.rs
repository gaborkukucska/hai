//! # START OF FILE hainet-persona/src/agents/session_tasks.rs
//! Session Task List
//! 
//! Provides workers with short-term memory of their progress within a session.
//! Tasks are stored with minimal metadata (title + status) to keep prompts lean.
//! Details are loaded on-demand when the LLM requests them.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// Session task list - tracks worker progress within a session
#[derive(Debug, Clone)]
pub struct SessionTaskList {
    /// List of tasks in this session
    tasks: Vec<SessionTask>,
    
    /// Optional detailed metadata (lazy-loaded)
    metadata_store: HashMap<String, String>,
    
    /// Maximum number of tasks to track (FIFO when exceeded)
    max_tasks: usize,
}

impl SessionTaskList {
    /// Create new session task list
    pub fn new() -> Self {
        Self {
            tasks: Vec::new(),
            metadata_store: HashMap::new(),
            max_tasks: 10, // Keep last 10 tasks
        }
    }
    
    /// Create with custom max tasks
    pub fn with_capacity(max_tasks: usize) -> Self {
        Self {
            tasks: Vec::new(),
            metadata_store: HashMap::new(),
            max_tasks,
        }
    }
    
    /// Add a new task to the session
    pub fn add_task(&mut self, title: String, metadata: Option<String>) -> usize {
        // FIFO: Remove oldest if at capacity
        if self.tasks.len() >= self.max_tasks {
            let removed = self.tasks.remove(0);
            self.metadata_store.remove(&removed.title);
        }
        
        let task = SessionTask {
            title: title.clone(),
            status: TaskStatus::Pending,
            metadata_key: metadata.is_some().then(|| title.clone()),
        };
        
        if let Some(metadata_content) = metadata {
            self.metadata_store.insert(title, metadata_content);
        }
        
        self.tasks.push(task);
        self.tasks.len() - 1
    }
    
    /// Update task status
    pub fn update_status(&mut self, title: &str, status: TaskStatus) -> Result<(), String> {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.title == title) {
            task.status = status;
            Ok(())
        } else {
            Err(format!("Task not found: {}", title))
        }
    }
    
    /// Mark task as in progress
    pub fn start_task(&mut self, title: &str) -> Result<(), String> {
        self.update_status(title, TaskStatus::InProgress)
    }
    
    /// Mark task as complete
    pub fn complete_task(&mut self, title: &str) -> Result<(), String> {
        self.update_status(title, TaskStatus::Complete)
    }
    
    /// Mark task as failed
    pub fn fail_task(&mut self, title: &str) -> Result<(), String> {
        self.update_status(title, TaskStatus::Failed)
    }
    
    /// Get task by title
    pub fn get_task(&self, title: &str) -> Option<&SessionTask> {
        self.tasks.iter().find(|t| t.title == title)
    }
    
    /// Get task details (lazy-loaded metadata)
    pub fn get_task_details(&self, title: &str) -> Option<&str> {
        self.metadata_store.get(title).map(|s| s.as_str())
    }
    
    /// Get all tasks
    pub fn tasks(&self) -> &[SessionTask] {
        &self.tasks
    }
    
    /// Get number of tasks
    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }
    
    /// Get number of pending tasks
    pub fn pending_count(&self) -> usize {
        self.tasks.iter().filter(|t| matches!(t.status, TaskStatus::Pending)).count()
    }
    
    /// Get number of in-progress tasks
    pub fn in_progress_count(&self) -> usize {
        self.tasks.iter().filter(|t| matches!(t.status, TaskStatus::InProgress)).count()
    }
    
    /// Get number of complete tasks
    pub fn complete_count(&self) -> usize {
        self.tasks.iter().filter(|t| matches!(t.status, TaskStatus::Complete)).count()
    }
    
    /// Get number of failed tasks
    pub fn failed_count(&self) -> usize {
        self.tasks.iter().filter(|t| matches!(t.status, TaskStatus::Failed)).count()
    }
    
    /// Clear all tasks
    pub fn clear(&mut self) {
        self.tasks.clear();
        self.metadata_store.clear();
    }
    
    /// Format for prompt injection (minimal, LLM-readable)
    pub fn to_prompt_format(&self) -> String {
        if self.tasks.is_empty() {
            return "No active tasks.".to_string();
        }
        
        self.tasks.iter()
            .map(|task| format!("- [{}] {}", task.status.symbol(), task.title))
            .collect::<Vec<_>>()
            .join("\n")
    }
    
    /// Get summary statistics
    pub fn stats(&self) -> SessionTaskStats {
        SessionTaskStats {
            total: self.task_count(),
            pending: self.pending_count(),
            in_progress: self.in_progress_count(),
            complete: self.complete_count(),
            failed: self.failed_count(),
        }
    }
}

impl Default for SessionTaskList {
    fn default() -> Self {
        Self::new()
    }
}

/// Individual session task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTask {
    /// Task title (short, descriptive)
    pub title: String,
    
    /// Current status
    pub status: TaskStatus,
    
    /// Optional key for lazy-loading details
    pub metadata_key: Option<String>,
}

/// Task status within a session
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    /// Task not yet started
    Pending,
    
    /// Task currently being worked on
    InProgress,
    
    /// Task completed successfully
    Complete,
    
    /// Task failed
    Failed,
}

impl TaskStatus {
    /// Get status symbol for prompt formatting
    pub fn symbol(&self) -> &'static str {
        match self {
            TaskStatus::Pending => "pending",
            TaskStatus::InProgress => "in_progress",
            TaskStatus::Complete => "complete",
            TaskStatus::Failed => "failed",
        }
    }
    
    /// Get emoji symbol (optional, for UI)
    pub fn emoji(&self) -> &'static str {
        match self {
            TaskStatus::Pending => "⏳",
            TaskStatus::InProgress => "🔄",
            TaskStatus::Complete => "✅",
            TaskStatus::Failed => "❌",
        }
    }
}

/// Session task statistics
#[derive(Debug, Clone, Copy)]
pub struct SessionTaskStats {
    pub total: usize,
    pub pending: usize,
    pub in_progress: usize,
    pub complete: usize,
    pub failed: usize,
}

impl SessionTaskStats {
    /// Calculate completion percentage (0.0 - 1.0)
    pub fn completion_rate(&self) -> f32 {
        if self.total == 0 {
            return 0.0;
        }
        self.complete as f32 / self.total as f32
    }
    
    /// Calculate failure rate (0.0 - 1.0)
    pub fn failure_rate(&self) -> f32 {
        if self.total == 0 {
            return 0.0;
        }
        self.failed as f32 / self.total as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_session_task_list_creation() {
        let list = SessionTaskList::new();
        assert_eq!(list.task_count(), 0);
        assert_eq!(list.max_tasks, 10);
    }
    
    #[test]
    fn test_add_task() {
        let mut list = SessionTaskList::new();
        
        let idx = list.add_task("Create grid".to_string(), None);
        assert_eq!(idx, 0);
        assert_eq!(list.task_count(), 1);
        
        let task = list.get_task("Create grid").unwrap();
        assert_eq!(task.status, TaskStatus::Pending);
    }
    
    #[test]
    fn test_task_status_updates() {
        let mut list = SessionTaskList::new();
        list.add_task("Task 1".to_string(), None);
        
        list.start_task("Task 1").unwrap();
        assert_eq!(list.get_task("Task 1").unwrap().status, TaskStatus::InProgress);
        
        list.complete_task("Task 1").unwrap();
        assert_eq!(list.get_task("Task 1").unwrap().status, TaskStatus::Complete);
    }
    
    #[test]
    fn test_prompt_format() {
        let mut list = SessionTaskList::new();
        list.add_task("Task 1".to_string(), None);
        list.add_task("Task 2".to_string(), None);
        list.add_task("Task 3".to_string(), None);
        
        list.start_task("Task 2").unwrap();
        list.complete_task("Task 1").unwrap();
        
        let formatted = list.to_prompt_format();
        assert!(formatted.contains("[complete] Task 1"));
        assert!(formatted.contains("[in_progress] Task 2"));
        assert!(formatted.contains("[pending] Task 3"));
    }
    
    #[test]
    fn test_fifo_capacity() {
        let mut list = SessionTaskList::with_capacity(3);
        
        list.add_task("Task 1".to_string(), None);
        list.add_task("Task 2".to_string(), None);
        list.add_task("Task 3".to_string(), None);
        assert_eq!(list.task_count(), 3);
        
        // Adding 4th task should remove Task 1
        list.add_task("Task 4".to_string(), None);
        assert_eq!(list.task_count(), 3);
        assert!(list.get_task("Task 1").is_none());
        assert!(list.get_task("Task 4").is_some());
    }
    
    #[test]
    fn test_metadata_lazy_loading() {
        let mut list = SessionTaskList::new();
        list.add_task("Task 1".to_string(), Some("Detailed description".to_string()));
        
        // Title shows in prompt format
        let formatted = list.to_prompt_format();
        assert!(formatted.contains("Task 1"));
        assert!(!formatted.contains("Detailed description"));
        
        // Metadata loaded on-demand
        let details = list.get_task_details("Task 1").unwrap();
        assert_eq!(details, "Detailed description");
    }
    
    #[test]
    fn test_task_statistics() {
        let mut list = SessionTaskList::new();
        list.add_task("Task 1".to_string(), None);
        list.add_task("Task 2".to_string(), None);
        list.add_task("Task 3".to_string(), None);
        list.add_task("Task 4".to_string(), None);
        
        list.complete_task("Task 1").unwrap();
        list.complete_task("Task 2").unwrap();
        list.start_task("Task 3").unwrap();
        list.fail_task("Task 4").unwrap();
        
        let stats = list.stats();
        assert_eq!(stats.total, 4);
        assert_eq!(stats.complete, 2);
        assert_eq!(stats.in_progress, 1);
        assert_eq!(stats.failed, 1);
        assert_eq!(stats.completion_rate(), 0.5);
        assert_eq!(stats.failure_rate(), 0.25);
    }
}
