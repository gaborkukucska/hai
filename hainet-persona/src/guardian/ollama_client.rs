// START OF FILE hainet-persona/src/guardian/ollama_client.rs

//! Ollama Client for Guardian-specific Inference
//!
//! This module provides a Guardian-specific wrapper around Ollama API
//! for PII detection, bias analysis, and harm detection using LLMs.

use crate::ai_providers::{AIProviderManager, SelectionContext};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::trace;

/// Guardian-specific Ollama client with JSON-structured output parsing
#[derive(Clone)]
pub struct GuardianOllamaClient {
    ai_provider_manager: Arc<AIProviderManager>,
    _default_model: String,
}

impl GuardianOllamaClient {
    /// Create new Guardian Ollama client
    pub fn new(ai_provider_manager: Arc<AIProviderManager>, default_model: String) -> Self {
        Self {
            ai_provider_manager,
            _default_model: default_model,
        }
    }

    /// Analyze text for PII (Personally Identifiable Information)
    pub async fn analyze_pii(&self, text: &str) -> Result<PiiAnalysisResult> {
        let prompt = format!(
            r#"Analyze the following text for personally identifiable information (PII).
Respond ONLY with valid JSON in this exact format:
{{
  "contains_pii": true/false,
  "pii_types": ["email", "phone", "ssn", "credit_card", "ip_address"],
  "risk_level": "none/low/medium/high/critical",
  "redacted_count": 0
}}

Text to analyze:
{}

JSON response:"#,
            text
        );

        // Log Guardian PII analysis request at TRACE level
        trace!(
            target: "llm_messages",
            "[GUARDIAN PII REQUEST] Analyzing text ({} chars):\n{}\nPrompt ({} chars):\n{}",
            text.len(),
            text,
            prompt.len(),
            prompt
        );

        let options = crate::ai_providers::providers::GenerationOptions {
            temperature: Some(0.1),
            max_tokens: Some(512),
            top_p: Some(0.9),
            stop: None,
            system: None,
        };

        let selection_context = SelectionContext::for_guardian();
        let selected_model = self
            .ai_provider_manager
            .select_model_for_agent(selection_context)
            .await
            .context("Failed to select a model for PII analysis")?;

        let client = selected_model.get_client()?;
        let model_name = if selected_model.model_id.contains("::") {
            selected_model.model_id.split("::").nth(1).unwrap_or(&selected_model.model_id)
        } else {
            &selected_model.model_id
        };

        let response = client
            .generate(model_name, &prompt, options)
            .await
            .context("Failed to generate PII analysis")?;

        // Log Guardian PII analysis response at TRACE level
        trace!(
            target: "llm_messages",
            "[GUARDIAN PII RESPONSE] Raw response ({} chars):\n{}",
            response.text.len(),
            response.text
        );

        // Parse JSON response
        let result = self.parse_json_response::<PiiAnalysisResult>(&response.text)
            .context("Failed to parse PII analysis JSON")?;

        // Log parsed result
        trace!(
            target: "llm_messages",
            "[GUARDIAN PII RESULT] Parsed: contains_pii={}, types={:?}, risk={:?}",
            result.contains_pii,
            result.pii_types,
            result.risk_level
        );

        Ok(result)
    }

    /// Analyze text for bias
    pub async fn analyze_bias(&self, text: &str) -> Result<BiasAnalysisResult> {
        let prompt = format!(
            r#"Analyze the following text for bias and stereotypes.
Respond ONLY with valid JSON in this exact format:
{{
  "contains_bias": true/false,
  "bias_types": ["gender", "age", "ethnicity", "disability", "religion"],
  "severity": "none/low/medium/high/critical",
  "stereotype_count": 0
}}

Text to analyze:
{}

JSON response:"#,
            text
        );

        // Log Guardian bias analysis request at TRACE level
        trace!(
            target: "llm_messages",
            "[GUARDIAN BIAS REQUEST] Analyzing text ({} chars):\n{}\nPrompt ({} chars):\n{}",
            text.len(),
            text,
            prompt.len(),
            prompt
        );

        let options = crate::ai_providers::providers::GenerationOptions {
            temperature: Some(0.1),
            max_tokens: Some(512),
            top_p: Some(0.9),
            stop: None,
            system: None,
        };

        let selection_context = SelectionContext::for_guardian();
        let selected_model = self
            .ai_provider_manager
            .select_model_for_agent(selection_context)
            .await
            .context("Failed to select a model for bias analysis")?;

        let client = selected_model.get_client()?;
        let model_name = if selected_model.model_id.contains("::") {
            selected_model.model_id.split("::").nth(1).unwrap_or(&selected_model.model_id)
        } else {
            &selected_model.model_id
        };

        let response = client
            .generate(model_name, &prompt, options)
            .await
            .context("Failed to generate bias analysis")?;

        // Log Guardian bias analysis response at TRACE level
        trace!(
            target: "llm_messages",
            "[GUARDIAN BIAS RESPONSE] Raw response ({} chars):\n{}",
            response.text.len(),
            response.text
        );

        let result = self.parse_json_response::<BiasAnalysisResult>(&response.text)
            .context("Failed to parse bias analysis JSON")?;

        // Log parsed result
        trace!(
            target: "llm_messages",
            "[GUARDIAN BIAS RESULT] Parsed: contains_bias={}, types={:?}, severity={:?}",
            result.contains_bias,
            result.bias_types,
            result.severity
        );

        Ok(result)
    }

    /// Analyze text for harmful content
    pub async fn analyze_harm(&self, text: &str) -> Result<HarmAnalysisResult> {
        let prompt = format!(
            r#"Analyze the following text for harmful content.
Respond ONLY with valid JSON in this exact format:
{{
  "contains_harm": true/false,
  "harm_types": ["violence", "hate_speech", "self_harm", "sexual", "illegal"],
  "toxicity_score": 0.0,
  "intent": "benign/concerning/malicious/emergency",
  "risk_level": "none/low/medium/high/critical"
}}

Text to analyze:
{}

JSON response:"#,
            text
        );

        // Log Guardian harm analysis request at TRACE level
        trace!(
            target: "llm_messages",
            "[GUARDIAN HARM REQUEST] Analyzing text ({} chars):\n{}\nPrompt ({} chars):\n{}",
            text.len(),
            text,
            prompt.len(),
            prompt
        );

        let options = crate::ai_providers::providers::GenerationOptions {
            temperature: Some(0.1),
            max_tokens: Some(512),
            top_p: Some(0.9),
            stop: None,
            system: None,
        };

        let selection_context = SelectionContext::for_guardian();
        let selected_model = self
            .ai_provider_manager
            .select_model_for_agent(selection_context)
            .await
            .context("Failed to select a model for harm analysis")?;

        let client = selected_model.get_client()?;
        let model_name = if selected_model.model_id.contains("::") {
            selected_model.model_id.split("::").nth(1).unwrap_or(&selected_model.model_id)
        } else {
            &selected_model.model_id
        };

        let response = client
            .generate(model_name, &prompt, options)
            .await
            .context("Failed to generate harm analysis")?;

        // Log Guardian harm analysis response at TRACE level
        trace!(
            target: "llm_messages",
            "[GUARDIAN HARM RESPONSE] Raw response ({} chars):\n{}",
            response.text.len(),
            response.text
        );

        let result = self.parse_json_response::<HarmAnalysisResult>(&response.text)
            .context("Failed to parse harm analysis JSON")?;

        // Log parsed result
        trace!(
            target: "llm_messages",
            "[GUARDIAN HARM RESULT] Parsed: contains_harm={}, types={:?}, toxicity={}, intent={:?}, risk={:?}",
            result.contains_harm,
            result.harm_types,
            result.toxicity_score,
            result.intent,
            result.risk_level
        );

        Ok(result)
    }

    /// Parse JSON response from LLM, handling markdown code blocks
    fn parse_json_response<T: for<'de> Deserialize<'de>>(&self, response: &str) -> Result<T> {
        // Try to extract JSON from markdown code blocks
        let json_str = if response.contains("```json") {
            // Extract from ```json ... ```
            response
                .split("```json")
                .nth(1)
                .and_then(|s| s.split("```").next())
                .unwrap_or(response)
                .trim()
        } else if response.contains("```") {
            // Extract from ``` ... ```
            response
                .split("```")
                .nth(1)
                .unwrap_or(response)
                .trim()
        } else {
            response.trim()
        };

        serde_json::from_str(json_str).context("Failed to parse JSON response")
    }
}

/// PII analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiiAnalysisResult {
    pub contains_pii: bool,
    pub pii_types: Vec<String>,
    pub risk_level: RiskLevel,
    pub redacted_count: usize,
}

/// Bias analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BiasAnalysisResult {
    pub contains_bias: bool,
    pub bias_types: Vec<String>,
    pub severity: Severity,
    pub stereotype_count: usize,
}

/// Harm analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarmAnalysisResult {
    pub contains_harm: bool,
    pub harm_types: Vec<String>,
    pub toxicity_score: f32,
    pub intent: Intent,
    pub risk_level: RiskLevel,
}

/// Risk level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    None,
    Low,
    Medium,
    High,
    Critical,
}

/// Severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    None,
    Low,
    Medium,
    High,
    Critical,
}

/// Intent classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Intent {
    Benign,
    Concerning,
    Malicious,
    Emergency,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_risk_level_serde() {
        let json = r#""high""#;
        let level: RiskLevel = serde_json::from_str(json).unwrap();
        assert_eq!(level, RiskLevel::High);
    }

    #[test]
    fn test_intent_serde() {
        let json = r#""malicious""#;
        let intent: Intent = serde_json::from_str(json).unwrap();
        assert_eq!(intent, Intent::Malicious);
    }

    #[test]
    fn test_pii_result_deserialization() {
        let json = r#"{
            "contains_pii": true,
            "pii_types": ["email", "phone"],
            "risk_level": "high",
            "redacted_count": 2
        }"#;
        
        let result: PiiAnalysisResult = serde_json::from_str(json).unwrap();
        assert!(result.contains_pii);
        assert_eq!(result.pii_types.len(), 2);
        assert_eq!(result.risk_level, RiskLevel::High);
    }

    #[tokio::test]
    async fn test_parse_json_from_markdown() {
        let ai_provider_manager = Arc::new(AIProviderManager::new().await.unwrap());
        let client = GuardianOllamaClient::new(
            ai_provider_manager,
            "gemma3:4b-it".to_string(),
        );

        let response = r#"```json
{
  "contains_pii": true,
  "pii_types": ["email"],
  "risk_level": "medium",
  "redacted_count": 1
}
```"#;

        let result: PiiAnalysisResult = client.parse_json_response(response).unwrap();
        assert!(result.contains_pii);
    }
}
