// START OF FILE hainet-persona/src/guardian/mod.rs

//! Constitutional Guardian System
//!
//! This module implements the independent constitutional compliance monitoring system
//! as required by Article V of the HAI-Net Constitution. The Guardian system monitors
//! all agent communications for privacy violations, bias, and potential harm.

pub mod ollama_client;
pub mod pii_detector;
pub mod bias_detector;
pub mod harm_analyzer;
pub mod decision_engine;

// Re-export key types from each module
pub use ollama_client::{
    GuardianOllamaClient, PiiAnalysisResult, BiasAnalysisResult, HarmAnalysisResult,
    RiskLevel, Severity, Intent,
};
pub use pii_detector::{PIIDetector, PiiReport};
pub use bias_detector::{BiasDetector, BiasReport};
pub use harm_analyzer::{HarmAnalyzer, HarmReport, AnalysisContext};
pub use decision_engine::{
    DecisionEngine, GuardianDecision, GuardianAction, DecisionConfig,
    Violation, ViolationCategory,
};

use anyhow::Result;
use crate::ai_providers::AIProviderManager;
use std::sync::Arc;

/// Complete Guardian system orchestrating all constitutional compliance checks
pub struct GuardianSystem {
    _ollama_client: Option<GuardianOllamaClient>,
    pii_detector: PIIDetector,
    bias_detector: BiasDetector,
    harm_analyzer: HarmAnalyzer,
    decision_engine: DecisionEngine,
}

impl GuardianSystem {
    /// Create new Guardian system with Ollama client (optional for rule-based mode)
    pub fn new(ai_provider_manager: Arc<AIProviderManager>, model: Option<String>) -> Self {
        let ollama_client = if let Some(model_name) = model {
            Some(GuardianOllamaClient::new(ai_provider_manager, model_name))
        } else {
            None
        };
        
        Self {
            _ollama_client: ollama_client.clone(),
            pii_detector: PIIDetector::new(ollama_client.clone()),
            bias_detector: BiasDetector::new(ollama_client.clone()),
            harm_analyzer: HarmAnalyzer::new(ollama_client.clone()),
            decision_engine: DecisionEngine::new(),
        }
    }
    
    /// Analyze a message for constitutional compliance
    pub async fn analyze_message(
        &self,
        message_text: &str,
        conversation_history: &[String],
    ) -> Result<GuardianDecision> {
        let context = AnalysisContext {
            conversation_id: "default".to_string(),
            message_count: conversation_history.len(),
            previous_violations: 0,
        };

        // Run all detectors
        let pii_result = self.pii_detector.analyze(message_text).await?;
        let bias_result = self.bias_detector.analyze(message_text).await?;
        let harm_result = self.harm_analyzer.analyze(message_text, &context).await?;
        
        // Make decision based on all results
        let decision = self.decision_engine.make_decision(&pii_result, &bias_result, &harm_result);
        
        Ok(decision)
    }
}
