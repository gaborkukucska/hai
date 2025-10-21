//! # Task Planner
//! 
//! Breaks down complex user requests into executable task plans with dependencies,
//! resource requirements, and PM agent assignments.

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::messaging::AgentId;
use super::intent::Intent;

/// Single step in a task execution plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStep {
    /// Unique step identifier
    pub id: String,
    
    /// Step description
    pub description: String,
    
    /// PM agent responsible for this step
    pub assigned_pm: String,
    
    /// Required worker type (Email, Search, Files, etc.)
    pub required_worker: Option<String>,
    
    /// MCP tool to use (e.g., "hainet_file_read", "hainet_http_get")
    pub mcp_tool: Option<String>,
    
    /// Tool parameters
    pub tool_params: HashMap<String, serde_json::Value>,
    
    /// IDs of steps that must complete before this one
    pub dependencies: Vec<String>,
    
    /// Estimated time in seconds
    pub estimated_time: u64,
    
    /// Whether this step requires user approval
    pub requires_approval: bool,
}

/// Complete task execution plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPlan {
    /// Unique plan identifier
    pub id: String,
    
    /// Original user intent
    pub intent: Intent,
    
    /// Ordered execution steps
    pub steps: Vec<TaskStep>,
    
    /// Overall plan status
    pub status: PlanStatus,
    
    /// Total estimated time in seconds
    pub total_estimated_time: u64,
    
    /// Current step being executed
    pub current_step: Option<usize>,
    
    /// Results from completed steps
    pub step_results: HashMap<String, serde_json::Value>,
}

/// Status of a task plan
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanStatus {
    /// Plan created, awaiting approval
    Pending,
    
    /// Plan approved, executing
    InProgress,
    
    /// Plan completed successfully
    Completed,
    
    /// Plan failed
    Failed,
    
    /// Plan cancelled by user
    Cancelled,
}

/// Task planning system
pub struct TaskPlanner {
    /// Counter for generating unique IDs
    next_id: std::sync::atomic::AtomicU64,
}

impl TaskPlanner {
    /// Create new task planner
    pub fn new() -> Self {
        Self {
            next_id: std::sync::atomic::AtomicU64::new(1),
        }
    }
    
    /// Create task plan from user intent
    /// 
    /// This is a rule-based implementation. In Phase 1.1, we'll integrate with
    /// AI providers for ML-based task decomposition.
    pub async fn create_plan(&self, intent: Intent) -> Result<TaskPlan> {
        let plan_id = self.generate_id();
        
        // Decompose intent into steps based on suggested domain/action
        let steps = self.decompose_intent(&intent)?;
        
        // Calculate total estimated time
        let total_time = steps.iter().map(|s| s.estimated_time).sum();
        
        Ok(TaskPlan {
            id: plan_id,
            intent: intent.clone(),
            steps,
            status: PlanStatus::Pending,
            total_estimated_time: total_time,
            current_step: None,
            step_results: HashMap::new(),
        })
    }
    
    /// Decompose intent into executable steps
    fn decompose_intent(&self, intent: &Intent) -> Result<Vec<TaskStep>> {
        let mut steps = Vec::new();
        
        match (intent.suggested_domain.as_deref(), intent.suggested_action.as_deref()) {
            // Email-related tasks
            (Some("Communications"), Some("email_management")) => {
                if intent.normalized_text.contains("send") {
                    steps.push(TaskStep {
                        id: self.generate_id(),
                        description: "Validate email address".to_string(),
                        assigned_pm: "PM:Communications".to_string(),
                        required_worker: Some("Email".to_string()),
                        mcp_tool: None,
                        tool_params: HashMap::new(),
                        dependencies: vec![],
                        estimated_time: 1,
                        requires_approval: false,
                    });
                    
                    steps.push(TaskStep {
                        id: self.generate_id(),
                        description: "Send email".to_string(),
                        assigned_pm: "PM:Communications".to_string(),
                        required_worker: Some("Email".to_string()),
                        mcp_tool: Some("hainet_email_send".to_string()),
                        tool_params: {
                            let mut params = HashMap::new();
                            if let Some(email) = intent.entities.get("email") {
                                params.insert("to".to_string(), serde_json::json!(email));
                            }
                            params
                        },
                        dependencies: vec![steps[0].id.clone()],
                        estimated_time: 5,
                        requires_approval: true,
                    });
                }
            },
            
            // Search-related tasks
            (Some("Knowledge"), Some("search")) => {
                steps.push(TaskStep {
                    id: self.generate_id(),
                    description: "Perform search query".to_string(),
                    assigned_pm: "PM:Knowledge".to_string(),
                    required_worker: Some("Search".to_string()),
                    mcp_tool: Some("hainet_http_get".to_string()),
                    tool_params: {
                        let mut params = HashMap::new();
                        params.insert("query".to_string(), serde_json::json!(intent.normalized_text));
                        params
                    },
                    dependencies: vec![],
                    estimated_time: 10,
                    requires_approval: false,
                });
                
                steps.push(TaskStep {
                    id: self.generate_id(),
                    description: "Format and present results".to_string(),
                    assigned_pm: "PM:Knowledge".to_string(),
                    required_worker: Some("Search".to_string()),
                    mcp_tool: None,
                    tool_params: HashMap::new(),
                    dependencies: vec![steps[0].id.clone()],
                    estimated_time: 2,
                    requires_approval: false,
                });
            },
            
            // File-related tasks
            (Some("System"), Some("file_management")) => {
                if intent.normalized_text.contains("find") || intent.normalized_text.contains("search") {
                    steps.push(TaskStep {
                        id: self.generate_id(),
                        description: "Search files".to_string(),
                        assigned_pm: "PM:System".to_string(),
                        required_worker: Some("Files".to_string()),
                        mcp_tool: Some("hainet_file_search".to_string()),
                        tool_params: {
                            let mut params = HashMap::new();
                            if let Some(path) = intent.entities.get("file_path") {
                                params.insert("path".to_string(), serde_json::json!(path));
                            } else {
                                params.insert("path".to_string(), serde_json::json!("~"));
                            }
                            params
                        },
                        dependencies: vec![],
                        estimated_time: 5,
                        requires_approval: false,
                    });
                } else if intent.normalized_text.contains("read") || intent.normalized_text.contains("open") {
                    steps.push(TaskStep {
                        id: self.generate_id(),
                        description: "Read file".to_string(),
                        assigned_pm: "PM:System".to_string(),
                        required_worker: Some("Files".to_string()),
                        mcp_tool: Some("hainet_file_read".to_string()),
                        tool_params: {
                            let mut params = HashMap::new();
                            if let Some(path) = intent.entities.get("file_path") {
                                params.insert("path".to_string(), serde_json::json!(path));
                            }
                            params
                        },
                        dependencies: vec![],
                        estimated_time: 2,
                        requires_approval: false,
                    });
                }
            },
            
            // Default fallback
            _ => {
                steps.push(TaskStep {
                    id: self.generate_id(),
                    description: "Process generic request".to_string(),
                    assigned_pm: "PM:Knowledge".to_string(),
                    required_worker: None,
                    mcp_tool: None,
                    tool_params: HashMap::new(),
                    dependencies: vec![],
                    estimated_time: 5,
                    requires_approval: false,
                });
            }
        }
        
        if steps.is_empty() {
            return Err(anyhow!("Could not decompose intent into actionable steps"));
        }
        
        Ok(steps)
    }
    
    /// Generate unique ID
    fn generate_id(&self) -> String {
        let id = self.next_id.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        format!("step_{}", id)
    }
    
    /// Mark step as complete and move to next
    pub fn complete_step(&mut self, plan: &mut TaskPlan, step_id: &str, result: serde_json::Value) -> Result<()> {
        plan.step_results.insert(step_id.to_string(), result);
        
        // Find next step that has all dependencies met
        if let Some(current_idx) = plan.current_step {
            if current_idx + 1 < plan.steps.len() {
                plan.current_step = Some(current_idx + 1);
            } else {
                // All steps complete
                plan.status = PlanStatus::Completed;
                plan.current_step = None;
            }
        }
        
        Ok(())
    }
    
    /// Get next step to execute
    pub fn get_next_step<'a>(&self, plan: &'a TaskPlan) -> Option<&'a TaskStep> {
        if let Some(idx) = plan.current_step {
            plan.steps.get(idx)
        } else {
            None
        }
    }
    
    /// Check if all dependencies for a step are met
    pub fn dependencies_met(&self, plan: &TaskPlan, step: &TaskStep) -> bool {
        step.dependencies.iter().all(|dep_id| {
            plan.step_results.contains_key(dep_id)
        })
    }
}

impl Default for TaskPlanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::intent::{IntentType, Intent};
    
    fn create_test_intent(intent_type: IntentType, text: &str, domain: Option<&str>, action: Option<&str>) -> Intent {
        Intent {
            intent_type,
            original_text: text.to_string(),
            normalized_text: text.to_lowercase(),
            entities: HashMap::new(),
            confidence: 0.8,
            suggested_domain: domain.map(|s| s.to_string()),
            suggested_action: action.map(|s| s.to_string()),
        }
    }
    
    #[tokio::test]
    async fn test_planner_creation() {
        let planner = TaskPlanner::new();
        assert_eq!(planner.next_id.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
    
    #[tokio::test]
    async fn test_create_plan_email() {
        let planner = TaskPlanner::new();
        let mut intent = create_test_intent(
            IntentType::Task,
            "Send an email",
            Some("Communications"),
            Some("email_management")
        );
        intent.entities.insert("email".to_string(), "test@example.com".to_string());
        
        let plan = planner.create_plan(intent).await.unwrap();
        
        assert_eq!(plan.status, PlanStatus::Pending);
        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.steps[0].assigned_pm, "PM:Communications");
        assert_eq!(plan.steps[1].requires_approval, true);
    }
    
    #[tokio::test]
    async fn test_create_plan_search() {
        let planner = TaskPlanner::new();
        let intent = create_test_intent(
            IntentType::Task,
            "Search for rust documentation",
            Some("Knowledge"),
            Some("search")
        );
        
        let plan = planner.create_plan(intent).await.unwrap();
        
        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.steps[0].assigned_pm, "PM:Knowledge");
        assert_eq!(plan.steps[0].mcp_tool, Some("hainet_http_get".to_string()));
    }
    
    #[tokio::test]
    async fn test_create_plan_file_search() {
        let planner = TaskPlanner::new();
        let intent = create_test_intent(
            IntentType::Task,
            "Find my config files",
            Some("System"),
            Some("file_management")
        );
        
        let plan = planner.create_plan(intent).await.unwrap();
        
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].mcp_tool, Some("hainet_file_search".to_string()));
    }
    
    #[tokio::test]
    async fn test_complete_step() {
        let mut planner = TaskPlanner::new();
        let intent = create_test_intent(
            IntentType::Task,
            "Search for something",
            Some("Knowledge"),
            Some("search")
        );
        
        let mut plan = planner.create_plan(intent).await.unwrap();
        plan.current_step = Some(0);
        
        let step_id = plan.steps[0].id.clone();
        planner.complete_step(&mut plan, &step_id, serde_json::json!({"status": "ok"})).unwrap();
        
        assert!(plan.step_results.contains_key(&step_id));
        assert_eq!(plan.current_step, Some(1));
    }
    
    #[tokio::test]
    async fn test_dependencies_met() {
        let planner = TaskPlanner::new();
        let intent = create_test_intent(
            IntentType::Task,
            "Send email",
            Some("Communications"),
            Some("email_management")
        );
        
        let mut plan = planner.create_plan(intent).await.unwrap();
        
        // First step has no dependencies
        assert!(planner.dependencies_met(&plan, &plan.steps[0]));
        
        // Second step depends on first
        assert!(!planner.dependencies_met(&plan, &plan.steps[1]));
        
        // Mark first step complete
        plan.step_results.insert(plan.steps[0].id.clone(), serde_json::json!({}));
        assert!(planner.dependencies_met(&plan, &plan.steps[1]));
    }
    
    #[tokio::test]
    async fn test_get_next_step() {
        let planner = TaskPlanner::new();
        let intent = create_test_intent(
            IntentType::Task,
            "Search",
            Some("Knowledge"),
            Some("search")
        );
        
        let mut plan = planner.create_plan(intent).await.unwrap();
        plan.current_step = Some(0);
        
        let next = planner.get_next_step(&plan);
        assert!(next.is_some());
        assert_eq!(next.unwrap().id, plan.steps[0].id);
    }
}
