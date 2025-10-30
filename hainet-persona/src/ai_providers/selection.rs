// START OF FILE hainet-persona/src/ai_providers/selection.rs

//! Model Selection System
//!
//! Selects the optimal model for a specific agent and task based on:
//! - Agent type and requirements
//! - Task characteristics
//! - Model rankings
//! - Fallback strategies

use crate::ai_providers::catalog::ModelCapability;
use crate::ai_providers::ranking::ModelScore;
use crate::prompts::types::AgentType;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Model selector with fallback strategies
pub struct ModelSelector {
    catalog: Arc<RwLock<crate::ai_providers::catalog::ModelCatalog>>,
}

impl ModelSelector {
    /// Create new selector
    pub fn new(catalog: Arc<RwLock<crate::ai_providers::catalog::ModelCatalog>>) -> Self {
        Self { catalog }
    }

    /// Select best model from ranked list
    pub async fn select_best(
        &self,
        ranked_models: &[ModelScore],
        context: &SelectionContext,
    ) -> Result<SelectedModel> {
        if ranked_models.is_empty() {
            return Err(anyhow!("No models available for selection"));
        }

        // Try models in order of ranking
        for (index, score) in ranked_models.iter().enumerate() {
            // Skip if total score is too low
            if score.total_score < context.min_acceptable_score() {
                debug!(
                    "Skipping model {} (score {:.2} < min {:.2})",
                    score.model_id, score.total_score, context.min_acceptable_score()
                );
                continue;
            }

            // Verify model still exists in catalog
            let catalog = self.catalog.read().await;
            if let Some(model) = catalog.get_model(&score.model_id) {
                info!(
                    "Selected model {} (rank {}, score {:.2}) for agent {:?}",
                    score.model_id,
                    index + 1,
                    score.total_score,
                    context.agent_type
                );

                return Ok(SelectedModel {
                    model_id: score.model_id.clone(),
                    model_name: model.name.clone(),
                    endpoint: model.endpoint.clone(),
                    provider_type: model.provider_type,
                    score: score.total_score,
                    rank: index + 1,
                    context: context.clone(),
                });
            }
        }

        Err(anyhow!(
            "No suitable models found for agent {:?} with requirements {:?}",
            context.agent_type,
            context.required_capabilities
        ))
    }
}

/// Selection context for model choice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionContext {
    pub agent_type: AgentType,
    pub required_capabilities: Vec<ModelCapability>,
    pub preferred_capabilities: Vec<ModelCapability>,
    pub max_latency_ms: u64,
    pub min_context_length: usize,
    pub preferred_model_size: ModelSizePreference,
}

impl SelectionContext {
    /// Create context for Guardian agent
    pub fn for_guardian() -> Self {
        Self {
            agent_type: AgentType::Admin,
            required_capabilities: vec![
                ModelCapability::SafetyAnalysis,
                ModelCapability::ConstitutionalCompliance,
            ],
            preferred_capabilities: vec![
                ModelCapability::ContentModeration,
                ModelCapability::InstructionFollowing,
            ],
            max_latency_ms: 500,
            min_context_length: 4096,
            preferred_model_size: ModelSizePreference::Small,
        }
    }

    /// Create context for Admin agent
    pub fn for_admin() -> Self {
        Self {
            agent_type: AgentType::Admin,
            required_capabilities: vec![
                ModelCapability::GeneralConversation,
                ModelCapability::InstructionFollowing,
            ],
            preferred_capabilities: vec![
                ModelCapability::LogicalReasoning,
                ModelCapability::TaskPlanning,
            ],
            max_latency_ms: 1000,
            min_context_length: 8192,
            preferred_model_size: ModelSizePreference::Medium,
        }
    }

    /// Create context for PM agent
    pub fn for_pm() -> Self {
        Self {
            agent_type: AgentType::PM,
            required_capabilities: vec![
                ModelCapability::InstructionFollowing,
                ModelCapability::TaskPlanning,
            ],
            preferred_capabilities: vec![
                ModelCapability::LogicalReasoning,
                ModelCapability::GeneralConversation,
            ],
            max_latency_ms: 800,
            min_context_length: 4096,
            preferred_model_size: ModelSizePreference::Small,
        }
    }

    /// Create context for Worker agent
    pub fn for_worker() -> Self {
        Self {
            agent_type: AgentType::Worker,
            required_capabilities: vec![ModelCapability::InstructionFollowing],
            preferred_capabilities: vec![ModelCapability::FastInference],
            max_latency_ms: 500,
            min_context_length: 2048,
            preferred_model_size: ModelSizePreference::Small,
        }
    }

    /// Minimum acceptable score to consider a model
    pub fn min_acceptable_score(&self) -> f32 {
        match self.agent_type {
            AgentType::Admin => 0.6, // Admin needs high-quality models
            AgentType::PM => 0.5,
            AgentType::Worker => 0.4,
            AgentType::Guardian => 0.7, // Guardian needs highest quality for safety analysis
            AgentType::User => 0.0, // User is human, doesn't use models directly
        }
    }
}

impl SelectionContext {
    /// Get minimum acceptable score
    pub fn min_acceptable_score_value(&self) -> f32 {
        self.min_acceptable_score()
    }
}

/// Model size preference
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelSizePreference {
    Small,  // < 5GB (e.g., Gemma 3 4B)
    Medium, // 5-15GB (e.g., Gemma 3 12B, Llama 3 8B)
    Large,  // > 15GB (e.g., Llama 3 70B)
    Any,    // No preference
}

/// Selected model with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectedModel {
    pub model_id: String,
    pub model_name: String,
    pub endpoint: String,
    pub provider_type: crate::ai_providers::discovery::ProviderType,
    pub score: f32,
    pub rank: usize,
    pub context: SelectionContext,
}

impl SelectedModel {
    /// Get inference URL for this model
    pub fn inference_url(&self) -> String {
        match self.provider_type {
            crate::ai_providers::discovery::ProviderType::Ollama => {
                format!("{}/api/generate", self.endpoint)
            }
            crate::ai_providers::discovery::ProviderType::VLLM => {
                format!("{}/v1/completions", self.endpoint)
            }
            crate::ai_providers::discovery::ProviderType::LiteLLM => {
                format!("{}/v1/completions", self.endpoint)
            }
            crate::ai_providers::discovery::ProviderType::OpenAICompatible => {
                format!("{}/v1/completions", self.endpoint)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guardian_context() {
        let context = SelectionContext::for_guardian();
        assert!(context
            .required_capabilities
            .contains(&ModelCapability::SafetyAnalysis));
        assert_eq!(context.preferred_model_size, ModelSizePreference::Small);
    }

    #[test]
    fn test_admin_context() {
        let context = SelectionContext::for_admin();
        assert!(context
            .required_capabilities
            .contains(&ModelCapability::GeneralConversation));
        assert_eq!(context.preferred_model_size, ModelSizePreference::Medium);
    }

    #[test]
    fn test_min_acceptable_scores() {
        let guardian = SelectionContext::for_guardian();
        let worker = SelectionContext::for_worker();

        assert!(guardian.min_acceptable_score() > worker.min_acceptable_score());
    }

    #[test]
    fn test_inference_url_generation() {
        let selected = SelectedModel {
            model_id: "Ollama::gemma3:4b-it-q4_K_M".to_string(),
            model_name: "gemma3:4b-it-q4_K_M".to_string(),
            endpoint: "http://localhost:11434".to_string(),
            provider_type: crate::ai_providers::discovery::ProviderType::Ollama,
            score: 0.95,
            rank: 1,
            context: SelectionContext::for_guardian(),
        };

        assert_eq!(
            selected.inference_url(),
            "http://localhost:11434/api/generate"
        );
    }

    #[test]
    fn test_size_preferences() {
        assert_ne!(ModelSizePreference::Small, ModelSizePreference::Large);
        assert_eq!(ModelSizePreference::Any, ModelSizePreference::Any);
    }
}
