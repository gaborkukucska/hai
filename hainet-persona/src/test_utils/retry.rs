//! # START OF FILE hainet-persona/src/test_utils/retry.rs
//! Retry Logic with Exponential Backoff for HAI-Net Agent System
//! 
//! Provides configurable retry mechanisms for:
//! - LLM API calls with transient failures
//! - JSON parsing with format validation
//! - Network operations with exponential backoff
//! - Error categorization for debugging
//! 
//! Originally developed for test infrastructure, now used in production
//! for reliable agent operations.

use anyhow::{Result, anyhow};
use std::time::Duration;
use tracing::{warn, debug};

// ============================================================================
// Retry Configuration
// ============================================================================

/// Configuration for retry behavior
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_attempts: usize,
    /// Base delay between retries (will be multiplied by attempt number)
    pub base_delay: Duration,
    /// Whether to log each attempt
    pub log_attempts: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(100),
            log_attempts: true,
        }
    }
}

impl RetryConfig {
    /// Create config with custom max attempts
    pub fn with_attempts(attempts: usize) -> Self {
        Self {
            max_attempts: attempts,
            ..Default::default()
        }
    }
    
    /// Create config with custom base delay
    pub fn with_delay(delay: Duration) -> Self {
        Self {
            base_delay: delay,
            ..Default::default()
        }
    }
    
    /// Create config without logging (for performance-sensitive operations)
    pub fn silent() -> Self {
        Self {
            log_attempts: false,
            ..Default::default()
        }
    }
}

// ============================================================================
// Retry Logic with Validation
// ============================================================================

/// Execute an operation with retry logic and exponential backoff
/// 
/// # Arguments
/// * `config` - Retry configuration
/// * `operation` - Async operation to execute
/// 
/// # Returns
/// * `Ok(T)` - Operation succeeded
/// * `Err(anyhow::Error)` - Operation failed after all retries
pub async fn retry_with_validation<F, Fut, T>(
    config: RetryConfig,
    mut operation: F,
) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut last_error = None;
    
    for attempt in 1..=config.max_attempts {
        if config.log_attempts && attempt > 1 {
            debug!("Retry attempt {}/{}", attempt, config.max_attempts);
        }
        
        match operation().await {
            Ok(result) => {
                if config.log_attempts && attempt > 1 {
                    debug!("Operation succeeded on attempt {}", attempt);
                }
                return Ok(result);
            }
            Err(e) => {
                if config.log_attempts {
                    warn!("Attempt {} failed: {}", attempt, e);
                }
                
                last_error = Some(e);
                
                // Exponential backoff: delay = base_delay * attempt
                if attempt < config.max_attempts {
                    let delay = config.base_delay * attempt as u32;
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }
    
    Err(last_error.unwrap_or_else(|| anyhow!("Operation failed with no error details")))
}

// ============================================================================
// Error Categorization
// ============================================================================

/// Categorizes failures into types for better error handling
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FailureCategory {
    /// Infrastructure failure (database, network, etc.)
    Infrastructure,
    /// LLM output variability (format issues, parsing errors)
    LlmVariability,
    /// Actual bug in code logic
    CodeBug,
    /// Environment issue (Ollama not running, etc.)
    Environment,
    /// Transient failure that may succeed on retry
    Transient,
    /// Unknown/uncategorized failure
    Unknown,
}

impl FailureCategory {
    /// Categorize an error based on its message
    pub fn from_error(error: &anyhow::Error) -> Self {
        let error_str = error.to_string().to_lowercase();
        
        if error_str.contains("ollama") || error_str.contains("connection refused") {
            FailureCategory::Environment
        } else if error_str.contains("json") || error_str.contains("parse") || 
                  error_str.contains("unexpected") || error_str.contains("invalid format") {
            FailureCategory::LlmVariability
        } else if error_str.contains("database") || error_str.contains("sqlite") ||
                  error_str.contains("migration") {
            FailureCategory::Infrastructure
        } else if error_str.contains("timeout") || error_str.contains("temporary") {
            FailureCategory::Transient
        } else if error_str.contains("assertion") || error_str.contains("expected") {
            FailureCategory::CodeBug
        } else {
            FailureCategory::Unknown
        }
    }
    
    /// Check if this error category is worth retrying
    pub fn should_retry(&self) -> bool {
        matches!(self, 
            FailureCategory::LlmVariability | 
            FailureCategory::Transient | 
            FailureCategory::Environment
        )
    }
}

// ============================================================================
// Specialized Retry Functions
// ============================================================================

/// Retry an LLM operation with smart categorization
pub async fn retry_llm_operation<F, Fut, T>(
    operation: F,
) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    retry_with_validation(
        RetryConfig {
            max_attempts: 3,
            base_delay: Duration::from_millis(100),
            log_attempts: true,
        },
        operation,
    ).await
}

/// Retry a parsing operation with faster retries
pub async fn retry_parse_operation<F, Fut, T>(
    operation: F,
) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    retry_with_validation(
        RetryConfig {
            max_attempts: 2,
            base_delay: Duration::from_millis(50),
            log_attempts: false,
        },
        operation,
    ).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    
    #[tokio::test]
    async fn test_retry_succeeds_on_first_attempt() {
        let result = retry_with_validation(
            RetryConfig::default(),
            || async { Ok::<_, anyhow::Error>(42) }
        ).await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }
    
    #[tokio::test]
    async fn test_retry_succeeds_on_second_attempt() {
        let attempt_count = Arc::new(Mutex::new(0));
        let attempt_count_clone = attempt_count.clone();
        
        let result = retry_with_validation(
            RetryConfig::default(),
            move || {
                let attempt_count = attempt_count_clone.clone();
                async move {
                    let mut count = attempt_count.lock().unwrap();
                    *count += 1;
                    let current_attempt = *count;
                    drop(count);
                    
                    if current_attempt < 2 {
                        Err(anyhow!("Transient failure"))
                    } else {
                        Ok::<_, anyhow::Error>(42)
                    }
                }
            }
        ).await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        assert_eq!(*attempt_count.lock().unwrap(), 2);
    }
    
    #[tokio::test]
    async fn test_retry_fails_after_max_attempts() {
        let result = retry_with_validation(
            RetryConfig::with_attempts(2),
            || async { Err::<i32, _>(anyhow!("Persistent failure")) }
        ).await;
        
        assert!(result.is_err());
    }
    
    #[test]
    fn test_failure_category_from_error() {
        let json_error = anyhow!("Failed to parse JSON");
        assert_eq!(FailureCategory::from_error(&json_error), FailureCategory::LlmVariability);
        
        let ollama_error = anyhow!("Ollama connection refused");
        assert_eq!(FailureCategory::from_error(&ollama_error), FailureCategory::Environment);
        
        let db_error = anyhow!("SQLite database error");
        assert_eq!(FailureCategory::from_error(&db_error), FailureCategory::Infrastructure);
        
        let timeout_error = anyhow!("Operation timeout");
        assert_eq!(FailureCategory::from_error(&timeout_error), FailureCategory::Transient);
    }
    
    #[test]
    fn test_should_retry() {
        assert!(FailureCategory::LlmVariability.should_retry());
        assert!(FailureCategory::Transient.should_retry());
        assert!(FailureCategory::Environment.should_retry());
        assert!(!FailureCategory::CodeBug.should_retry());
        assert!(!FailureCategory::Infrastructure.should_retry());
    }
}
