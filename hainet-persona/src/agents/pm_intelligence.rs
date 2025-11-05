//! # START OF FILE hainet-persona/src/agents/pm_intelligence.rs
//! PM Intelligence Module - Enhanced task decomposition and learning capabilities

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::SystemTime;
use tracing::{debug, info};

/// Strategy for decomposing tasks based on project characteristics
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DecompositionStrategy {
    /// Sequential execution - tasks depend on previous completion
    Sequential,
    /// Parallel execution - independent tasks can run simultaneously
    Parallel,
    /// Hybrid approach - mix of sequential and parallel
    Hybrid,
}

impl Default for DecompositionStrategy {
    fn default() -> Self {
        Self::Hybrid
    }
}

/// Project complexity metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectComplexity {
    /// Number of initial tasks
    pub task_count: usize,
    /// Estimated lines of code or work units
    pub estimated_size: usize,
    /// Number of different domains involved (files, network, research, etc.)
    pub domain_count: usize,
    /// Presence of external dependencies
    pub has_external_deps: bool,
    /// Complexity score (0.0 = simple, 1.0 = very complex)
    pub score: f64,
}

impl ProjectComplexity {
    /// Analyze project overview and initial tasks to determine complexity
    pub fn analyze(overview: &str, initial_tasks: &[String]) -> Self {
        let task_count = initial_tasks.len();
        
        // Simple heuristics for complexity scoring
        let overview_len = overview.len();
        let avg_task_len = if task_count > 0 {
            initial_tasks.iter().map(|t| t.len()).sum::<usize>() / task_count
        } else {
            0
        };
        
        // Domain detection (simplified)
        let mut domains = vec![];
        let text = format!("{} {}", overview, initial_tasks.join(" ")).to_lowercase();
        
        if text.contains("file") || text.contains("read") || text.contains("write") {
            domains.push("files");
        }
        if text.contains("network") || text.contains("http") || text.contains("api") {
            domains.push("network");
        }
        if text.contains("research") || text.contains("search") || text.contains("analyze") {
            domains.push("research");
        }
        if text.contains("code") || text.contains("implement") || text.contains("develop") {
            domains.push("code");
        }
        
        let domain_count = domains.len();
        let has_external_deps = text.contains("api") || text.contains("external") || text.contains("library");
        
        // Calculate complexity score (normalized 0.0-1.0)
        let task_complexity = (task_count as f64 / 10.0).min(1.0);
        let size_complexity = ((overview_len + avg_task_len) as f64 / 1000.0).min(1.0);
        let domain_complexity = (domain_count as f64 / 4.0).min(1.0);
        let deps_complexity = if has_external_deps { 0.3 } else { 0.0 };
        
        let score = (task_complexity * 0.3 + size_complexity * 0.2 + domain_complexity * 0.3 + deps_complexity * 0.2).min(1.0);
        
        Self {
            task_count,
            estimated_size: overview_len + avg_task_len * task_count,
            domain_count,
            has_external_deps,
            score,
        }
    }
    
    /// Get complexity category
    pub fn category(&self) -> &str {
        if self.score < 0.3 {
            "simple"
        } else if self.score < 0.6 {
            "moderate"
        } else {
            "complex"
        }
    }
}

/// Historical project outcome for learning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectOutcome {
    /// Project ID
    pub project_id: String,
    /// Strategy used
    pub strategy: DecompositionStrategy,
    /// Complexity metrics
    pub complexity: ProjectComplexity,
    /// Success (completed without failures)
    pub success: bool,
    /// Total duration in seconds
    pub duration_secs: u64,
    /// Number of task revisions required
    pub revision_count: usize,
    /// Timestamp
    pub timestamp: SystemTime,
}

/// Learns from historical project outcomes to improve strategy selection
pub struct HistoricalLearner {
    /// Past project outcomes
    outcomes: Vec<ProjectOutcome>,
    /// Maximum outcomes to retain
    max_history: usize,
}

impl HistoricalLearner {
    /// Create new learner with default capacity
    pub fn new() -> Self {
        Self {
            outcomes: Vec::new(),
            max_history: 100,
        }
    }
    
    /// Create with custom capacity
    pub fn with_capacity(max_history: usize) -> Self {
        Self {
            outcomes: Vec::new(),
            max_history,
        }
    }
    
    /// Record a project outcome
    pub fn record_outcome(&mut self, outcome: ProjectOutcome) {
        info!(
            "Recording project outcome: strategy={:?}, success={}, duration={}s",
            outcome.strategy, outcome.success, outcome.duration_secs
        );
        
        self.outcomes.push(outcome);
        
        // Trim to max capacity (keep most recent)
        if self.outcomes.len() > self.max_history {
            self.outcomes.drain(0..self.outcomes.len() - self.max_history);
        }
    }
    
    /// Analyze historical data to recommend strategy
    pub fn recommend_strategy(&self, complexity: &ProjectComplexity) -> DecompositionStrategy {
        if self.outcomes.is_empty() {
            debug!("No historical data, using default strategy");
            return DecompositionStrategy::Hybrid;
        }
        
        // Find similar past projects (complexity score within 0.2)
        let similar: Vec<_> = self.outcomes.iter()
            .filter(|o| (o.complexity.score - complexity.score).abs() < 0.2)
            .collect();
        
        if similar.is_empty() {
            debug!("No similar projects found, using default strategy");
            return DecompositionStrategy::Hybrid;
        }
        
        // Calculate success rate for each strategy
        let mut strategy_stats: HashMap<DecompositionStrategy, (usize, usize)> = HashMap::new();
        
        for outcome in &similar {
            let entry = strategy_stats.entry(outcome.strategy).or_insert((0, 0));
            entry.0 += 1; // total count
            if outcome.success {
                entry.1 += 1; // success count
            }
        }
        
        // Select strategy with highest success rate
        let best_strategy = strategy_stats.iter()
            .max_by(|(_, (total_a, success_a)), (_, (total_b, success_b))| {
                let rate_a = *success_a as f64 / *total_a as f64;
                let rate_b = *success_b as f64 / *total_b as f64;
                rate_a.partial_cmp(&rate_b).unwrap()
            })
            .map(|(strategy, _)| *strategy)
            .unwrap_or(DecompositionStrategy::Hybrid);
        
        info!(
            "Recommended strategy: {:?} based on {} similar projects",
            best_strategy, similar.len()
        );
        
        best_strategy
    }
    
    /// Get success rate for a specific strategy
    pub fn strategy_success_rate(&self, strategy: DecompositionStrategy) -> f64 {
        let matching: Vec<_> = self.outcomes.iter()
            .filter(|o| o.strategy == strategy)
            .collect();
        
        if matching.is_empty() {
            return 0.5; // No data, assume neutral
        }
        
        let success_count = matching.iter().filter(|o| o.success).count();
        success_count as f64 / matching.len() as f64
    }
    
    /// Get total number of recorded outcomes
    pub fn outcome_count(&self) -> usize {
        self.outcomes.len()
    }
    
    /// Get average duration for successful projects
    pub fn average_success_duration(&self) -> Option<u64> {
        let successful: Vec<_> = self.outcomes.iter()
            .filter(|o| o.success)
            .collect();
        
        if successful.is_empty() {
            return None;
        }
        
        let total_duration: u64 = successful.iter().map(|o| o.duration_secs).sum();
        Some(total_duration / successful.len() as u64)
    }
}

impl Default for HistoricalLearner {
    fn default() -> Self {
        Self::new()
    }
}

/// Analyzes task complexity to inform decomposition
pub struct TaskComplexityAnalyzer;

impl TaskComplexityAnalyzer {
    /// Determine optimal strategy based on project complexity
    pub fn select_strategy(complexity: &ProjectComplexity) -> DecompositionStrategy {
        match complexity.category() {
            "simple" => {
                // Simple projects work well with sequential or parallel
                if complexity.task_count <= 3 {
                    DecompositionStrategy::Sequential
                } else {
                    DecompositionStrategy::Parallel
                }
            },
            "moderate" => {
                // Moderate complexity benefits from hybrid approach
                DecompositionStrategy::Hybrid
            },
            "complex" => {
                // Complex projects need careful sequencing with some parallelism
                DecompositionStrategy::Hybrid
            },
            _ => DecompositionStrategy::Hybrid,
        }
    }
    
    /// Estimate task execution time based on complexity
    pub fn estimate_duration(complexity: &ProjectComplexity) -> u64 {
        // Simple heuristic: base time + task count factor + size factor
        let base = 60; // 1 minute base
        let task_factor = complexity.task_count as u64 * 120; // 2 minutes per task
        let size_factor = (complexity.estimated_size as u64 / 100).max(60); // Size-based
        
        base + task_factor + size_factor
    }
}

/// Adjusts tasks dynamically during execution
pub struct DynamicTaskAdjuster;

impl DynamicTaskAdjuster {
    /// Check if task should be split based on progress
    pub fn should_split_task(task_duration_secs: u64, estimated_secs: u64) -> bool {
        // If task takes >2x estimated time, consider splitting
        task_duration_secs > estimated_secs * 2
    }
    
    /// Check if tasks should be merged
    pub fn should_merge_tasks(task_durations: &[u64], avg_duration: u64) -> bool {
        // If multiple tasks complete very quickly (<10% of average), consider merging
        task_durations.iter().filter(|&&d| d < avg_duration / 10).count() > 1
    }
    
    /// Suggest task split strategy
    pub fn suggest_split(task_description: &str) -> Vec<String> {
        // Simple heuristic: split on conjunctions or lists
        let mut subtasks = Vec::new();
        
        // Look for "and" patterns
        if task_description.contains(" and ") {
            let parts: Vec<_> = task_description.split(" and ").collect();
            for (i, part) in parts.iter().enumerate() {
                subtasks.push(format!("Subtask {}: {}", i + 1, part.trim()));
            }
        } else if task_description.contains(", ") {
            let parts: Vec<_> = task_description.split(", ").collect();
            for (i, part) in parts.iter().enumerate() {
                subtasks.push(format!("Subtask {}: {}", i + 1, part.trim()));
            }
        }
        
        // If no clear split found, return original
        if subtasks.is_empty() {
            subtasks.push(task_description.to_string());
        }
        
        subtasks
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_complexity_analysis_simple() {
        let complexity = ProjectComplexity::analyze(
            "Create a simple hello world app",
            &vec!["Write code".to_string(), "Test".to_string()],
        );
        
        assert_eq!(complexity.category(), "simple");
        assert!(complexity.score < 0.3);
    }
    
    #[test]
    fn test_complexity_analysis_complex() {
        let complexity = ProjectComplexity::analyze(
            "Build a distributed microservices architecture with API gateway, multiple databases, authentication, monitoring, and deployment pipeline",
            &vec![
                "Design architecture".to_string(),
                "Implement API gateway".to_string(),
                "Create authentication service".to_string(),
                "Set up databases".to_string(),
                "Build monitoring".to_string(),
                "Configure deployment".to_string(),
                "Write tests".to_string(),
            ],
        );
        
        assert!(complexity.score > 0.5);
        assert!(complexity.domain_count >= 2);
    }
    
    #[test]
    fn test_strategy_selection_simple() {
        let complexity = ProjectComplexity {
            task_count: 2,
            estimated_size: 100,
            domain_count: 1,
            has_external_deps: false,
            score: 0.2,
        };
        
        let strategy = TaskComplexityAnalyzer::select_strategy(&complexity);
        assert_eq!(strategy, DecompositionStrategy::Sequential);
    }
    
    #[test]
    fn test_strategy_selection_complex() {
        let complexity = ProjectComplexity {
            task_count: 8,
            estimated_size: 2000,
            domain_count: 4,
            has_external_deps: true,
            score: 0.8,
        };
        
        let strategy = TaskComplexityAnalyzer::select_strategy(&complexity);
        assert_eq!(strategy, DecompositionStrategy::Hybrid);
    }
    
    #[test]
    fn test_historical_learner_empty() {
        let learner = HistoricalLearner::new();
        let complexity = ProjectComplexity {
            task_count: 5,
            estimated_size: 500,
            domain_count: 2,
            has_external_deps: false,
            score: 0.5,
        };
        
        let strategy = learner.recommend_strategy(&complexity);
        assert_eq!(strategy, DecompositionStrategy::Hybrid); // Default
    }
    
    #[test]
    fn test_historical_learner_recommendation() {
        let mut learner = HistoricalLearner::new();
        
        // Record successful sequential projects
        for i in 0..5 {
            learner.record_outcome(ProjectOutcome {
                project_id: format!("proj_{}", i),
                strategy: DecompositionStrategy::Sequential,
                complexity: ProjectComplexity {
                    task_count: 3,
                    estimated_size: 300,
                    domain_count: 1,
                    has_external_deps: false,
                    score: 0.25,
                },
                success: true,
                duration_secs: 180,
                revision_count: 0,
                timestamp: SystemTime::now(),
            });
        }
        
        // Query for similar complexity
        let complexity = ProjectComplexity {
            task_count: 3,
            estimated_size: 320,
            domain_count: 1,
            has_external_deps: false,
            score: 0.28,
        };
        
        let strategy = learner.recommend_strategy(&complexity);
        assert_eq!(strategy, DecompositionStrategy::Sequential);
    }
    
    #[test]
    fn test_task_split_suggestion() {
        let task = "Read file and parse JSON and validate schema";
        let subtasks = DynamicTaskAdjuster::suggest_split(task);
        
        assert!(subtasks.len() > 1);
        assert!(subtasks[0].contains("Read file"));
    }
}
