//! # START OF FILE hainet-persona/src/agents/worker_intelligence.rs
//! Worker Intelligence Module
//!
//! Provides learning capabilities for Worker agents to improve task execution
//! through historical outcome tracking, adaptive execution strategies, and
//! intelligent tool selection.

use std::collections::HashMap;
use std::time::SystemTime;
use serde::{Deserialize, Serialize};

/// Category of task execution error for self-correction decisions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCategory {
    /// Temporary error that may succeed on retry (network timeout, resource busy)
    Transient,
    /// Permanent error that won't succeed on retry (file not found, permission denied)
    Permanent,
    /// Unknown error type, unclear if retry will help
    Unknown,
}

impl ErrorCategory {
    /// Classify error from error message patterns
    pub fn classify(error_msg: &str) -> Self {
        let msg = error_msg.to_lowercase();
        
        // Transient errors
        if msg.contains("timeout") || 
           msg.contains("connection refused") ||
           msg.contains("temporarily unavailable") ||
           msg.contains("resource busy") ||
           msg.contains("try again") {
            return ErrorCategory::Transient;
        }
        
        // Permanent errors
        if msg.contains("not found") ||
           msg.contains("permission denied") ||
           msg.contains("access denied") ||
           msg.contains("invalid") ||
           msg.contains("does not exist") {
            return ErrorCategory::Permanent;
        }
        
        // Default to unknown
        ErrorCategory::Unknown
    }
}

/// Record of a single task execution outcome
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskOutcome {
    /// Type of task executed (e.g., "file_edit", "api_call", "code_generation")
    pub task_type: String,
    
    /// MCP tool used (e.g., "hainet-files::write_file")
    pub tool_used: String,
    
    /// Whether the task succeeded
    pub success: bool,
    
    /// Duration of execution in milliseconds
    pub duration_ms: u64,
    
    /// Number of retry attempts before final outcome
    pub retry_count: u32,
    
    /// Error category if task failed
    pub error_category: Option<ErrorCategory>,
    
    /// When the task was executed
    pub timestamp: SystemTime,
}

/// Success metrics for a specific tool or task type
#[derive(Debug, Clone)]
pub struct SuccessMetrics {
    /// Total number of attempts
    pub total_attempts: u32,
    
    /// Number of successful attempts
    pub successes: u32,
    
    /// Average duration in milliseconds
    pub avg_duration_ms: u64,
    
    /// Average retry count
    pub avg_retries: f64,
}

impl SuccessMetrics {
    /// Calculate success rate (0.0 to 1.0)
    pub fn success_rate(&self) -> f64 {
        if self.total_attempts == 0 {
            0.0
        } else {
            self.successes as f64 / self.total_attempts as f64
        }
    }
    
    /// Check if metrics indicate reliable tool
    pub fn is_reliable(&self) -> bool {
        self.total_attempts >= 3 && self.success_rate() >= 0.8
    }
}

/// Learns from task execution history to improve future performance
pub struct WorkerLearner {
    /// Historical task outcomes
    outcomes: Vec<TaskOutcome>,
    
    /// Maximum number of outcomes to store (FIFO)
    capacity: usize,
    
    /// Cached success metrics per tool
    tool_metrics: HashMap<String, SuccessMetrics>,
    
    /// Cached success metrics per task type
    task_type_metrics: HashMap<String, SuccessMetrics>,
}

impl WorkerLearner {
    /// Create new learner with default capacity (100 outcomes)
    pub fn new() -> Self {
        Self::with_capacity(100)
    }
    
    /// Create new learner with custom capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            outcomes: Vec::new(),
            capacity,
            tool_metrics: HashMap::new(),
            task_type_metrics: HashMap::new(),
        }
    }
    
    /// Record a task outcome
    pub fn record_outcome(&mut self, outcome: TaskOutcome) {
        // Add outcome
        self.outcomes.push(outcome);
        
        // Enforce capacity limit (FIFO)
        if self.outcomes.len() > self.capacity {
            self.outcomes.remove(0);
        }
        
        // Invalidate cached metrics (will be recalculated on next access)
        self.tool_metrics.clear();
        self.task_type_metrics.clear();
    }
    
    /// Get number of recorded outcomes
    pub fn outcome_count(&self) -> usize {
        self.outcomes.len()
    }
    
    /// Calculate success metrics for a specific tool
    pub fn get_tool_metrics(&mut self, tool: &str) -> Option<&SuccessMetrics> {
        // Return cached if available
        if self.tool_metrics.contains_key(tool) {
            return self.tool_metrics.get(tool);
        }
        
        // Calculate metrics
        let relevant_outcomes: Vec<&TaskOutcome> = self.outcomes.iter()
            .filter(|o| o.tool_used == tool)
            .collect();
        
        if relevant_outcomes.is_empty() {
            return None;
        }
        
        let total_attempts = relevant_outcomes.len() as u32;
        let successes = relevant_outcomes.iter().filter(|o| o.success).count() as u32;
        let total_duration: u64 = relevant_outcomes.iter().map(|o| o.duration_ms).sum();
        let total_retries: u32 = relevant_outcomes.iter().map(|o| o.retry_count).sum();
        
        let metrics = SuccessMetrics {
            total_attempts,
            successes,
            avg_duration_ms: total_duration / total_attempts as u64,
            avg_retries: total_retries as f64 / total_attempts as f64,
        };
        
        self.tool_metrics.insert(tool.to_string(), metrics);
        self.tool_metrics.get(tool)
    }
    
    /// Calculate success metrics for a specific task type
    pub fn get_task_type_metrics(&mut self, task_type: &str) -> Option<&SuccessMetrics> {
        // Return cached if available
        if self.task_type_metrics.contains_key(task_type) {
            return self.task_type_metrics.get(task_type);
        }
        
        // Calculate metrics
        let relevant_outcomes: Vec<&TaskOutcome> = self.outcomes.iter()
            .filter(|o| o.task_type == task_type)
            .collect();
        
        if relevant_outcomes.is_empty() {
            return None;
        }
        
        let total_attempts = relevant_outcomes.len() as u32;
        let successes = relevant_outcomes.iter().filter(|o| o.success).count() as u32;
        let total_duration: u64 = relevant_outcomes.iter().map(|o| o.duration_ms).sum();
        let total_retries: u32 = relevant_outcomes.iter().map(|o| o.retry_count).sum();
        
        let metrics = SuccessMetrics {
            total_attempts,
            successes,
            avg_duration_ms: total_duration / total_attempts as u64,
            avg_retries: total_retries as f64 / total_attempts as f64,
        };
        
        self.task_type_metrics.insert(task_type.to_string(), metrics);
        self.task_type_metrics.get(task_type)
    }
    
    /// Recommend best tool for a task type based on history
    pub fn recommend_tool(&mut self, task_type: &str, available_tools: &[String]) -> Option<String> {
        // Filter outcomes for this task type
        let relevant_outcomes: Vec<&TaskOutcome> = self.outcomes.iter()
            .filter(|o| o.task_type == task_type)
            .collect();
        
        if relevant_outcomes.is_empty() {
            // No history, return first available tool
            return available_tools.first().cloned();
        }
        
        // Calculate success rate for each tool used with this task type
        let mut tool_scores: Vec<(String, f64)> = Vec::new();
        
        for tool in available_tools {
            let tool_outcomes: Vec<&&TaskOutcome> = relevant_outcomes.iter()
                .filter(|o| &o.tool_used == tool)
                .collect();
            
            if tool_outcomes.is_empty() {
                continue;
            }
            
            let successes = tool_outcomes.iter().filter(|o| o.success).count();
            let success_rate = successes as f64 / tool_outcomes.len() as f64;
            
            tool_scores.push((tool.clone(), success_rate));
        }
        
        // Sort by success rate (descending)
        tool_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        
        // Return best tool if we have data, otherwise first available
        tool_scores.first()
            .map(|(tool, _)| tool.clone())
            .or_else(|| available_tools.first().cloned())
    }
}

impl Default for WorkerLearner {
    fn default() -> Self {
        Self::new()
    }
}

/// Adaptive execution strategy that adjusts based on task history
#[derive(Debug, Clone)]
pub struct ExecutionStrategy {
    /// Base timeout in milliseconds
    pub base_timeout_ms: u64,
    
    /// Maximum number of retry attempts
    pub max_retries: u32,
    
    /// Backoff multiplier for retry delays (e.g., 1.5 = 50% increase each retry)
    pub backoff_multiplier: f64,
}

impl ExecutionStrategy {
    /// Create default strategy (60s timeout for complex tasks, 3 retries, 1.5x backoff)
    /// Note: LLM calls use 2x this timeout (120s) to allow for complex code generation
    pub fn default() -> Self {
        Self {
            base_timeout_ms: 60000, // 60s base, 120s for LLM calls (2x multiplier)
            max_retries: 3,
            backoff_multiplier: 1.5,
        }
    }
    
    /// Adjust strategy for task type based on history
    pub fn adjust_for_task(&mut self, task_type: &str, learner: &mut WorkerLearner) {
        if let Some(metrics) = learner.get_task_type_metrics(task_type) {
            // If average duration is high, increase timeout
            if metrics.avg_duration_ms > self.base_timeout_ms {
                self.base_timeout_ms = (metrics.avg_duration_ms as f64 * 1.5) as u64;
            }
            
            // If success rate is high, reduce retries (fast fail)
            if metrics.success_rate() > 0.9 {
                self.max_retries = 2;
            }
            
            // If average retries are high, allow more attempts
            if metrics.avg_retries > 2.0 {
                self.max_retries = 5;
            }
        }
    }
    
    /// Calculate retry delay for attempt number (0-based)
    pub fn retry_delay_ms(&self, attempt: u32) -> u64 {
        let base_delay = 500; // 500ms base delay
        (base_delay as f64 * self.backoff_multiplier.powi(attempt as i32)) as u64
    }
}

/// Intelligent tool selector that learns best tools for task types
pub struct ToolSelector {
    /// Worker learner for historical data
    learner: WorkerLearner,
    
    /// Fallback tool order when no history available
    fallback_order: Vec<String>,
}

impl ToolSelector {
    /// Create new tool selector
    pub fn new(fallback_order: Vec<String>) -> Self {
        Self {
            learner: WorkerLearner::new(),
            fallback_order,
        }
    }
    
    /// Select best tool for task based on history
    pub fn select_best_tool(&mut self, task_type: &str, available_tools: &[String]) -> String {
        // Try to get recommendation from learner
        if let Some(tool) = self.learner.recommend_tool(task_type, available_tools) {
            return tool;
        }
        
        // Fallback to predefined order
        for fallback_tool in &self.fallback_order {
            if available_tools.contains(fallback_tool) {
                return fallback_tool.clone();
            }
        }
        
        // Last resort: return first available
        available_tools.first()
            .cloned()
            .unwrap_or_else(|| "unknown".to_string())
    }
    
    /// Record outcome for learning
    pub fn record_outcome(&mut self, outcome: TaskOutcome) {
        self.learner.record_outcome(outcome);
    }
    
    /// Get reference to learner for direct access
    pub fn learner(&mut self) -> &mut WorkerLearner {
        &mut self.learner
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_error_classification() {
        assert_eq!(ErrorCategory::classify("Connection timeout"), ErrorCategory::Transient);
        assert_eq!(ErrorCategory::classify("File not found"), ErrorCategory::Permanent);
        assert_eq!(ErrorCategory::classify("Unknown error"), ErrorCategory::Unknown);
    }
    
    #[test]
    fn test_success_metrics() {
        let metrics = SuccessMetrics {
            total_attempts: 10,
            successes: 8,
            avg_duration_ms: 1000,
            avg_retries: 1.5,
        };
        
        assert_eq!(metrics.success_rate(), 0.8);
        assert!(metrics.is_reliable());
    }
    
    #[test]
    fn test_learner_capacity() {
        let mut learner = WorkerLearner::with_capacity(3);
        
        for i in 0..5 {
            learner.record_outcome(TaskOutcome {
                task_type: "test".to_string(),
                tool_used: "tool".to_string(),
                success: true,
                duration_ms: 100,
                retry_count: 0,
                error_category: None,
                timestamp: SystemTime::now(),
            });
        }
        
        assert_eq!(learner.outcome_count(), 3); // Only last 3 retained
    }
}
