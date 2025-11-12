//! # START OF FILE hainet-persona/src/agents/pm_discovery.rs
//! PM Agent Discovery-Based Execution Module
//! 
//! Implements just-in-time tool loading for PM agents.
//! Similar to worker discovery but focused on project management operations.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Tool identification request from PM LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PMToolSelectionRequest {
    /// Tools the PM wants to use
    pub needed_tools: Vec<String>,
    /// Reasoning for tool selection
    pub reasoning: String,
}

/// Project task decomposition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectTask {
    pub task_number: usize,
    pub title: String,
    pub description: String,
    pub assigned_worker_type: String,
    pub depends_on: Vec<usize>,
    pub priority: TaskPriority,
}

/// Task priority level
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskPriority {
    High,
    Medium,
    Low,
}

/// Project execution plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectExecutionPlan {
    pub tasks: Vec<ProjectTask>,
}

/// Worker task feedback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerTaskFeedback {
    pub status: WorkerTaskStatus,
    pub reasoning: String,
    pub next_action: String,
    #[serde(default)]
    pub feedback_to_worker: Option<String>,
    pub escalate: bool,
}

/// Worker task status
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkerTaskStatus {
    Success,
    #[serde(rename = "needs_revision")]
    NeedsRevision,
    Failed,
}

/// PM discovery context
pub struct PMDiscoveryContext {
    pub project_title: String,
    pub pm_role: String,
    pub pm_specialization: String,
    pub user_request: String,
    pub session_tasks: String,
    pub project_context: String,
}

impl PMDiscoveryContext {
    pub fn new(
        project_title: String,
        pm_role: String,
        pm_specialization: String,
        user_request: String,
        session_tasks: String,
    ) -> Self {
        Self {
            project_title,
            pm_role,
            pm_specialization,
            user_request,
            session_tasks,
            project_context: String::new(),
        }
    }
    
    pub fn set_project_context(&mut self, context: String) {
        self.project_context = context;
    }
}

/// Parse PM tool selection from LLM response
pub fn parse_pm_tool_selection(llm_response: &str) -> Result<PMToolSelectionRequest> {
    if let Ok(selection) = serde_json::from_str::<PMToolSelectionRequest>(llm_response) {
        return Ok(selection);
    }
    
    if let Some(json_str) = extract_json_from_markdown(llm_response) {
        if let Ok(selection) = serde_json::from_str::<PMToolSelectionRequest>(&json_str) {
            return Ok(selection);
        }
    }
    
    if let Some(json_str) = extract_json_from_braces(llm_response) {
        if let Ok(selection) = serde_json::from_str::<PMToolSelectionRequest>(&json_str) {
            return Ok(selection);
        }
    }
    
    Err(anyhow::anyhow!(
        "Failed to parse PM tool selection: {}",
        &llm_response[..llm_response.len().min(200)]
    ))
}

/// Parse project execution plan from LLM response
pub fn parse_project_execution_plan(llm_response: &str) -> Result<ProjectExecutionPlan> {
    if let Ok(plan) = serde_json::from_str::<ProjectExecutionPlan>(llm_response) {
        return Ok(plan);
    }
    
    if let Some(json_str) = extract_json_from_markdown(llm_response) {
        if let Ok(plan) = serde_json::from_str::<ProjectExecutionPlan>(&json_str) {
            return Ok(plan);
        }
    }
    
    if let Some(json_str) = extract_json_from_braces(llm_response) {
        if let Ok(plan) = serde_json::from_str::<ProjectExecutionPlan>(&json_str) {
            return Ok(plan);
        }
    }
    
    Err(anyhow::anyhow!(
        "Failed to parse project execution plan: {}",
        &llm_response[..llm_response.len().min(200)]
    ))
}

/// Parse worker task feedback from LLM response
pub fn parse_worker_task_feedback(llm_response: &str) -> Result<WorkerTaskFeedback> {
    if let Ok(feedback) = serde_json::from_str::<WorkerTaskFeedback>(llm_response) {
        return Ok(feedback);
    }
    
    if let Some(json_str) = extract_json_from_markdown(llm_response) {
        if let Ok(feedback) = serde_json::from_str::<WorkerTaskFeedback>(&json_str) {
            return Ok(feedback);
        }
    }
    
    if let Some(json_str) = extract_json_from_braces(llm_response) {
        if let Ok(feedback) = serde_json::from_str::<WorkerTaskFeedback>(&json_str) {
            return Ok(feedback);
        }
    }
    
    Err(anyhow::anyhow!(
        "Failed to parse worker task feedback: {}",
        &llm_response[..llm_response.len().min(200)]
    ))
}

fn extract_json_from_markdown(text: &str) -> Option<String> {
    let markers = ["```json\n", "```\n", "```"];
    
    for marker in markers.iter() {
        if let Some(start_idx) = text.find(marker) {
            let json_start = start_idx + marker.len();
            
            if let Some(end_idx) = text[json_start..].find("```") {
                let json_text = &text[json_start..json_start + end_idx];
                return Some(json_text.trim().to_string());
            }
        }
    }
    
    None
}

fn extract_json_from_braces(text: &str) -> Option<String> {
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            return Some(text[start..=end].to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_pm_tool_selection() {
        let json = r#"{"needed_tools": ["hainet-dev::code_analysis"], "reasoning": "Need to analyze code"}"#;
        let selection = parse_pm_tool_selection(json).unwrap();
        assert_eq!(selection.needed_tools.len(), 1);
        assert_eq!(selection.needed_tools[0], "hainet-dev::code_analysis");
    }
    
    #[test]
    fn test_parse_project_execution_plan() {
        let json = r#"{"tasks": [{"task_number": 1, "title": "Setup", "description": "Init", "assigned_worker_type": "FileWorker", "depends_on": [], "priority": "high"}]}"#;
        let plan = parse_project_execution_plan(json).unwrap();
        assert_eq!(plan.tasks.len(), 1);
        assert_eq!(plan.tasks[0].title, "Setup");
    }
}
