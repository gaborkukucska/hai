// START OF FILE hainet-persona/src/ai_providers/catalog.rs

//! Model Catalog and Capability Database
//!
//! Maintains a comprehensive database of all discovered AI models with their
//! capabilities, performance metrics, and suitability for different agent types.

use crate::ai_providers::discovery::{ModelInfo, ModelSpecialization, ProviderType};
use crate::prompts::types::AgentType;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::debug;

/// Comprehensive model catalog
pub struct ModelCatalog {
    models: HashMap<String, CatalogedModel>,
}

impl ModelCatalog {
    /// Create new empty catalog
    pub fn new() -> Self {
        Self {
            models: HashMap::new(),
        }
    }

    /// Add a model to the catalog
    pub fn add_model(&mut self, model_info: ModelInfo) {
        let capabilities = Self::infer_capabilities(&model_info);
        
        let cataloged = CatalogedModel {
            id: format!("{}::{}", model_info.provider_type.to_string(), model_info.name),
            name: model_info.name.clone(),
            provider_type: model_info.provider_type,
            endpoint: model_info.endpoint.clone(),
            size_gb: model_info.size_gb,
            context_length: model_info.context_length,
            specialization: model_info.specialization,
            capabilities,
            performance_metrics: PerformanceMetrics::default(),
            availability_score: 1.0, // Start optimistic
            last_used: None,
        };

        debug!("Cataloging model: {} ({})", cataloged.id, cataloged.capabilities.len());
        self.models.insert(cataloged.id.clone(), cataloged);
    }

    /// Get model by ID
    pub fn get_model(&self, model_id: &str) -> Option<&CatalogedModel> {
        self.models.get(model_id)
    }

    /// Get all models
    pub fn all_models(&self) -> Vec<&CatalogedModel> {
        self.models.values().collect()
    }

    /// Get models by capability
    pub fn models_with_capability(&self, capability: ModelCapability) -> Vec<&CatalogedModel> {
        self.models
            .values()
            .filter(|m| m.capabilities.contains(&capability))
            .collect()
    }

    /// Get models by specialization
    pub fn models_with_specialization(
        &self,
        specialization: ModelSpecialization,
    ) -> Vec<&CatalogedModel> {
        self.models
            .values()
            .filter(|m| m.specialization == specialization)
            .collect()
    }

    /// Get models suitable for agent type
    pub fn models_for_agent(&self, agent_type: &AgentType) -> Vec<&CatalogedModel> {
        let required_capabilities = Self::agent_requirements(agent_type);

        self.models
            .values()
            .filter(|m| {
                required_capabilities
                    .iter()
                    .all(|cap| m.capabilities.contains(cap))
            })
            .collect()
    }

    /// Update performance metrics for a model
    pub fn update_metrics(&mut self, model_id: &str, metrics: PerformanceMetrics) {
        if let Some(model) = self.models.get_mut(model_id) {
            model.performance_metrics = metrics;
            model.last_used = Some(std::time::SystemTime::now());
        }
    }

    /// Update availability score
    pub fn update_availability(&mut self, model_id: &str, success: bool) {
        if let Some(model) = self.models.get_mut(model_id) {
            // Exponential moving average
            let alpha = 0.3;
            let new_score = if success { 1.0 } else { 0.0 };
            model.availability_score = alpha * new_score + (1.0 - alpha) * model.availability_score;
        }
    }

    /// Clear all models
    pub fn clear(&mut self) {
        self.models.clear();
    }

    /// Get catalog statistics
    pub fn get_stats(&self) -> CatalogStats {
        let total_models = self.models.len();
        let providers = self
            .models
            .values()
            .map(|m| m.provider_type)
            .collect::<std::collections::HashSet<_>>()
            .len();

        let avg_model_size_gb = if total_models > 0 {
            self.models.values().map(|m| m.size_gb).sum::<f32>() / total_models as f32
        } else {
            0.0
        };

        let capabilities = self
            .models
            .values()
            .flat_map(|m| &m.capabilities)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .map(|c| format!("{:?}", c))
            .collect();

        CatalogStats {
            total_models,
            providers,
            avg_model_size_gb,
            capabilities,
        }
    }

    /// Count models in catalog
    pub fn model_count(&self) -> usize {
        self.models.len()
    }

    /// Infer capabilities from model metadata
    fn infer_capabilities(model_info: &ModelInfo) -> Vec<ModelCapability> {
        let mut capabilities = Vec::new();

        let name_lower = model_info.name.to_lowercase();

        // Infer from specialization
        match model_info.specialization {
            ModelSpecialization::Code => {
                capabilities.push(ModelCapability::CodeGeneration);
                capabilities.push(ModelCapability::ProgrammingAssistance);
            }
            ModelSpecialization::Math => {
                capabilities.push(ModelCapability::MathematicalReasoning);
            }
            ModelSpecialization::Safety => {
                capabilities.push(ModelCapability::SafetyAnalysis);
                capabilities.push(ModelCapability::ContentModeration);
            }
            ModelSpecialization::Reasoning => {
                capabilities.push(ModelCapability::LogicalReasoning);
            }
            ModelSpecialization::Creative => {
                capabilities.push(ModelCapability::CreativeWriting);
            }
            ModelSpecialization::General => {
                capabilities.push(ModelCapability::GeneralConversation);
            }
        }

        // Infer from model name patterns
        if name_lower.contains("it") || name_lower.contains("instruct") {
            capabilities.push(ModelCapability::InstructionFollowing);
            capabilities.push(ModelCapability::SafetyAnalysis);
        }

        if name_lower.contains("code") {
            capabilities.push(ModelCapability::CodeGeneration);
        }

        if name_lower.contains("gemma") {
            capabilities.push(ModelCapability::SafetyAnalysis);
            capabilities.push(ModelCapability::ConstitutionalCompliance);
        }

        if name_lower.contains("llama") || name_lower.contains("qwen") {
            capabilities.push(ModelCapability::GeneralConversation);
            capabilities.push(ModelCapability::LogicalReasoning);
        }

        // Context length based capabilities
        if model_info.context_length >= 8192 {
            capabilities.push(ModelCapability::LongContext);
        }

        // Size-based capabilities
        if model_info.size_gb < 5.0 {
            capabilities.push(ModelCapability::FastInference);
        }

        // Vision model detection
        if name_lower.contains("vision") || 
           name_lower.contains("vl") || 
           name_lower.contains("clip") ||
           name_lower.contains("llava") {
            capabilities.push(ModelCapability::VisionUnderstanding);
        }

        // Deduplicate
        capabilities.sort();
        capabilities.dedup();

        capabilities
    }

    /// Get required capabilities for agent type
    fn agent_requirements(agent_type: &AgentType) -> Vec<ModelCapability> {
        match agent_type {
            AgentType::User => vec![],
            AgentType::Admin => vec![
                ModelCapability::GeneralConversation,
                ModelCapability::InstructionFollowing,
                ModelCapability::LogicalReasoning,
            ],
            AgentType::PM => vec![
                ModelCapability::InstructionFollowing,
                ModelCapability::TaskPlanning,
            ],
            AgentType::Worker => vec![ModelCapability::InstructionFollowing],
            AgentType::Guardian => vec![
                ModelCapability::SafetyAnalysis,
                ModelCapability::LogicalReasoning,
                ModelCapability::ConstitutionalCompliance,
            ],
        }
    }
}

/// Model with full catalog metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogedModel {
    pub id: String,
    pub name: String,
    pub provider_type: ProviderType,
    pub endpoint: String,
    pub size_gb: f32,
    pub context_length: usize,
    pub specialization: ModelSpecialization,
    pub capabilities: Vec<ModelCapability>,
    pub performance_metrics: PerformanceMetrics,
    pub availability_score: f32,
    pub last_used: Option<std::time::SystemTime>,
}

/// Model capabilities
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ModelCapability {
    GeneralConversation,
    InstructionFollowing,
    CodeGeneration,
    MathematicalReasoning,
    LogicalReasoning,
    CreativeWriting,
    SafetyAnalysis,
    ContentModeration,
    ConstitutionalCompliance,
    ProgrammingAssistance,
    TaskPlanning,
    LongContext,
    FastInference,
    VisionUnderstanding,
}

/// Performance metrics for a model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub avg_latency_ms: f32,
    pub tokens_per_second: f32,
    pub success_rate: f32,
    pub total_requests: u64,
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self {
            avg_latency_ms: 0.0,
            tokens_per_second: 0.0,
            success_rate: 1.0,
            total_requests: 0,
        }
    }
}

/// Catalog statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogStats {
    pub total_models: usize,
    pub providers: usize,
    pub avg_model_size_gb: f32,
    pub capabilities: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_model(name: &str, size_gb: f32) -> ModelInfo {
        ModelInfo {
            name: name.to_string(),
            provider_type: ProviderType::Ollama,
            endpoint: "http://localhost:11434".to_string(),
            size_gb,
            context_length: 4096,
            specialization: ModelSpecialization::General,
        }
    }

    #[test]
    fn test_catalog_creation() {
        let catalog = ModelCatalog::new();
        assert_eq!(catalog.model_count(), 0);
    }

    #[test]
    fn test_add_model() {
        let mut catalog = ModelCatalog::new();
        let model = create_test_model("gemma3:4b-it-q4_K_M", 3.3);
        
        catalog.add_model(model);
        assert_eq!(catalog.model_count(), 1);
    }

    #[test]
    fn test_capability_inference() {
        let mut catalog = ModelCatalog::new();
        let model = create_test_model("gemma3:4b-it-q4_K_M", 3.3);
        
        catalog.add_model(model);
        
        let models = catalog.models_with_capability(ModelCapability::SafetyAnalysis);
        assert!(!models.is_empty());
    }

    #[test]
    fn test_agent_requirements() {
        let requirements = ModelCatalog::agent_requirements(&AgentType::Admin);
        assert!(requirements.contains(&ModelCapability::GeneralConversation));
        assert!(requirements.contains(&ModelCapability::LogicalReasoning));
    }

    #[test]
    fn test_availability_tracking() {
        let mut catalog = ModelCatalog::new();
        let model = create_test_model("test-model", 3.0);
        catalog.add_model(model);
        
        let model_id = "Ollama::test-model";
        
        // Initial availability should be 1.0
        assert_eq!(catalog.get_model(model_id).unwrap().availability_score, 1.0);
        
        // Update with failure
        catalog.update_availability(model_id, false);
        let score = catalog.get_model(model_id).unwrap().availability_score;
        assert!(score < 1.0);
    }

    #[test]
    fn test_performance_metrics_update() {
        let mut catalog = ModelCatalog::new();
        let model = create_test_model("test-model", 3.0);
        catalog.add_model(model);
        
        let model_id = "Ollama::test-model";
        let metrics = PerformanceMetrics {
            avg_latency_ms: 150.0,
            tokens_per_second: 20.0,
            success_rate: 0.95,
            total_requests: 100,
        };
        
        catalog.update_metrics(model_id, metrics);
        
        let updated = catalog.get_model(model_id).unwrap();
        assert_eq!(updated.performance_metrics.avg_latency_ms, 150.0);
        assert!(updated.last_used.is_some());
    }

    #[test]
    fn test_catalog_stats() {
        let mut catalog = ModelCatalog::new();
        catalog.add_model(create_test_model("model1", 3.0));
        catalog.add_model(create_test_model("model2", 5.0));
        
        let stats = catalog.get_stats();
        assert_eq!(stats.total_models, 2);
        assert_eq!(stats.avg_model_size_gb, 4.0);
    }
}
