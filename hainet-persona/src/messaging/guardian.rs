// START OF FILE hainet-persona/src/messaging/guardian.rs

//! Constitutional Guardian interceptor for message monitoring
//!
//! This module implements the Guardian system that monitors all agent messages
//! for constitutional compliance. The Guardian enforces:
//! - Article I: Privacy-first principles (PII detection)
//! - Article II: Human rights protection (bias detection, harm prevention)
//! - Article V: Constitutional enforcement (independent monitoring)
//!
//! NOTE: This is a foundational implementation with stub detectors.
//! Full PII/bias/harm detection will be implemented in Cycle 0.4.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::types::{Message, MessageContent};

/// Result of Guardian interception
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InterceptResult {
    Allow,
    Pause(PauseReason),
    Block(BlockReason),
}

/// Reasons for pausing a message (user review required)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PauseReason {
    PrivacyReview,
    BiasDetected,
    UncertainContent,
    UserConfirmationRequired,
}

/// Reasons for blocking a message (immediate action)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockReason {
    HarmRisk,
    PrivacyViolation,
    SecurityThreat,
    ConstitutionalBreach,
}

/// Compliance scores for a message
#[derive(Debug, Clone, Default)]
pub struct ComplianceScores {
    pub privacy_score: f32,    // 0.0 = violations, 1.0 = compliant
    pub bias_score: f32,       // 0.0 = biased, 1.0 = fair
    pub harm_score: f32,       // 0.0 = harmful, 1.0 = safe
    pub overall_score: f32,    // Combined score
}

/// Guardian interceptor configuration
#[derive(Debug, Clone)]
pub struct GuardianConfig {
    pub block_threshold: f32,   // Below this = block
    pub pause_threshold: f32,   // Below this = pause for review
    pub enable_privacy_check: bool,
    pub enable_bias_check: bool,
    pub enable_harm_check: bool,
}

impl Default for GuardianConfig {
    fn default() -> Self {
        Self {
            block_threshold: 0.3,   // Very low scores are blocked
            pause_threshold: 0.7,   // Medium scores need review
            enable_privacy_check: true,
            enable_bias_check: true,
            enable_harm_check: true,
        }
    }
}

/// Statistics for Guardian operations
#[derive(Debug, Clone, Default)]
pub struct GuardianStats {
    pub messages_intercepted: u64,
    pub messages_allowed: u64,
    pub messages_paused: u64,
    pub messages_blocked: u64,
    pub privacy_violations: u64,
    pub bias_detections: u64,
    pub harm_risks: u64,
}

/// Guardian interceptor for constitutional compliance
///
/// The Guardian operates independently and monitors all agent messages.
/// It can pause messages for human review or block them entirely.
/// 
/// Constitutional authority: Article V, Section 1
pub struct GuardianInterceptor {
    config: Arc<RwLock<GuardianConfig>>,
    stats: Arc<RwLock<GuardianStats>>,
    
    // Detector hooks (will be implemented in Cycle 0.4)
    privacy_detector: Arc<RwLock<Option<PrivacyDetectorHook>>>,
    bias_detector: Arc<RwLock<Option<BiasDetectorHook>>>,
    harm_detector: Arc<RwLock<Option<HarmDetectorHook>>>,
}

/// Privacy detector hook (placeholder for Cycle 0.4)
pub type PrivacyDetectorHook = Arc<dyn Fn(&str) -> f32 + Send + Sync>;

/// Bias detector hook (placeholder for Cycle 0.4)
pub type BiasDetectorHook = Arc<dyn Fn(&str) -> f32 + Send + Sync>;

/// Harm detector hook (placeholder for Cycle 0.4)
pub type HarmDetectorHook = Arc<dyn Fn(&str) -> f32 + Send + Sync>;

impl GuardianInterceptor {
    /// Create a new Guardian interceptor with default configuration
    pub fn new() -> Self {
        Self::with_config(GuardianConfig::default())
    }

    /// Create a new Guardian interceptor with custom configuration
    pub fn with_config(config: GuardianConfig) -> Self {
        info!("Initializing Guardian interceptor");
        
        Self {
            config: Arc::new(RwLock::new(config)),
            stats: Arc::new(RwLock::new(GuardianStats::default())),
            privacy_detector: Arc::new(RwLock::new(None)),
            bias_detector: Arc::new(RwLock::new(None)),
            harm_detector: Arc::new(RwLock::new(None)),
        }
    }

    /// Intercept and analyze a message for constitutional compliance
    ///
    /// This is the core Guardian function that enforces HAI-Net's principles.
    pub async fn intercept(&self, msg: &Message) -> Result<InterceptResult> {
        // Increment interception counter
        {
            let mut stats = self.stats.write().await;
            stats.messages_intercepted += 1;
        }

        // Extract text content for analysis
        let text_content = self.extract_text_content(msg);

        // Calculate compliance scores
        let scores = self.calculate_compliance_scores(&text_content).await;

        // Make decision based on scores
        let result = self.make_decision(&scores, msg).await;

        // Update statistics
        self.update_stats(&result).await;

        // Log result
        match &result {
            InterceptResult::Allow => {
                debug!("Guardian: Message allowed (score: {:.2})", scores.overall_score);
            }
            InterceptResult::Pause(reason) => {
                warn!("Guardian: Message paused for {:?} (score: {:.2})", reason, scores.overall_score);
            }
            InterceptResult::Block(reason) => {
                warn!("Guardian: Message blocked for {:?} (score: {:.2})", reason, scores.overall_score);
            }
        }

        Ok(result)
    }

    /// Extract text content from message for analysis
    fn extract_text_content(&self, msg: &Message) -> String {
        match &msg.content {
            MessageContent::UserInput(text) => text.clone(),
            MessageContent::TaskAssignment(task) => task.description.clone(),
            MessageContent::TaskResult(result) => format!("{:?}", result.output),
            MessageContent::Query(text) => text.clone(),
            MessageContent::Response(text) => text.clone(),
            MessageContent::PMCoordination(coord) => format!("{}: {:?}", coord.topic, coord.data),
            MessageContent::StatusUpdate(status) => status.message.clone(),
            MessageContent::GuardianAlert(alert) => alert.message.clone(),
            MessageContent::ErrorReport(error) => error.message.clone(),
        }
    }

    /// Calculate compliance scores for message content
    async fn calculate_compliance_scores(&self, text: &str) -> ComplianceScores {
        let config = self.config.read().await;

        let mut scores = ComplianceScores::default();

        // Privacy check (PII detection)
        if config.enable_privacy_check {
            scores.privacy_score = self.check_privacy(text).await;
        } else {
            scores.privacy_score = 1.0; // Disabled = pass
        }

        // Bias check
        if config.enable_bias_check {
            scores.bias_score = self.check_bias(text).await;
        } else {
            scores.bias_score = 1.0; // Disabled = pass
        }

        // Harm check
        if config.enable_harm_check {
            scores.harm_score = self.check_harm(text).await;
        } else {
            scores.harm_score = 1.0; // Disabled = pass
        }

        // Calculate overall score (weighted average)
        scores.overall_score = scores.privacy_score * 0.4 
            + scores.bias_score * 0.3 
            + scores.harm_score * 0.3;

        scores
    }

    /// Check privacy compliance (PII detection stub)
    async fn check_privacy(&self, text: &str) -> f32 {
        let detector = self.privacy_detector.read().await;
        
        if let Some(detector_fn) = detector.as_ref() {
            detector_fn(text)
        } else {
            // Stub implementation: simple keyword check
            let pii_keywords = ["ssn", "social security", "credit card", "password"];
            let lower_text = text.to_lowercase();
            
            let violations = pii_keywords.iter()
                .filter(|kw| lower_text.contains(*kw))
                .count();

            if violations > 0 {
                0.2 // Low score if PII keywords detected
            } else {
                1.0 // High score otherwise
            }
        }
    }

    /// Check bias compliance (stub)
    async fn check_bias(&self, text: &str) -> f32 {
        let detector = self.bias_detector.read().await;
        
        if let Some(detector_fn) = detector.as_ref() {
            detector_fn(text)
        } else {
            // Stub implementation: assume fair
            1.0
        }
    }

    /// Check harm risk (stub)
    async fn check_harm(&self, text: &str) -> f32 {
        let detector = self.harm_detector.read().await;
        
        if let Some(detector_fn) = detector.as_ref() {
            detector_fn(text)
        } else {
            // Stub implementation: simple keyword check
            let harm_keywords = ["kill", "harm", "destroy", "attack"];
            let lower_text = text.to_lowercase();
            
            let risks = harm_keywords.iter()
                .filter(|kw| lower_text.contains(*kw))
                .count();

            if risks > 0 {
                0.2 // Low score if harm keywords detected (below block threshold)
            } else {
                1.0 // High score otherwise
            }
        }
    }

    /// Make decision based on compliance scores
    async fn make_decision(&self, scores: &ComplianceScores, _msg: &Message) -> InterceptResult {
        let config = self.config.read().await;

        // Check individual scores first (stricter enforcement)
        // Block if any individual score is critically low
        if scores.privacy_score < config.block_threshold {
            return InterceptResult::Block(BlockReason::PrivacyViolation);
        }
        if scores.harm_score < config.block_threshold {
            return InterceptResult::Block(BlockReason::HarmRisk);
        }

        // Block if overall score is critically low
        if scores.overall_score < config.block_threshold {
            return InterceptResult::Block(BlockReason::ConstitutionalBreach);
        }

        // Pause if any individual score needs review
        if scores.privacy_score < config.pause_threshold {
            return InterceptResult::Pause(PauseReason::PrivacyReview);
        }
        if scores.bias_score < config.pause_threshold {
            return InterceptResult::Pause(PauseReason::BiasDetected);
        }
        if scores.harm_score < config.pause_threshold {
            return InterceptResult::Pause(PauseReason::UncertainContent);
        }

        // Pause if overall score is medium
        if scores.overall_score < config.pause_threshold {
            return InterceptResult::Pause(PauseReason::UncertainContent);
        }

        // Allow if all scores are high
        InterceptResult::Allow
    }

    /// Update statistics based on intercept result
    async fn update_stats(&self, result: &InterceptResult) {
        let mut stats = self.stats.write().await;

        match result {
            InterceptResult::Allow => {
                stats.messages_allowed += 1;
            }
            InterceptResult::Pause(reason) => {
                stats.messages_paused += 1;
                match reason {
                    PauseReason::PrivacyReview => stats.privacy_violations += 1,
                    PauseReason::BiasDetected => stats.bias_detections += 1,
                    _ => {}
                }
            }
            InterceptResult::Block(reason) => {
                stats.messages_blocked += 1;
                match reason {
                    BlockReason::PrivacyViolation => stats.privacy_violations += 1,
                    BlockReason::HarmRisk => stats.harm_risks += 1,
                    _ => {}
                }
            }
        }
    }

    /// Set custom privacy detector
    pub async fn set_privacy_detector(&self, detector: PrivacyDetectorHook) {
        let mut hook = self.privacy_detector.write().await;
        *hook = Some(detector);
        info!("Privacy detector registered");
    }

    /// Set custom bias detector
    pub async fn set_bias_detector(&self, detector: BiasDetectorHook) {
        let mut hook = self.bias_detector.write().await;
        *hook = Some(detector);
        info!("Bias detector registered");
    }

    /// Set custom harm detector
    pub async fn set_harm_detector(&self, detector: HarmDetectorHook) {
        let mut hook = self.harm_detector.write().await;
        *hook = Some(detector);
        info!("Harm detector registered");
    }

    /// Get Guardian statistics
    pub async fn get_stats(&self) -> GuardianStats {
        self.stats.read().await.clone()
    }

    /// Reset statistics
    pub async fn reset_stats(&self) {
        let mut stats = self.stats.write().await;
        *stats = GuardianStats::default();
    }

    /// Update configuration
    pub async fn update_config(&self, config: GuardianConfig) {
        let mut cfg = self.config.write().await;
        *cfg = config;
        info!("Guardian configuration updated");
    }

    /// Get current configuration
    pub async fn get_config(&self) -> GuardianConfig {
        self.config.read().await.clone()
    }
}

impl Default for GuardianInterceptor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messaging::types::{AgentId, AgentType};

    fn create_test_message(content: MessageContent) -> Message {
        let from = AgentId::new(AgentType::Admin, "admin-1".to_string());
        let to = AgentId::new(AgentType::PM, "pm-1".to_string());
        
        Message::new(from, to, content)
    }

    #[tokio::test]
    async fn test_guardian_creation() {
        let guardian = GuardianInterceptor::new();
        let stats = guardian.get_stats().await;
        assert_eq!(stats.messages_intercepted, 0);
    }

    #[tokio::test]
    async fn test_allow_safe_message() {
        let guardian = GuardianInterceptor::new();
        
        let msg = create_test_message(
            MessageContent::UserInput("Hello, please help me organize my files".to_string())
        );

        let result = guardian.intercept(&msg).await.unwrap();
        assert_eq!(result, InterceptResult::Allow);

        let stats = guardian.get_stats().await;
        assert_eq!(stats.messages_allowed, 1);
    }

    #[tokio::test]
    async fn test_block_pii_violation() {
        let guardian = GuardianInterceptor::new();
        
        let msg = create_test_message(
            MessageContent::UserInput("My SSN is 123-45-6789".to_string())
        );

        let result = guardian.intercept(&msg).await.unwrap();
        assert!(matches!(result, InterceptResult::Block(BlockReason::PrivacyViolation)));

        let stats = guardian.get_stats().await;
        assert_eq!(stats.messages_blocked, 1);
        assert_eq!(stats.privacy_violations, 1);
    }

    #[tokio::test]
    async fn test_block_harm_risk() {
        let guardian = GuardianInterceptor::new();
        
        let msg = create_test_message(
            MessageContent::UserInput("How to kill the process".to_string())
        );

        let result = guardian.intercept(&msg).await.unwrap();
        // Should block due to "kill" keyword
        assert!(matches!(result, InterceptResult::Block(_)));

        let stats = guardian.get_stats().await;
        assert_eq!(stats.messages_blocked, 1);
    }

    #[tokio::test]
    async fn test_custom_privacy_detector() {
        let guardian = GuardianInterceptor::new();
        
        // Set custom detector that always flags privacy issues
        let detector: PrivacyDetectorHook = Arc::new(|_text| 0.5);
        guardian.set_privacy_detector(detector).await;

        let msg = create_test_message(
            MessageContent::UserInput("Test message".to_string())
        );

        let result = guardian.intercept(&msg).await.unwrap();
        // Should pause due to medium score (0.5)
        assert!(matches!(result, InterceptResult::Pause(_)));
    }

    #[tokio::test]
    async fn test_custom_harm_detector() {
        let guardian = GuardianInterceptor::new();
        
        // Set custom detector that always allows
        let detector: HarmDetectorHook = Arc::new(|_text| 1.0);
        guardian.set_harm_detector(detector).await;

        let msg = create_test_message(
            MessageContent::UserInput("How to kill the process".to_string())
        );

        let result = guardian.intercept(&msg).await.unwrap();
        // Should allow despite "kill" keyword, because custom detector overrides
        assert_eq!(result, InterceptResult::Allow);
    }

    #[tokio::test]
    async fn test_config_update() {
        let guardian = GuardianInterceptor::new();
        
        let mut config = GuardianConfig::default();
        config.block_threshold = 0.5;
        config.pause_threshold = 0.8;
        
        guardian.update_config(config.clone()).await;
        
        let current_config = guardian.get_config().await;
        assert_eq!(current_config.block_threshold, 0.5);
        assert_eq!(current_config.pause_threshold, 0.8);
    }

    #[tokio::test]
    async fn test_statistics_tracking() {
        let guardian = GuardianInterceptor::new();
        
        // Process multiple messages
        let safe_msg = create_test_message(
            MessageContent::UserInput("Safe message".to_string())
        );
        guardian.intercept(&safe_msg).await.unwrap();

        let pii_msg = create_test_message(
            MessageContent::UserInput("My password is secret123".to_string())
        );
        guardian.intercept(&pii_msg).await.unwrap();

        let stats = guardian.get_stats().await;
        assert_eq!(stats.messages_intercepted, 2);
        assert_eq!(stats.messages_allowed, 1);
        assert_eq!(stats.messages_blocked, 1);
        assert_eq!(stats.privacy_violations, 1);
    }

    #[tokio::test]
    async fn test_disabled_checks() {
        let mut config = GuardianConfig::default();
        config.enable_privacy_check = false;
        config.enable_bias_check = false;
        config.enable_harm_check = false;
        
        let guardian = GuardianInterceptor::with_config(config);
        
        // Even with PII, should allow if checks are disabled
        let msg = create_test_message(
            MessageContent::UserInput("My SSN is 123-45-6789".to_string())
        );

        let result = guardian.intercept(&msg).await.unwrap();
        assert_eq!(result, InterceptResult::Allow);
    }

    #[tokio::test]
    async fn test_reset_stats() {
        let guardian = GuardianInterceptor::new();
        
        let msg = create_test_message(
            MessageContent::UserInput("Test".to_string())
        );
        guardian.intercept(&msg).await.unwrap();

        let stats = guardian.get_stats().await;
        assert_eq!(stats.messages_intercepted, 1);

        guardian.reset_stats().await;

        let stats = guardian.get_stats().await;
        assert_eq!(stats.messages_intercepted, 0);
    }
}
