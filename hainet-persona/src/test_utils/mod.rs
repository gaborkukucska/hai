//! # START OF FILE hainet-persona/src/test_utils/mod.rs
//! Test Utilities and Production-Ready Parsing Helpers
//! 
//! This module provides robust utilities for:
//! - Multi-strategy JSON parsing with fallback mechanisms
//! - Schema validation for structured LLM outputs
//! - Retry logic with exponential backoff
//! - Error categorization and analysis
//! 
//! Originally developed for test infrastructure, these utilities are
//! production-ready and used throughout the agent system for reliable
//! LLM output processing.

pub mod json_validator;
pub mod retry;

// Re-export commonly used types
pub use json_validator::{
    JSONValidator,
    ParsingStrategy,
    ParseResult,
    ProjectPlanSchema,
    TaskDecompositionSchema,
    SchemaValidator,
};

pub use retry::{
    RetryConfig,
    retry_with_validation,
    FailureCategory,
};

#[cfg(test)]
pub const TEST_OLLAMA_ENDPOINT: &str = "http://localhost:11434";

#[cfg(test)]
pub fn create_test_model_info(name: &str, size_gb: f32) -> crate::ai_providers::discovery::ModelInfo {
    use crate::ai_providers::discovery::{ModelInfo, ModelSpecialization, ProviderType};
    ModelInfo {
        name: name.to_string(),
        provider_type: ProviderType::Ollama,
        endpoint: TEST_OLLAMA_ENDPOINT.to_string(),
        size_gb,
        context_length: 4096,
        specialization: ModelSpecialization::General,
    }
}

#[cfg(test)]
pub fn create_test_cataloged_model(name: &str, size_gb: f32, capabilities: Vec<crate::ai_providers::catalog::ModelCapability>) -> crate::ai_providers::catalog::CatalogedModel {
    use crate::ai_providers::catalog::{CatalogedModel, PerformanceMetrics};
    use crate::ai_providers::discovery::{ModelSpecialization, ProviderType};

    CatalogedModel {
        id: format!("Ollama::{}", name),
        name: name.to_string(),
        provider_type: ProviderType::Ollama,
        endpoint: TEST_OLLAMA_ENDPOINT.to_string(),
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
