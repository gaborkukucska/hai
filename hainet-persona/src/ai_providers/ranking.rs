// START OF FILE hainet-persona/src/ai_providers/ranking.rs

//! Model Ranking System
//!
//! Scores and ranks available models based on multiple criteria including:
//! - Capability match
//! - Performance metrics
//! - Availability
//! - Resource efficiency
//! - Task-specific requirements

use crate::ai_providers::catalog::{CatalogedModel, ModelCapability};
use crate::ai_providers::selection::SelectionContext;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::debug;

/// Model ranker with configurable criteria
pub struct ModelRanker {
    criteria: RankingCriteria,
}

impl ModelRanker {
    /// Create new ranker with default criteria
    pub fn new() -> Self {
        Self {
            criteria: RankingCriteria::default(),
        }
    }

    /// Create ranker with custom criteria
    pub fn with_criteria(criteria: RankingCriteria) -> Self {
        Self { criteria }
    }

    /// Rank models for a given selection context
    pub async fn rank_models(
        &self,
        catalog: &crate::ai_providers::catalog::ModelCatalog,
        context: &SelectionContext,
    ) -> Result<Vec<ModelScore>> {
        let models = catalog.all_models();

        let mut scores: Vec<ModelScore> = models
            .into_iter()
            .map(|model| self.score_model(model, context))
            .collect();

        // Sort by score descending
        scores.sort_by(|a, b| b.total_score.partial_cmp(&a.total_score).unwrap());

        debug!("Ranked {} models for {:?}", scores.len(), context.agent_type);

        Ok(scores)
    }

    /// Score a single model against context
    fn score_model(&self, model: &CatalogedModel, context: &SelectionContext) -> ModelScore {
        let capability_score = self.score_capabilities(model, context);
        let performance_score = self.score_performance(model, context);
        let availability_score = model.availability_score;
        let efficiency_score = self.score_efficiency(model, context);
        let recency_score = self.score_recency(model);

        let total_score = capability_score * self.criteria.capability_weight
            + performance_score * self.criteria.performance_weight
            + availability_score * self.criteria.availability_weight
            + efficiency_score * self.criteria.efficiency_weight
            + recency_score * self.criteria.recency_weight;

        ModelScore {
            model_id: model.id.clone(),
            total_score,
            capability_score,
            performance_score,
            availability_score,
            efficiency_score,
            recency_score,
            breakdown: ScoreBreakdown {
                capabilities_matched: capability_score,
                latency_factor: performance_score,
                availability_factor: availability_score,
                size_efficiency: efficiency_score,
                recency_bonus: recency_score,
            },
        }
    }

    /// Score capability match
    fn score_capabilities(&self, model: &CatalogedModel, context: &SelectionContext) -> f32 {
        let required = &context.required_capabilities;
        let preferred = &context.preferred_capabilities;

        if required.is_empty() && preferred.is_empty() {
            return 1.0; // No specific requirements
        }

        // Check required capabilities (must have all)
        let required_match = required.iter().all(|cap| model.capabilities.contains(cap));
        if !required_match {
            return 0.0; // Hard requirement not met
        }

        // Score preferred capabilities (bonus for each match)
        let preferred_count = preferred.len();
        let preferred_matches = preferred
            .iter()
            .filter(|cap| model.capabilities.contains(cap))
            .count();

        if preferred_count == 0 {
            1.0
        } else {
            0.7 + 0.3 * (preferred_matches as f32 / preferred_count as f32)
        }
    }

    /// Score performance metrics
    fn score_performance(&self, model: &CatalogedModel, context: &SelectionContext) -> f32 {
        let metrics = &model.performance_metrics;

        if metrics.total_requests == 0 {
            return 0.5; // No historical data, neutral score
        }

        // Latency score (lower is better)
        let latency_score = if metrics.avg_latency_ms <= context.max_latency_ms as f32 {
            1.0 - (metrics.avg_latency_ms / (context.max_latency_ms as f32 * 2.0)).min(1.0)
        } else {
            0.0 // Exceeds max latency
        };

        // Throughput score
        let throughput_score = (metrics.tokens_per_second / 100.0).min(1.0);

        // Success rate is direct
        let success_score = metrics.success_rate;

        // Weighted combination
        (latency_score * 0.4 + throughput_score * 0.3 + success_score * 0.3).max(0.0).min(1.0)
    }

    /// Score resource efficiency
    fn score_efficiency(&self, model: &CatalogedModel, context: &SelectionContext) -> f32 {
        // Prefer smaller models (faster, less VRAM) when performance is adequate
        let size_score = match context.preferred_model_size {
            crate::ai_providers::selection::ModelSizePreference::Small => {
                if model.size_gb < 5.0 {
                    1.0
                } else if model.size_gb < 10.0 {
                    0.7
                } else {
                    0.4
                }
            }
            crate::ai_providers::selection::ModelSizePreference::Medium => {
                if model.size_gb >= 5.0 && model.size_gb < 15.0 {
                    1.0
                } else if model.size_gb < 5.0 {
                    0.8
                } else {
                    0.6
                }
            }
            crate::ai_providers::selection::ModelSizePreference::Large => {
                if model.size_gb >= 10.0 {
                    1.0
                } else if model.size_gb >= 5.0 {
                    0.7
                } else {
                    0.4
                }
            }
            crate::ai_providers::selection::ModelSizePreference::Any => 0.8,
        };

        // Context length bonus
        let context_score = if model.context_length >= context.min_context_length {
            1.0
        } else {
            0.5
        };

        // Fast inference bonus
        let fast_inference_bonus = if model.capabilities.contains(&ModelCapability::FastInference) {
            1.1
        } else {
            1.0
        };

        (size_score * 0.6 + context_score * 0.4) * fast_inference_bonus
    }

    /// Score recency (favor recently used models)
    fn score_recency(&self, model: &CatalogedModel) -> f32 {
        match model.last_used {
            Some(last_used) => {
                let elapsed = std::time::SystemTime::now()
                    .duration_since(last_used)
                    .unwrap_or(std::time::Duration::from_secs(u64::MAX))
                    .as_secs();

                // Decay over 1 hour
                let recency = 1.0 - (elapsed as f32 / 3600.0).min(1.0);
                recency * 0.2 + 0.8 // Small bonus for recency
            }
            None => 0.8, // Never used, slight penalty
        }
    }
}

/// Ranking criteria weights
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankingCriteria {
    pub capability_weight: f32,
    pub performance_weight: f32,
    pub availability_weight: f32,
    pub efficiency_weight: f32,
    pub recency_weight: f32,
}

impl Default for RankingCriteria {
    fn default() -> Self {
        Self {
            capability_weight: 0.35,   // Most important: can it do the job?
            performance_weight: 0.25,  // Second: how well does it perform?
            availability_weight: 0.20, // Third: is it reliable?
            efficiency_weight: 0.15,   // Fourth: resource usage
            recency_weight: 0.05,      // Fifth: slight preference for familiar models
        }
    }
}

impl RankingCriteria {
    /// Criteria optimized for constitutional compliance tasks
    pub fn constitutional_compliance() -> Self {
        Self {
            capability_weight: 0.50, // Capability is critical
            performance_weight: 0.15,
            availability_weight: 0.20,
            efficiency_weight: 0.10,
            recency_weight: 0.05,
        }
    }

    /// Criteria optimized for high-throughput tasks
    pub fn high_throughput() -> Self {
        Self {
            capability_weight: 0.20,
            performance_weight: 0.45, // Performance is critical
            availability_weight: 0.20,
            efficiency_weight: 0.10,
            recency_weight: 0.05,
        }
    }

    /// Criteria optimized for resource-constrained environments
    pub fn resource_efficient() -> Self {
        Self {
            capability_weight: 0.25,
            performance_weight: 0.15,
            availability_weight: 0.20,
            efficiency_weight: 0.35, // Efficiency is critical
            recency_weight: 0.05,
        }
    }
}

/// Model score with breakdown
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelScore {
    pub model_id: String,
    pub total_score: f32,
    pub capability_score: f32,
    pub performance_score: f32,
    pub availability_score: f32,
    pub efficiency_score: f32,
    pub recency_score: f32,
    pub breakdown: ScoreBreakdown,
}

/// Detailed score breakdown for analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreBreakdown {
    pub capabilities_matched: f32,
    pub latency_factor: f32,
    pub availability_factor: f32,
    pub size_efficiency: f32,
    pub recency_bonus: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_providers::catalog::{CatalogedModel, PerformanceMetrics};
    use crate::ai_providers::discovery::{ModelSpecialization, ProviderType};
    use crate::ai_providers::selection::{ModelSizePreference, SelectionContext};
    use crate::prompts::types::AgentType;

    fn create_test_model(name: &str, size_gb: f32, capabilities: Vec<ModelCapability>) -> CatalogedModel {
        CatalogedModel {
            id: format!("Ollama::{}", name),
            name: name.to_string(),
            provider_type: ProviderType::Ollama,
            endpoint: "http://localhost:11434".to_string(),
            size_gb,
            context_length: 4096,
            specialization: ModelSpecialization::General,
            capabilities,
            performance_metrics: PerformanceMetrics {
                avg_latency_ms: 150.0,
                tokens_per_second: 20.0,
                success_rate: 0.95,
                total_requests: 100,
            },
            availability_score: 1.0,
            last_used: None,
        }
    }

    #[test]
    fn test_ranker_creation() {
        let ranker = ModelRanker::new();
        assert_eq!(ranker.criteria.capability_weight, 0.35);
    }

    #[test]
    fn test_capability_scoring() {
        let ranker = ModelRanker::new();
        let model = create_test_model(
            "test-model",
            3.0,
            vec![ModelCapability::GeneralConversation, ModelCapability::SafetyAnalysis],
        );

        let context = SelectionContext {
            agent_type: AgentType::Admin,
            required_capabilities: vec![ModelCapability::GeneralConversation],
            preferred_capabilities: vec![ModelCapability::SafetyAnalysis],
            max_latency_ms: 500,
            min_context_length: 2048,
            preferred_model_size: ModelSizePreference::Small,
            requires_math: false,
            requires_coding: false,
            task_type: None,
        };

        let score = ranker.score_capabilities(&model, &context);
        assert!(score > 0.9); // Should have high score with all requirements met
    }

    #[test]
    fn test_missing_required_capability() {
        let ranker = ModelRanker::new();
        let model = create_test_model("test-model", 3.0, vec![ModelCapability::GeneralConversation]);

        let context = SelectionContext {
            agent_type: AgentType::Admin,
            required_capabilities: vec![ModelCapability::CodeGeneration], // Not in model
            preferred_capabilities: vec![],
            max_latency_ms: 500,
            min_context_length: 2048,
            preferred_model_size: ModelSizePreference::Small,
            requires_math: false,
            requires_coding: false,
            task_type: None,
        };

        let score = ranker.score_capabilities(&model, &context);
        assert_eq!(score, 0.0); // Should fail hard on missing required capability
    }

    #[test]
    fn test_efficiency_scoring() {
        let ranker = ModelRanker::new();

        let small_model = create_test_model("small", 3.0, vec![]);
        let large_model = create_test_model("large", 15.0, vec![]);

        let context_prefer_small = SelectionContext {
            agent_type: AgentType::Admin,
            required_capabilities: vec![],
            preferred_capabilities: vec![],
            max_latency_ms: 500,
            min_context_length: 2048,
            preferred_model_size: ModelSizePreference::Small,
            requires_math: false,
            requires_coding: false,
            task_type: None,
        };

        let small_score = ranker.score_efficiency(&small_model, &context_prefer_small);
        let large_score = ranker.score_efficiency(&large_model, &context_prefer_small);

        assert!(small_score > large_score);
    }

    #[test]
    fn test_constitutional_criteria() {
        let criteria = RankingCriteria::constitutional_compliance();
        assert_eq!(criteria.capability_weight, 0.50);
        assert!(criteria.capability_weight > criteria.performance_weight);
    }

    #[test]
    fn test_high_throughput_criteria() {
        let criteria = RankingCriteria::high_throughput();
        assert_eq!(criteria.performance_weight, 0.45);
    }
}
