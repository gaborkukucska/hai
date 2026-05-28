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

    /// Check if a model is a vision model based on its name
    fn is_vision_model(model_id: &str) -> bool {
        let model_lower = model_id.to_lowercase();
        model_lower.contains("vision") || 
        model_lower.contains("vl") || 
        model_lower.contains("clip") ||
        model_lower.contains("llava")
    }

    /// Check if model is specialized for mathematical reasoning
    fn is_math_model(model_id: &str) -> bool {
        model_id.to_lowercase().contains("math")
    }
    
    /// Check if model is specialized for code generation
    fn is_coder_model(model_id: &str) -> bool {
        let lower = model_id.to_lowercase();
        lower.contains("coder") || lower.contains("code")
    }

    /// Check if model belongs to a specific family
    fn matches_family(model_id: &str, family: &str) -> bool {
        let model_lower = model_id.to_lowercase();
        let family_lower = family.to_lowercase();
        
        // Handle "auto" - matches all models
        if family_lower == "auto" {
            return true;
        }
        
        // Match by family name in model ID
        model_lower.contains(&family_lower)
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

        // First pass: Try to find model from preferred family
        if let Some(ref preferred_family) = context.preferred_family {
            if preferred_family != "auto" {
                info!(
                    "Filtering models by preferred family '{}' for agent {:?}",
                    preferred_family, context.agent_type
                );
                
                // Collect all suitable models from the preferred family
                let catalog = self.catalog.read().await;
                let mut candidates: Vec<(&ModelScore, &crate::ai_providers::catalog::CatalogedModel)> = Vec::new();
                
                for (_index, score) in ranked_models.iter().enumerate() {
                    // Check if model matches preferred family
                    if !Self::matches_family(&score.model_id, preferred_family) {
                        debug!(
                            "Skipping model {} (not in preferred family '{}')",
                            score.model_id, preferred_family
                        );
                        continue;
                    }
                    
                    // Skip if total score is too low
                    if score.total_score < context.min_acceptable_score() {
                        debug!(
                            "Skipping model {} (score {:.2} < min {:.2})",
                            score.model_id, score.total_score, context.min_acceptable_score()
                        );
                        continue;
                    }

                    // Skip vision models for text-only tasks
                    if !context.requires_vision() && Self::is_vision_model(&score.model_id) {
                        debug!(
                            "Skipping vision model {} for text-only task",
                            score.model_id
                        );
                        continue;
                    }

                    // Prefer math models for math tasks
                    if context.requires_math && !Self::is_math_model(&score.model_id) {
                        debug!(
                            "Preferring math models for math task, skipping {}",
                            score.model_id
                        );
                        continue;
                    }

                    // Prefer coder models for coding tasks
                    if context.requires_coding && !Self::is_coder_model(&score.model_id) {
                        debug!(
                            "Preferring coder models for coding task, skipping {}",
                            score.model_id
                        );
                        continue;
                    }

                    // Verify model still exists in catalog
                    if let Some(model) = catalog.get_model(&score.model_id) {
                        candidates.push((score, model));
                    }
                }
                
                // If we have candidates, prefer optimal-sized models for balance of speed and quality
                if !candidates.is_empty() {
                    // Sort by size (ascending)
                    candidates.sort_by(|a, b| {
                        a.1.size_gb.partial_cmp(&b.1.size_gb).unwrap_or(std::cmp::Ordering::Equal)
                    });
                    
                    // Find the optimal model based on context preference
                    // Always prefer smaller models for better responsiveness - avoid large models that timeout
                    let optimal_index = match context.preferred_model_size {
                        ModelSizePreference::Small => {
                            // Prefer 2.5-4GB range (e.g., gemma3:4b), avoid tiny models < 2.5GB
                            candidates.iter().position(|(_, m)| m.size_gb >= 2.5 && m.size_gb <= 4.5)
                                .or_else(|| candidates.iter().position(|(_, m)| m.size_gb >= 2.5 && m.size_gb <= 6.0))
                                .unwrap_or(0)
                        },
                        ModelSizePreference::Medium => {
                            // Prefer 4-6GB range (e.g., gemma3:4b, gemma3:9b-q4), prioritize reliability
                            candidates.iter().position(|(_, m)| m.size_gb >= 4.0 && m.size_gb <= 6.0)
                                .or_else(|| candidates.iter().position(|(_, m)| m.size_gb >= 2.5 && m.size_gb <= 4.0))
                                .or_else(|| candidates.iter().position(|(_, m)| m.size_gb >= 6.0 && m.size_gb <= 8.0))
                                .unwrap_or(0)
                        },
                        ModelSizePreference::Large => {
                            // For "large", still cap at 8GB to avoid timeouts, prefer 6-8GB range
                            candidates.iter().position(|(_, m)| m.size_gb >= 6.0 && m.size_gb <= 8.0)
                                .or_else(|| candidates.iter().position(|(_, m)| m.size_gb >= 4.0 && m.size_gb <= 6.0))
                                .unwrap_or_else(|| candidates.len() - 1)
                        },
                        ModelSizePreference::Any => {
                            // Prefer sweet spot (3-5GB) as default for best balance
                            candidates.iter().position(|(_, m)| m.size_gb >= 3.0 && m.size_gb <= 5.0)
                                .or_else(|| candidates.iter().position(|(_, m)| m.size_gb >= 2.5 && m.size_gb <= 6.0))
                                .unwrap_or(0)
                        }
                    };
                    
                    let (selected_score, selected_model) = candidates[optimal_index];
                    let rank = ranked_models.iter().position(|s| s.model_id == selected_score.model_id).map(|p| p + 1).unwrap_or(0);
                    
                    info!(
                        "Selected model {} from preferred family '{}' (rank {}, score {:.2}, size {:.1}GB) for agent {:?}",
                        selected_score.model_id,
                        preferred_family,
                        rank,
                        selected_score.total_score,
                        selected_model.size_gb,
                        context.agent_type
                    );

                    return Ok(SelectedModel {
                        model_id: selected_score.model_id.clone(),
                        model_name: selected_model.name.clone(),
                        endpoint: selected_model.endpoint.clone(),
                        provider_type: selected_model.provider_type,
                        score: selected_score.total_score,
                        rank,
                        context: context.clone(),
                        hardware_max_ctx: 0,
                    });
                }
                
                // No suitable model found in preferred family
                if !context.allow_fallback {
                    return Err(anyhow!(
                        "No suitable models found in preferred family '{}' for agent {:?}, and fallback is disabled",
                        preferred_family,
                        context.agent_type
                    ));
                }
                
                info!(
                    "No suitable model found in preferred family '{}', falling back to all models",
                    preferred_family
                );
            }
        }

        // Second pass: Try all models (fallback or no preference)
        for (index, score) in ranked_models.iter().enumerate() {
            // Skip if total score is too low
            if score.total_score < context.min_acceptable_score() {
                debug!(
                    "Skipping model {} (score {:.2} < min {:.2})",
                    score.model_id, score.total_score, context.min_acceptable_score()
                );
                continue;
            }

            // Skip vision models for text-only tasks
            if !context.requires_vision() && Self::is_vision_model(&score.model_id) {
                debug!(
                    "Skipping vision model {} for text-only task",
                    score.model_id
                );
                continue;
            }

            // Prefer math models for math tasks
            if context.requires_math && !Self::is_math_model(&score.model_id) {
                debug!(
                    "Preferring math models for math task, skipping {}",
                    score.model_id
                );
                continue;
            }

            // Prefer coder models for coding tasks
            if context.requires_coding && !Self::is_coder_model(&score.model_id) {
                debug!(
                    "Preferring coder models for coding task, skipping {}",
                    score.model_id
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
                    hardware_max_ctx: 0,
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

/// Task type for specialized model selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskType {
    General,
    FileOperation,
    CodeGeneration,
    CodeAnalysis,
    MathematicalComputation,
    DataAnalysis,
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
    pub requires_math: bool,
    pub requires_coding: bool,
    pub task_type: Option<TaskType>,
    /// User's preferred model family (e.g., "gemma3", "llama3", "qwen")
    /// If Some, only models from this family will be considered
    /// If None or "auto", all models are considered
    pub preferred_family: Option<String>,
    /// Whether to allow fallback to other families if preferred family has no suitable models
    pub allow_fallback: bool,
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
            requires_math: false,
            requires_coding: false,
            task_type: None,
            preferred_family: None,
            allow_fallback: true,
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
            requires_math: false,
            requires_coding: false,
            task_type: None,
            preferred_family: None,
            allow_fallback: true,
        }
    }

    /// Create context for Worker agent doing coding tasks
    pub fn for_worker_coding() -> Self {
        Self {
            agent_type: AgentType::Worker,
            required_capabilities: vec![
                ModelCapability::InstructionFollowing,
                ModelCapability::CodeGeneration,
            ],
            preferred_capabilities: vec![
                ModelCapability::CodeAnalysis,
                ModelCapability::FastInference,
            ],
            max_latency_ms: 500,
            min_context_length: 4096,
            preferred_model_size: ModelSizePreference::Small,
            requires_math: false,
            requires_coding: true,
            task_type: Some(TaskType::CodeGeneration),
            preferred_family: None,
            allow_fallback: true,
        }
    }
    
    /// Create context for Worker agent doing math tasks
    pub fn for_worker_math() -> Self {
        Self {
            agent_type: AgentType::Worker,
            required_capabilities: vec![
                ModelCapability::InstructionFollowing,
                ModelCapability::MathematicalReasoning,
            ],
            preferred_capabilities: vec![ModelCapability::FastInference],
            max_latency_ms: 500,
            min_context_length: 2048,
            preferred_model_size: ModelSizePreference::Small,
            requires_math: true,
            requires_coding: false,
            task_type: Some(TaskType::MathematicalComputation),
            preferred_family: None,
            allow_fallback: true,
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
            requires_math: false,
            requires_coding: false,
            task_type: None,
            preferred_family: None,
            allow_fallback: true,
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
            requires_math: false,
            requires_coding: false,
            task_type: None,
            preferred_family: None,
            allow_fallback: true,
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

    /// Check if this context requires vision capabilities
    pub fn requires_vision(&self) -> bool {
        self.required_capabilities.contains(&ModelCapability::VisionUnderstanding) ||
        self.preferred_capabilities.contains(&ModelCapability::VisionUnderstanding)
    }
    
    /// Set preferred model family for this context
    pub fn with_preferred_family(mut self, family: Option<String>) -> Self {
        self.preferred_family = family;
        self
    }
    
    /// Set fallback behavior for this context
    pub fn with_fallback(mut self, allow_fallback: bool) -> Self {
        self.allow_fallback = allow_fallback;
        self
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
    #[serde(default)]
    pub hardware_max_ctx: usize,
}

impl SelectedModel {
    /// Get a client for the selected provider
    pub fn get_client(&self) -> Result<Box<dyn crate::ai_providers::ProviderClient>> {
        match self.provider_type {
            crate::ai_providers::discovery::ProviderType::Ollama => {
                Ok(Box::new(crate::ai_providers::providers::OllamaClient::new(
                    self.endpoint.clone(),
                    self.context.min_context_length,
                    self.hardware_max_ctx
                )))
            }
            _ => Err(anyhow::anyhow!("Provider type not yet supported for client creation")),
        }
    }
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

    use crate::test_utils::TEST_OLLAMA_ENDPOINT;

    #[test]
    fn test_inference_url_generation() {
        let selected = SelectedModel {
            model_id: "Ollama::gemma3:4b-it-q4_K_M".to_string(),
            model_name: "gemma3:4b-it-q4_K_M".to_string(),
            endpoint: TEST_OLLAMA_ENDPOINT.to_string(),
            provider_type: crate::ai_providers::discovery::ProviderType::Ollama,
            score: 0.95,
            rank: 1,
            context: SelectionContext::for_guardian(),
        };

        assert_eq!(
            selected.inference_url(),
            format!("{}/api/generate", TEST_OLLAMA_ENDPOINT)
        );
    }

    #[test]
    fn test_size_preferences() {
        assert_ne!(ModelSizePreference::Small, ModelSizePreference::Large);
        assert_eq!(ModelSizePreference::Any, ModelSizePreference::Any);
    }
}
