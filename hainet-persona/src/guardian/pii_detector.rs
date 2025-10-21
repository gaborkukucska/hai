// START OF FILE hainet-persona/src/guardian/pii_detector.rs

//! PII (Personally Identifiable Information) Detection System
//!
//! This module implements hybrid regex + ML-based PII detection to enforce
//! Article I (Privacy First) of the HAI-Net Constitution.

use anyhow::Result;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

/// Regex patterns for PII detection
static EMAIL_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b").unwrap()
});

static PHONE_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b(\+?\d{1,3}[-.\\s]?)?\(?\d{3}\)?[-.\\s]?\d{3}[-.\\s]?\d{4}\b").unwrap()
});

static SSN_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b\d{3}-\d{2}-\d{4}\b").unwrap()
});

static CREDIT_CARD_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b(?:\d{4}[-\s]?){3}\d{4}\b").unwrap()
});

static IPV4_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b(?:\d{1,3}\.){3}\d{1,3}\b").unwrap()
});

/// PII report (main result type)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiiReport {
    pub contains_pii: bool,
    pub pii_types: Vec<String>,
    pub risk_level: RiskLevel,
    pub detected_patterns: Vec<String>,
}

/// Risk level for PII
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RiskLevel {
    None,
    Low,
    Medium,
    High,
    Critical,
}

/// PII detector with hybrid regex approach
pub struct PIIDetector {
    _llm_client: Option<crate::guardian::ollama_client::GuardianOllamaClient>,
}

impl PIIDetector {
    /// Create new PII detector
    pub fn new(llm_client: Option<crate::guardian::ollama_client::GuardianOllamaClient>) -> Self {
        Self { _llm_client: llm_client }
    }

    /// Detect PII in text
    pub async fn analyze(&self, text: &str) -> Result<PiiReport> {
        debug!("Scanning text for PII ({} chars)", text.len());

        let mut pii_types = Vec::new();
        let mut detected_patterns = Vec::new();

        // Regex-based detection
        if EMAIL_REGEX.is_match(text) {
            pii_types.push("email".to_string());
            for m in EMAIL_REGEX.find_iter(text) {
                detected_patterns.push(m.as_str().to_string());
            }
        }

        if PHONE_REGEX.is_match(text) {
            pii_types.push("phone".to_string());
            for m in PHONE_REGEX.find_iter(text) {
                detected_patterns.push(m.as_str().to_string());
            }
        }

        if SSN_REGEX.is_match(text) {
            pii_types.push("ssn".to_string());
            for m in SSN_REGEX.find_iter(text) {
                detected_patterns.push(m.as_str().to_string());
            }
        }

        if CREDIT_CARD_REGEX.is_match(text) {
            let matches: Vec<_> = CREDIT_CARD_REGEX.find_iter(text).collect();
            for m in matches {
                let digits: String = m.as_str().chars().filter(|c| c.is_numeric()).collect();
                if Self::luhn_check(&digits) {
                    pii_types.push("credit_card".to_string());
                    detected_patterns.push(m.as_str().to_string());
                }
            }
        }

        if IPV4_REGEX.is_match(text) {
            pii_types.push("ip_address".to_string());
            for m in IPV4_REGEX.find_iter(text) {
                if Self::is_valid_ipv4(m.as_str()) {
                    detected_patterns.push(m.as_str().to_string());
                }
            }
        }

        let contains_pii = !pii_types.is_empty();
        let risk_level = Self::calculate_risk_level(&pii_types);

        if contains_pii {
            warn!(
                "Detected {} PII types (risk: {:?})",
                pii_types.len(),
                risk_level
            );
        }

        Ok(PiiReport {
            contains_pii,
            pii_types,
            risk_level,
            detected_patterns,
        })
    }

    /// Luhn algorithm for credit card validation
    fn luhn_check(digits: &str) -> bool {
        let mut sum = 0;
        let mut double = false;

        for c in digits.chars().rev() {
            if let Some(digit) = c.to_digit(10) {
                let mut digit = digit as i32;
                if double {
                    digit *= 2;
                    if digit > 9 {
                        digit -= 9;
                    }
                }
                sum += digit;
                double = !double;
            }
        }

        sum % 10 == 0
    }

    /// Validate IPv4 address
    fn is_valid_ipv4(ip: &str) -> bool {
        ip.split('.')
            .filter_map(|octet| octet.parse::<u8>().ok())
            .count()
            == 4
    }

    /// Calculate overall risk level
    fn calculate_risk_level(pii_types: &[String]) -> RiskLevel {
        if pii_types.is_empty() {
            return RiskLevel::None;
        }

        let has_critical = pii_types.iter().any(|t| t == "ssn" || t == "credit_card");
        let has_high = pii_types.iter().any(|t| t == "phone");
        let count = pii_types.len();

        if has_critical {
            RiskLevel::Critical
        } else if has_high || count >= 3 {
            RiskLevel::High
        } else if count >= 2 {
            RiskLevel::Medium
        } else {
            RiskLevel::Low
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_detect_email() {
        let detector = PIIDetector::new(None);
        let result = detector
            .analyze("Contact me at john.doe@example.com")
            .await
            .unwrap();

        assert!(result.contains_pii);
        assert!(result.pii_types.contains(&"email".to_string()));
    }

    #[tokio::test]
    async fn test_detect_ssn() {
        let detector = PIIDetector::new(None);
        let result = detector.analyze("SSN: 123-45-6789").await.unwrap();

        assert!(result.contains_pii);
        assert!(result.pii_types.contains(&"ssn".to_string()));
        assert_eq!(result.risk_level, RiskLevel::Critical);
    }

    #[tokio::test]
    async fn test_no_pii() {
        let detector = PIIDetector::new(None);
        let result = detector
            .analyze("This is a safe message with no PII")
            .await
            .unwrap();

        assert!(!result.contains_pii);
        assert_eq!(result.risk_level, RiskLevel::None);
    }
}
