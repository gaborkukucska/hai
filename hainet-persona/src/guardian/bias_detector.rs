// START OF FILE hainet-persona/src/guardian/bias_detector.rs

//! Bias Detection System
//!
//! Implements bias and stereotype detection to enforce Article II (Human Rights Protection)
//! of the HAI-Net Constitution.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::debug;

/// Bias report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BiasReport {
    pub contains_bias: bool,
    pub bias_categories: Vec<String>,
    pub severity: Severity,
    pub fairness_metrics: FairnessMetrics,
}

/// Severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    None,
    Low,
    Medium,
    High,
    Critical,
}

/// Fairness metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FairnessMetrics {
    pub gender_score: f32,
    pub age_score: f32,
    pub ethnicity_score: f32,
}

/// Bias detector
pub struct BiasDetector {
    _llm_client: Option<crate::guardian::ollama_client::GuardianOllamaClient>,
}

impl BiasDetector {
    pub fn new(llm_client: Option<crate::guardian::ollama_client::GuardianOllamaClient>) -> Self {
        Self { _llm_client: llm_client }
    }

    pub async fn analyze(&self, text: &str) -> Result<BiasReport> {
        debug!("Analyzing text for bias: {} chars", text.len());

        let mut bias_categories = Vec::new();
        let text_lower = text.to_lowercase();

        // Rule-based stereotype detection
        if text_lower.contains("women") && (text_lower.contains("emotional") || text_lower.contains("weak")) {
            bias_categories.push("gender".to_string());
        }

        if text_lower.contains("old") && text_lower.contains("slow") {
            bias_categories.push("age".to_string());
        }

        if text_lower.contains("disabled") && text_lower.contains("unable") {
            bias_categories.push("disability".to_string());
        }

        let contains_bias = !bias_categories.is_empty();
        let severity = if bias_categories.len() >= 2 {
            Severity::High
        } else if contains_bias {
            Severity::Medium
        } else {
            Severity::None
        };

        Ok(BiasReport {
            contains_bias,
            bias_categories,
            severity,
            fairness_metrics: FairnessMetrics::default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_no_bias() {
        let detector = BiasDetector::new(None);
        let result = detector.analyze("This is neutral text").await.unwrap();
        assert!(!result.contains_bias);
    }
}
