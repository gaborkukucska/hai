// START OF FILE hainet-persona/src/guardian/harm_analyzer.rs

//! Harm Analyzer Module
//!
//! Context-aware harm detection using both rule-based and ML-powered approaches.
//! Analyzes toxicity, intent classification, and risk levels.

use crate::guardian::ollama_client::{GuardianOllamaClient, HarmAnalysisResult, RiskLevel};

// Re-export Intent for test usage
pub use crate::guardian::ollama_client::Intent;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

/// Harm analyzer with context-aware toxicity scoring
pub struct HarmAnalyzer {
    llm_client: Option<GuardianOllamaClient>,
}

impl HarmAnalyzer {
    /// Create new harm analyzer (LLM client optional for rule-based mode)
    pub fn new(llm_client: Option<GuardianOllamaClient>) -> Self {
        Self { llm_client }
    }

    /// Analyze text for harmful content
    pub async fn analyze(&self, text: &str, context: &AnalysisContext) -> Result<HarmReport> {
        debug!("Analyzing text for harm: {} chars", text.len());

        // Rule-based detection (always runs)
        let rule_based = self.rule_based_detection(text)?;

        // ML-based detection (if LLM client available)
        let ml_based = if let Some(client) = &self.llm_client {
            Some(client.analyze_harm(text).await?)
        } else {
            None
        };

        // Combine results
        let combined = self.combine_results(&rule_based, &ml_based, context);

        info!(
            "Harm analysis complete: risk={:?}, toxicity={:.2}",
            combined.risk_level, combined.toxicity_score
        );

        Ok(combined)
    }

    /// Rule-based harm detection using keyword matching
    fn rule_based_detection(&self, text: &str) -> Result<HarmReport> {
        let text_lower = text.to_lowercase();

        let mut harm_types = Vec::new();
        let mut toxicity_score: f32 = 0.0;

        // Violence keywords
        if text_lower.contains("kill")
            || text_lower.contains("murder")
            || text_lower.contains("hurt")
            || text_lower.contains("attack")
        {
            harm_types.push("violence".to_string());
            toxicity_score += 0.3;
        }

        // Hate speech keywords
        if text_lower.contains("hate")
            || text_lower.contains("racist")
            || text_lower.contains("sexist")
        {
            harm_types.push("hate_speech".to_string());
            toxicity_score += 0.4;
        }

        // Self-harm keywords
        if text_lower.contains("suicide")
            || text_lower.contains("self-harm")
            || text_lower.contains("end my life")
        {
            harm_types.push("self_harm".to_string());
            toxicity_score += 0.5;
        }

        // Sexual content keywords
        if text_lower.contains("sexual")
            || text_lower.contains("explicit")
            || text_lower.contains("nsfw")
        {
            harm_types.push("sexual".to_string());
            toxicity_score += 0.2;
        }

        // Illegal activity keywords
        if text_lower.contains("illegal")
            || text_lower.contains("drugs")
            || text_lower.contains("weapon")
        {
            harm_types.push("illegal".to_string());
            toxicity_score += 0.3;
        }

        // Cap toxicity score at 1.0
        toxicity_score = toxicity_score.min(1.0);

        let contains_harm = !harm_types.is_empty();
        let intent = self.classify_intent(toxicity_score, &harm_types);
        let risk_level = self.classify_risk(toxicity_score, &harm_types);

        Ok(HarmReport {
            contains_harm,
            harm_types,
            toxicity_score,
            intent,
            risk_level,
            confidence: 0.6, // Rule-based has moderate confidence
            details: "Rule-based detection".to_string(),
        })
    }

    /// Combine rule-based and ML-based results
    fn combine_results(
        &self,
        rule_based: &HarmReport,
        ml_based: &Option<HarmAnalysisResult>,
        context: &AnalysisContext,
    ) -> HarmReport {
        if let Some(ml) = ml_based {
            // ML result available - combine both
            let combined_toxicity = (rule_based.toxicity_score * 0.3 + ml.toxicity_score * 0.7)
                .max(rule_based.toxicity_score)
                .max(ml.toxicity_score);

            let mut combined_harm_types = rule_based.harm_types.clone();
            for ht in &ml.harm_types {
                if !combined_harm_types.contains(ht) {
                    combined_harm_types.push(ht.clone());
                }
            }

            let contains_harm = rule_based.contains_harm || ml.contains_harm;
            let risk_level = Self::max_risk(rule_based.risk_level, ml.risk_level);
            let intent = Self::max_intent(rule_based.intent, ml.intent);

            HarmReport {
                contains_harm,
                harm_types: combined_harm_types,
                toxicity_score: combined_toxicity,
                intent,
                risk_level,
                confidence: 0.9, // Combined has high confidence
                details: format!(
                    "Rule-based + ML detection (context: {})",
                    context.conversation_id
                ),
            }
        } else {
            // Only rule-based available
            rule_based.clone()
        }
    }

    /// Classify intent based on toxicity and harm types
    fn classify_intent(&self, toxicity_score: f32, harm_types: &[String]) -> Intent {
        if harm_types.contains(&"self_harm".to_string()) {
            Intent::Emergency
        } else if toxicity_score >= 0.7 {
            Intent::Malicious
        } else if toxicity_score >= 0.4 {
            Intent::Concerning
        } else {
            Intent::Benign
        }
    }

    /// Classify risk level based on toxicity and harm types
    fn classify_risk(&self, toxicity_score: f32, harm_types: &[String]) -> RiskLevel {
        if harm_types.contains(&"self_harm".to_string()) {
            RiskLevel::Critical
        } else if toxicity_score >= 0.8 {
            RiskLevel::High
        } else if toxicity_score >= 0.5 {
            RiskLevel::Medium
        } else if toxicity_score >= 0.2 {
            RiskLevel::Low
        } else {
            RiskLevel::None
        }
    }

    /// Get maximum risk level
    fn max_risk(a: RiskLevel, b: RiskLevel) -> RiskLevel {
        match (a, b) {
            (RiskLevel::Critical, _) | (_, RiskLevel::Critical) => RiskLevel::Critical,
            (RiskLevel::High, _) | (_, RiskLevel::High) => RiskLevel::High,
            (RiskLevel::Medium, _) | (_, RiskLevel::Medium) => RiskLevel::Medium,
            (RiskLevel::Low, _) | (_, RiskLevel::Low) => RiskLevel::Low,
            _ => RiskLevel::None,
        }
    }

    /// Get maximum intent severity
    fn max_intent(a: Intent, b: Intent) -> Intent {
        match (a, b) {
            (Intent::Emergency, _) | (_, Intent::Emergency) => Intent::Emergency,
            (Intent::Malicious, _) | (_, Intent::Malicious) => Intent::Malicious,
            (Intent::Concerning, _) | (_, Intent::Concerning) => Intent::Concerning,
            _ => Intent::Benign,
        }
    }
}

/// Analysis context with conversation history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisContext {
    pub conversation_id: String,
    pub message_count: usize,
    pub previous_violations: usize,
}

impl Default for AnalysisContext {
    fn default() -> Self {
        Self {
            conversation_id: "default".to_string(),
            message_count: 0,
            previous_violations: 0,
        }
    }
}

/// Harm analysis report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarmReport {
    pub contains_harm: bool,
    pub harm_types: Vec<String>,
    pub toxicity_score: f32,
    pub intent: Intent,
    pub risk_level: RiskLevel,
    pub confidence: f32,
    pub details: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rule_based_violence_detection() {
        let analyzer = HarmAnalyzer::new(None);
        let context = AnalysisContext::default();

        let report = analyzer
            .analyze("I want to kill someone", &context)
            .await
            .unwrap();

        assert!(report.contains_harm);
        assert!(report.harm_types.contains(&"violence".to_string()));
        assert!(report.toxicity_score > 0.0);
    }

    #[tokio::test]
    async fn test_rule_based_hate_speech_detection() {
        let analyzer = HarmAnalyzer::new(None);
        let context = AnalysisContext::default();

        let report = analyzer
            .analyze("I hate this racist person", &context)
            .await
            .unwrap();

        assert!(report.contains_harm);
        assert!(report.harm_types.contains(&"hate_speech".to_string()));
    }

    #[tokio::test]
    async fn test_rule_based_self_harm_detection() {
        let analyzer = HarmAnalyzer::new(None);
        let context = AnalysisContext::default();

        let report = analyzer
            .analyze("I want to commit suicide", &context)
            .await
            .unwrap();

        assert!(report.contains_harm);
        assert!(report.harm_types.contains(&"self_harm".to_string()));
        assert_eq!(report.intent, Intent::Emergency);
        assert_eq!(report.risk_level, RiskLevel::Critical);
    }

    #[tokio::test]
    async fn test_benign_text() {
        let analyzer = HarmAnalyzer::new(None);
        let context = AnalysisContext::default();

        let report = analyzer
            .analyze("Hello, how are you today?", &context)
            .await
            .unwrap();

        assert!(!report.contains_harm);
        assert_eq!(report.toxicity_score, 0.0);
        assert_eq!(report.intent, Intent::Benign);
        assert_eq!(report.risk_level, RiskLevel::None);
    }

    #[test]
    fn test_intent_classification() {
        let analyzer = HarmAnalyzer::new(None);

        let intent_low = analyzer.classify_intent(0.3, &[]);
        assert_eq!(intent_low, Intent::Benign);

        let intent_high = analyzer.classify_intent(0.8, &[]);
        assert_eq!(intent_high, Intent::Malicious);

        let intent_emergency = analyzer.classify_intent(0.5, &["self_harm".to_string()]);
        assert_eq!(intent_emergency, Intent::Emergency);
    }

    #[test]
    fn test_risk_classification() {
        let analyzer = HarmAnalyzer::new(None);

        let risk_low = analyzer.classify_risk(0.1, &[]);
        assert_eq!(risk_low, RiskLevel::None);

        let risk_high = analyzer.classify_risk(0.9, &[]);
        assert_eq!(risk_high, RiskLevel::High);

        let risk_critical = analyzer.classify_risk(0.5, &["self_harm".to_string()]);
        assert_eq!(risk_critical, RiskLevel::Critical);
    }
}
