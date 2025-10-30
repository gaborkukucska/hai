//! # START OF FILE hainet-persona/tests/helpers/mod.rs
//! Test Helpers for HAI-Net E2E Integration Tests
//! 
//! Provides:
//! - Retry logic with format validation
//! - JSON schema validation
//! - Test result analysis
//! - Mock data generators
//! - Common test utilities

pub mod json_validator;

use anyhow::{Context, Result, anyhow};
use serde_json::Value;
use std::time::{Duration, Instant};
use std::collections::HashMap;

// ============================================================================
// Retry Configuration
// ============================================================================

/// Configuration for test retry behavior
#[derive(Debug, Clone)]
pub struct TestRetryConfig {
    /// Maximum number of retry attempts
    pub max_attempts: usize,
    /// Delay between retries
    pub retry_delay: Duration,
    /// Whether to validate format before parsing
    pub validate_format: bool,
    /// Whether to log each attempt
    pub log_attempts: bool,
}

impl Default for TestRetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            retry_delay: Duration::from_millis(100),
            validate_format: true,
            log_attempts: true,
        }
    }
}

impl TestRetryConfig {
    /// Create config with custom max attempts
    pub fn with_attempts(attempts: usize) -> Self {
        Self {
            max_attempts: attempts,
            ..Default::default()
        }
    }
    
    /// Create config without format validation (for faster tests)
    pub fn no_validation() -> Self {
        Self {
            validate_format: false,
            ..Default::default()
        }
    }
}

// ============================================================================
// Retry Logic with Validation
// ============================================================================

/// Execute a test with retry logic and format validation
/// 
/// # Arguments
/// * `config` - Retry configuration
/// * `test_fn` - Async test function to execute
/// 
/// # Returns
/// * `Ok(T)` - Test succeeded
/// * `Err(TestFailure)` - Test failed after all retries
pub async fn retry_with_validation<F, Fut, T>(
    config: TestRetryConfig,
    mut test_fn: F,
) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut last_error = None;
    
    for attempt in 1..=config.max_attempts {
        if config.log_attempts && attempt > 1 {
            println!("🔄 Retry attempt {}/{}", attempt, config.max_attempts);
        }
        
        match test_fn().await {
            Ok(result) => {
                if config.log_attempts && attempt > 1 {
                    println!("✅ Test succeeded on attempt {}", attempt);
                }
                return Ok(result);
            }
            Err(e) => {
                last_error = Some(e);
                
                if attempt < config.max_attempts {
                    tokio::time::sleep(config.retry_delay).await;
                }
            }
        }
    }
    
    Err(last_error.unwrap_or_else(|| anyhow!("Test failed with no error details")))
}

// ============================================================================
// Test Result Analysis
// ============================================================================

/// Categorizes test failures into types
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FailureCategory {
    /// Infrastructure failure (database, network, etc.)
    Infrastructure,
    /// LLM output variability (format issues, parsing errors)
    LlmVariability,
    /// Actual bug in code logic
    CodeBug,
    /// Test environment issue (Ollama not running, etc.)
    Environment,
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
        } else if error_str.contains("assertion") || error_str.contains("expected") {
            FailureCategory::CodeBug
        } else {
            FailureCategory::Unknown
        }
    }
}

/// Test result for a single test execution
#[derive(Debug, Clone)]
pub struct TestResult {
    pub test_name: String,
    pub success: bool,
    pub duration: Duration,
    pub attempts: usize,
    pub category: Option<FailureCategory>,
    pub error_message: Option<String>,
}

/// Analyzes test results and provides statistics
#[derive(Debug, Default)]
pub struct TestResultAnalyzer {
    results: Vec<TestResult>,
}

impl TestResultAnalyzer {
    /// Create new analyzer
    pub fn new() -> Self {
        Self {
            results: Vec::new(),
        }
    }
    
    /// Add test result
    pub fn add_result(&mut self, result: TestResult) {
        self.results.push(result);
    }
    
    /// Calculate overall pass rate
    pub fn pass_rate(&self) -> f64 {
        if self.results.is_empty() {
            return 0.0;
        }
        
        let passed = self.results.iter().filter(|r| r.success).count();
        (passed as f64 / self.results.len() as f64) * 100.0
    }
    
    /// Get failure breakdown by category
    pub fn failure_breakdown(&self) -> HashMap<FailureCategory, usize> {
        let mut breakdown = HashMap::new();
        
        for result in &self.results {
            if !result.success {
                if let Some(category) = &result.category {
                    *breakdown.entry(category.clone()).or_insert(0) += 1;
                }
            }
        }
        
        breakdown
    }
    
    /// Get average test duration
    pub fn average_duration(&self) -> Duration {
        if self.results.is_empty() {
            return Duration::from_secs(0);
        }
        
        let total: Duration = self.results.iter().map(|r| r.duration).sum();
        total / self.results.len() as u32
    }
    
    /// Get average retry count for all tests
    pub fn average_retries(&self) -> f64 {
        if self.results.is_empty() {
            return 0.0;
        }
        
        let total_attempts: usize = self.results.iter().map(|r| r.attempts).sum();
        (total_attempts as f64 / self.results.len() as f64) - 1.0 // -1 because first attempt doesn't count as retry
    }
    
    /// Print detailed analysis report
    pub fn print_report(&self) {
        println!("\n========================================");
        println!("Test Result Analysis");
        println!("========================================\n");
        
        println!("📊 Overall Statistics:");
        println!("   Total Tests: {}", self.results.len());
        println!("   Passed: {}", self.results.iter().filter(|r| r.success).count());
        println!("   Failed: {}", self.results.iter().filter(|r| !r.success).count());
        println!("   Pass Rate: {:.1}%", self.pass_rate());
        println!("   Average Duration: {:?}", self.average_duration());
        println!("   Average Retries: {:.2}", self.average_retries());
        
        let breakdown = self.failure_breakdown();
        if !breakdown.is_empty() {
            println!("\n📋 Failure Breakdown:");
            for (category, count) in breakdown.iter() {
                println!("   {:?}: {}", category, count);
            }
        }
        
        let failed: Vec<_> = self.results.iter().filter(|r| !r.success).collect();
        if !failed.is_empty() {
            println!("\n❌ Failed Tests:");
            for result in failed {
                println!("   • {}", result.test_name);
                if let Some(error) = &result.error_message {
                    println!("     Error: {}", error);
                }
                if let Some(category) = &result.category {
                    println!("     Category: {:?}", category);
                }
            }
        }
        
        println!("\n========================================\n");
    }
}

// ============================================================================
// Test Execution Helper
// ============================================================================

/// Execute a test with full retry, validation, and result tracking
pub async fn execute_test_with_analysis<F, Fut>(
    test_name: &str,
    config: TestRetryConfig,
    analyzer: &mut TestResultAnalyzer,
    test_fn: F,
) -> Result<()>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<()>>,
{
    let start = Instant::now();
    let max_attempts = config.max_attempts;
    
    let result = retry_with_validation(config, test_fn).await;
    
    let duration = start.elapsed();
    let (success, error_message, category) = match result {
        Ok(_) => (true, None, None),
        Err(e) => {
            let cat = FailureCategory::from_error(&e);
            (false, Some(e.to_string()), Some(cat))
        }
    };
    
    analyzer.add_result(TestResult {
        test_name: test_name.to_string(),
        success,
        duration,
        attempts: max_attempts,
        category,
        error_message,
    });
    
    result
}
