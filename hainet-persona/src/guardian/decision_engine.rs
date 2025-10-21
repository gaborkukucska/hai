// START OF FILE hainet-persona/src/guardian/decision_engine.rs

//! Decision Engine for Constitutional Guardian
//!
//! Makes Block/Pause/Allow decisions based on PII, bias, and harm analysis.
//! Preserves human override authority as per Article II, Section 2 of the Constitution.

use crate::guardian::bias_detector::BiasReport;
use crate::guardian::harm_analyzer::HarmReport;
use crate::guardian::ollama_client::RiskLevel;
use crate::guardian::pii_detector::PiiReport;
use serde::{Deserialize, Serialize};
use tracing::info;

/// Decision engine for constitutional compliance
pub struct DecisionEngine {
    config: DecisionConfig,
}

impl DecisionEngine {
    /// Create new decision engine with default thresholds
    pub fn new() -> Self {
        Self {
            config: DecisionConfig::default(),
        }
    }

    /// Create with custom configuration
    pub fn with_config(config: DecisionConfig) -> Self {
        Self { config }
    }

    /// Make decision based on all analysis results
    pub fn make_decision(
        &self,
        pii: &PiiReport,
        bias: &BiasReport,
        harm: &HarmReport,
    ) -> GuardianDecision {
        // Calculate overall compliance score (0.0 = bad, 1.0 = good)
        let pii_score = self.calculate_pii_score(pii);
        let bias_score = self.calculate_bias_score(bias);
        let harm_score = self.calculate_harm_score(harm);

        let overall_score = (pii_score + bias_score + harm_score) / 3.0;

        // Determine action based on thresholds
        let action = if overall_score < self.config.block_threshold {
            GuardianAction::Block
        } else if overall_score < self.config.pause_threshold {
            GuardianAction::Pause
        } else {
            GuardianAction::Allow
        };

        // Generate explanation
        let explanation = self.generate_explanation(pii, bias, harm, overall_score);

        // Determine if user escalation is needed
        let requires_user_escalation = action == GuardianAction::Block
            || action == GuardianAction::Pause
            || overall_score < self.config.escalation_threshold;

        info!(
            "Guardian decision: {:?} (score: {:.2}, escalation: {})",
            action, overall_score, requires_user_escalation
        );

        GuardianDecision {
            action,
            overall_score,
            pii_score,
            bias_score,
            harm_score,
            explanation,
            requires_user_escalation,
            user_can_override: true, // Article II, Section 2: Human agency preserved
            violations: self.collect_violations(pii, bias, harm),
        }
    }

    /// Calculate PII compliance score (1.0 = no issues, 0.0 = critical)
    fn calculate_pii_score(&self, pii: &PiiReport) -> f32 {
        if !pii.contains_pii {
            return 1.0;
        }

        match pii.risk_level {
            crate::guardian::pii_detector::RiskLevel::None => 1.0,
            crate::guardian::pii_detector::RiskLevel::Low => 0.8,
            crate::guardian::pii_detector::RiskLevel::Medium => 0.5,
            crate::guardian::pii_detector::RiskLevel::High => 0.2,
            crate::guardian::pii_detector::RiskLevel::Critical => 0.0,
        }
    }

    /// Calculate bias compliance score (1.0 = no issues, 0.0 = critical)
    fn calculate_bias_score(&self, bias: &BiasReport) -> f32 {
        if !bias.contains_bias {
            return 1.0;
        }

        match bias.severity {
            crate::guardian::bias_detector::Severity::None => 1.0,
            crate::guardian::bias_detector::Severity::Low => 0.8,
            crate::guardian::bias_detector::Severity::Medium => 0.5,
            crate::guardian::bias_detector::Severity::High => 0.2,
            crate::guardian::bias_detector::Severity::Critical => 0.0,
        }
    }

    /// Calculate harm compliance score (1.0 = no issues, 0.0 = critical)
    fn calculate_harm_score(&self, harm: &HarmReport) -> f32 {
        if !harm.contains_harm {
            return 1.0;
        }

        match harm.risk_level {
            RiskLevel::None => 1.0,
            RiskLevel::Low => 0.8,
            RiskLevel::Medium => 0.5,
            RiskLevel::High => 0.2,
            RiskLevel::Critical => 0.0,
        }
    }

    /// Generate human-readable explanation
    fn generate_explanation(
        &self,
        pii: &PiiReport,
        bias: &BiasReport,
        harm: &HarmReport,
        overall_score: f32,
    ) -> String {
        let mut parts = Vec::new();

        if pii.contains_pii {
            parts.push(format!(
                "PII detected: {} (risk: {:?})",
                pii.pii_types.join(", "),
                pii.risk_level
            ));
        }

        if bias.contains_bias {
            parts.push(format!(
                "Bias detected: {} (severity: {:?})",
                bias.bias_categories.join(", "),
                bias.severity
            ));
        }

        if harm.contains_harm {
            parts.push(format!(
                "Harm detected: {} (risk: {:?})",
                harm.harm_types.join(", "),
                harm.risk_level
            ));
        }

        if parts.is_empty() {
            format!("No violations detected (score: {:.2})", overall_score)
        } else {
            format!(
                "Constitutional concerns found (score: {:.2}): {}",
                overall_score,
                parts.join("; ")
            )
        }
    }

    /// Collect all violations
    fn collect_violations(
        &self,
        pii: &PiiReport,
        bias: &BiasReport,
        harm: &HarmReport,
    ) -> Vec<Violation> {
        let mut violations = Vec::new();

        if pii.contains_pii {
            violations.push(Violation {
                category: ViolationCategory::Privacy,
                severity: Self::pii_risk_to_severity(pii.risk_level),
                details: format!("PII types: {}", pii.pii_types.join(", ")),
            });
        }

        if bias.contains_bias {
            violations.push(Violation {
                category: ViolationCategory::Bias,
                severity: bias.severity.clone(),
                details: format!("Bias categories: {}", bias.bias_categories.join(", ")),
            });
        }

        if harm.contains_harm {
            violations.push(Violation {
                category: ViolationCategory::Harm,
                severity: Self::risk_to_severity(harm.risk_level),
                details: format!("Harm types: {}", harm.harm_types.join(", ")),
            });
        }

        violations
    }

    /// Convert RiskLevel to Severity
    fn risk_to_severity(risk: RiskLevel) -> crate::guardian::bias_detector::Severity {
        match risk {
            RiskLevel::None => crate::guardian::bias_detector::Severity::None,
            RiskLevel::Low => crate::guardian::bias_detector::Severity::Low,
            RiskLevel::Medium => crate::guardian::bias_detector::Severity::Medium,
            RiskLevel::High => crate::guardian::bias_detector::Severity::High,
            RiskLevel::Critical => crate::guardian::bias_detector::Severity::Critical,
        }
    }

    /// Convert PII RiskLevel to Severity
    fn pii_risk_to_severity(
        risk: crate::guardian::pii_detector::RiskLevel,
    ) -> crate::guardian::bias_detector::Severity {
        match risk {
            crate::guardian::pii_detector::RiskLevel::None => {
                crate::guardian::bias_detector::Severity::None
            }
            crate::guardian::pii_detector::RiskLevel::Low => {
                crate::guardian::bias_detector::Severity::Low
            }
            crate::guardian::pii_detector::RiskLevel::Medium => {
                crate::guardian::bias_detector::Severity::Medium
            }
            crate::guardian::pii_detector::RiskLevel::High => {
                crate::guardian::bias_detector::Severity::High
            }
            crate::guardian::pii_detector::RiskLevel::Critical => {
                crate::guardian::bias_detector::Severity::Critical
            }
        }
    }
}

impl Default for DecisionEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Decision engine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionConfig {
    /// Block if score < this threshold (default: 0.3)
    pub block_threshold: f32,
    /// Pause if score < this threshold (default: 0.7)
    pub pause_threshold: f32,
    /// Escalate to user if score < this threshold (default: 0.8)
    pub escalation_threshold: f32,
}

impl Default for DecisionConfig {
    fn default() -> Self {
        Self {
            block_threshold: 0.3,
            pause_threshold: 0.7,
            escalation_threshold: 0.8,
        }
    }
}

/// Guardian action decision
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GuardianAction {
    /// Allow message to proceed
    Allow,
    /// Pause for user review
    Pause,
    /// Block message completely
    Block,
}

/// Complete guardian decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardianDecision {
    pub action: GuardianAction,
    pub overall_score: f32,
    pub pii_score: f32,
    pub bias_score: f32,
    pub harm_score: f32,
    pub explanation: String,
    pub requires_user_escalation: bool,
    pub user_can_override: bool, // Article II, Section 2
    pub violations: Vec<Violation>,
}

/// Violation details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Violation {
    pub category: ViolationCategory,
    pub severity: crate::guardian::bias_detector::Severity,
    pub details: String,
}

/// Violation category
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViolationCategory {
    Privacy,
    Bias,
    Harm,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guardian::bias_detector::{BiasReport, Severity};
    use crate::guardian::harm_analyzer::{HarmReport, Intent};
    use crate::guardian::ollama_client::RiskLevel;
    use crate::guardian::pii_detector::PiiReport;

    #[test]
    fn test_allow_decision() {
        let engine = DecisionEngine::new();

        let pii = PiiReport {
            contains_pii: false,
            pii_types: Vec::new(),
            risk_level: crate::guardian::pii_detector::RiskLevel::None,
            detected_patterns: Vec::new(),
        };

        let bias = BiasReport {
            contains_bias: false,
            bias_categories: Vec::new(),
            severity: Severity::None,
            fairness_metrics: Default::default(),
        };

        let harm = HarmReport {
            contains_harm: false,
            harm_types: Vec::new(),
            toxicity_score: 0.0,
            intent: Intent::Benign,
            risk_level: RiskLevel::None,
            confidence: 1.0,
            details: "Clean text".to_string(),
        };

        let decision = engine.make_decision(&pii, &bias, &harm);

        assert_eq!(decision.action, GuardianAction::Allow);
        assert_eq!(decision.overall_score, 1.0);
        assert!(!decision.requires_user_escalation);
    }

    #[test]
    fn test_block_decision() {
        let engine = DecisionEngine::new();

        // All three critical violations = average score 0.0 < 0.3 = Block
        let pii = PiiReport {
            contains_pii: true,
            pii_types: vec!["ssn".to_string()],
            risk_level: crate::guardian::pii_detector::RiskLevel::Critical,
            detected_patterns: Vec::new(),
        };

        let bias = BiasReport {
            contains_bias: true,
            bias_categories: vec!["gender".to_string()],
            severity: Severity::Critical,
            fairness_metrics: Default::default(),
        };

        let harm = HarmReport {
            contains_harm: true,
            harm_types: vec!["violence".to_string()],
            toxicity_score: 0.9,
            intent: Intent::Malicious,
            risk_level: RiskLevel::Critical,
            confidence: 0.95,
            details: "Multiple critical violations".to_string(),
        };

        let decision = engine.make_decision(&pii, &bias, &harm);

        assert_eq!(decision.action, GuardianAction::Block);
        assert!(decision.overall_score < 0.3);
        assert!(decision.requires_user_escalation);
        assert!(decision.user_can_override); // Human agency preserved
    }

    #[test]
    fn test_pause_decision() {
        let engine = DecisionEngine::new();

        // Medium violations = average score ~0.5 (0.3 <= score < 0.7) = Pause
        let pii = PiiReport {
            contains_pii: true,
            pii_types: vec!["email".to_string()],
            risk_level: crate::guardian::pii_detector::RiskLevel::Medium,
            detected_patterns: Vec::new(),
        };

        let bias = BiasReport {
            contains_bias: true,
            bias_categories: vec!["age".to_string()],
            severity: Severity::Medium,
            fairness_metrics: Default::default(),
        };

        let harm = HarmReport {
            contains_harm: true,
            harm_types: vec!["mild_violence".to_string()],
            toxicity_score: 0.4,
            intent: Intent::Concerning,
            risk_level: RiskLevel::Medium,
            confidence: 0.7,
            details: "Multiple medium-level violations".to_string(),
        };

        let decision = engine.make_decision(&pii, &bias, &harm);

        assert_eq!(decision.action, GuardianAction::Pause);
        assert!(decision.overall_score >= 0.3 && decision.overall_score < 0.7);
    }

    #[test]
    fn test_human_override_always_available() {
        let engine = DecisionEngine::new();

        // Test that human override is always available regardless of severity
        let pii = PiiReport {
            contains_pii: true,
            pii_types: vec!["ssn".to_string(), "credit_card".to_string()],
            risk_level: crate::guardian::pii_detector::RiskLevel::Critical,
            detected_patterns: Vec::new(),
        };

        let bias = BiasReport {
            contains_bias: true,
            bias_categories: vec!["gender".to_string(), "ethnicity".to_string()],
            severity: Severity::Critical,
            fairness_metrics: Default::default(),
        };

        let harm = HarmReport {
            contains_harm: true,
            harm_types: vec!["violence".to_string(), "hate_speech".to_string()],
            toxicity_score: 0.95,
            intent: Intent::Emergency,
            risk_level: RiskLevel::Critical,
            confidence: 0.99,
            details: "Severe multi-category violations".to_string(),
        };

        let decision = engine.make_decision(&pii, &bias, &harm);

        // Even with all critical violations, human can override (Article II, Section 2)
        assert!(decision.user_can_override);
        assert_eq!(decision.action, GuardianAction::Block);
        assert_eq!(decision.overall_score, 0.0);
    }
}
