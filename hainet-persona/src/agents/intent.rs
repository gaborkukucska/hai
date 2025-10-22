//! # Intent Parser
//! 
//! Analyzes user requests to understand intent, extract entities, and classify request types.
//! Uses AI provider system for natural language understanding.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Type of user intent
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntentType {
    /// User asking a question
    Question,
    
    /// User requesting task execution
    Task,
    
    /// User issuing a command
    Command,
    
    /// User providing information/feedback
    Information,
    
    /// Unclear or ambiguous intent
    Unclear,
}

/// Parsed user intent with extracted information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Intent {
    /// Classified intent type
    pub intent_type: IntentType,
    
    /// Original user input
    pub original_text: String,
    
    /// Simplified/normalized version of request
    pub normalized_text: String,
    
    /// Extracted entities (e.g., {"email": "user@example.com", "date": "tomorrow"})
    pub entities: HashMap<String, String>,
    
    /// Confidence score (0.0-1.0)
    pub confidence: f64,
    
    /// Suggested PM domain (Communications, Knowledge, System)
    pub suggested_domain: Option<String>,
    
    /// Suggested action/tool
    pub suggested_action: Option<String>,
}

/// Intent parsing system
pub struct IntentParser {
    /// Minimum confidence threshold for accepting parsed intent
    confidence_threshold: f64,
}

impl IntentParser {
    /// Create new intent parser
    pub fn new() -> Self {
        Self {
            confidence_threshold: 0.6,
        }
    }
    
    /// Create with custom confidence threshold
    pub fn with_threshold(threshold: f64) -> Self {
        Self {
            confidence_threshold: threshold,
        }
    }
    
    /// Parse user input to extract intent
    /// 
    /// This is a rule-based implementation. In Phase 1.1, we'll integrate with
    /// AI providers for ML-based intent classification.
    pub async fn parse(&self, user_input: &str) -> Result<Intent> {
        let normalized = self.normalize_text(user_input);
        
        // Rule-based classification (will be replaced with LLM in full implementation)
        let intent_type = self.classify_intent(&normalized);
        let entities = self.extract_entities(&normalized, &intent_type);
        let (domain, action) = self.suggest_domain_and_action(&normalized, &intent_type);
        
        // Calculate confidence based on keyword matches
        let confidence = self.calculate_confidence(&normalized, &intent_type);
        
        if confidence < self.confidence_threshold {
            return Ok(Intent {
                intent_type: IntentType::Unclear,
                original_text: user_input.to_string(),
                normalized_text: normalized,
                entities: HashMap::new(),
                confidence,
                suggested_domain: None,
                suggested_action: None,
            });
        }
        
        Ok(Intent {
            intent_type,
            original_text: user_input.to_string(),
            normalized_text: normalized,
            entities,
            confidence,
            suggested_domain: domain,
            suggested_action: action,
        })
    }
    
    /// Normalize text (lowercase, trim, remove extra whitespace)
    fn normalize_text(&self, text: &str) -> String {
        text.trim()
            .to_lowercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }
    
    /// Classify intent type using rule-based approach
    fn classify_intent(&self, text: &str) -> IntentType {
        // Question indicators
        let question_words = ["what", "when", "where", "who", "why", "how", "is", "are", "can", "could", "would"];
        if text.ends_with('?') || question_words.iter().any(|w| text.starts_with(w)) {
            return IntentType::Question;
        }
        
        // Command indicators
        let command_words = ["stop", "start", "pause", "resume", "cancel", "quit", "exit", "shutdown"];
        if command_words.iter().any(|w| text.contains(w)) {
            return IntentType::Command;
        }
        
        // Task indicators
        let task_words = ["send", "find", "search", "create", "delete", "update", "organize", "schedule", "remind"];
        if task_words.iter().any(|w| text.contains(w)) {
            return IntentType::Task;
        }
        
        // Information indicators
        let info_words = ["thank", "ok", "yes", "no", "correct", "wrong", "good", "bad"];
        if info_words.iter().any(|w| text.contains(w)) {
            return IntentType::Information;
        }
        
        // Default to unclear
        IntentType::Unclear
    }
    
    /// Extract entities from text based on intent type
    fn extract_entities(&self, text: &str, _intent_type: &IntentType) -> HashMap<String, String> {
        let mut entities = HashMap::new();
        
        // Simple entity extraction (will be enhanced with NER in full implementation)
        
        // Email extraction
        if let Some(email) = self.extract_email(text) {
            entities.insert("email".to_string(), email);
        }
        
        // Time/date keywords
        if text.contains("today") {
            entities.insert("date".to_string(), "today".to_string());
        } else if text.contains("tomorrow") {
            entities.insert("date".to_string(), "tomorrow".to_string());
        } else if text.contains("yesterday") {
            entities.insert("date".to_string(), "yesterday".to_string());
        }
        
        // File paths
        if let Some(path) = self.extract_file_path(text) {
            entities.insert("file_path".to_string(), path);
        }
        
        entities
    }
    
    /// Extract email addresses from text
    fn extract_email(&self, text: &str) -> Option<String> {
        // Simple email pattern matching
        for word in text.split_whitespace() {
            if word.contains('@') && word.contains('.') {
                return Some(word.trim_matches(|c: char| !c.is_alphanumeric() && c != '@' && c != '.').to_string());
            }
        }
        None
    }
    
    /// Extract file paths from text
    fn extract_file_path(&self, text: &str) -> Option<String> {
        // Look for common path indicators
        for word in text.split_whitespace() {
            if word.starts_with('/') || word.starts_with("~/") || word.contains("\\") {
                return Some(word.to_string());
            }
        }
        None
    }
    
    /// Suggest PM domain and action based on intent
    fn suggest_domain_and_action(&self, text: &str, _intent_type: &IntentType) -> (Option<String>, Option<String>) {
        // Communications domain
        if text.contains("email") || text.contains("message") || text.contains("chat") {
            return (Some("Communications".to_string()), Some("email_management".to_string()));
        }
        
        // Knowledge domain
        if text.contains("search") || text.contains("find") || text.contains("research") || text.contains("learn") {
            return (Some("Knowledge".to_string()), Some("search".to_string()));
        }
        
        // System domain
        if text.contains("file") || text.contains("folder") || text.contains("network") || text.contains("system") {
            return (Some("System".to_string()), Some("file_management".to_string()));
        }
        
        (None, None)
    }
    
    /// Calculate confidence score based on keyword matches
    fn calculate_confidence(&self, text: &str, intent_type: &IntentType) -> f64 {
        let base_confidence = match intent_type {
            IntentType::Question if text.ends_with('?') => 0.9,
            IntentType::Question => 0.7,
            IntentType::Task => 0.8,
            IntentType::Command => 0.85,
            IntentType::Information => 0.6,
            IntentType::Unclear => 0.3,
        };
        
        // Boost confidence if we found entities
        let has_email = text.contains('@');
        let has_date = text.contains("today") || text.contains("tomorrow");
        let has_path = text.contains('/') || text.contains('\\');
        
        let entity_boost = if has_email || has_date || has_path { 0.1_f64 } else { 0.0_f64 };
        
        (base_confidence + entity_boost).min(1.0_f64)
    }
}

impl Default for IntentParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_intent_parser_creation() {
        let parser = IntentParser::new();
        assert_eq!(parser.confidence_threshold, 0.6);
        
        let parser_custom = IntentParser::with_threshold(0.8);
        assert_eq!(parser_custom.confidence_threshold, 0.8);
    }
    
    #[tokio::test]
    async fn test_parse_question() {
        let parser = IntentParser::new();
        let intent = parser.parse("What is the weather today?").await.unwrap();
        
        assert_eq!(intent.intent_type, IntentType::Question);
        assert!(intent.confidence >= 0.6);
        assert_eq!(intent.original_text, "What is the weather today?");
    }
    
    #[tokio::test]
    async fn test_parse_task() {
        let parser = IntentParser::new();
        let intent = parser.parse("Send an email to user@example.com").await.unwrap();
        
        assert_eq!(intent.intent_type, IntentType::Task);
        assert!(intent.confidence >= 0.6);
        assert_eq!(intent.entities.get("email"), Some(&"user@example.com".to_string()));
        assert_eq!(intent.suggested_domain, Some("Communications".to_string()));
    }
    
    #[tokio::test]
    async fn test_parse_command() {
        let parser = IntentParser::new();
        let intent = parser.parse("Stop the current task").await.unwrap();
        
        assert_eq!(intent.intent_type, IntentType::Command);
        assert!(intent.confidence >= 0.6);
    }
    
    #[tokio::test]
    async fn test_entity_extraction_email() {
        let parser = IntentParser::new();
        let email = parser.extract_email("contact me at test@example.com please");
        assert_eq!(email, Some("test@example.com".to_string()));
    }
    
    #[tokio::test]
    async fn test_entity_extraction_date() {
        let parser = IntentParser::new();
        let intent = parser.parse("Schedule meeting for tomorrow").await.unwrap();
        
        assert_eq!(intent.entities.get("date"), Some(&"tomorrow".to_string()));
    }
    
    #[tokio::test]
    async fn test_normalize_text() {
        let parser = IntentParser::new();
        let normalized = parser.normalize_text("  What   IS    the   WEATHER?  ");
        assert_eq!(normalized, "what is the weather?");
    }
    
    #[tokio::test]
    async fn test_unclear_intent_low_confidence() {
        let parser = IntentParser::with_threshold(0.7);
        let intent = parser.parse("hmm maybe something").await.unwrap();
        
        assert_eq!(intent.intent_type, IntentType::Unclear);
        assert!(intent.confidence < 0.7);
    }
}
