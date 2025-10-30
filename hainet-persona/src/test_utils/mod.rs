//! # START OF FILE hainet-persona/src/test_utils/mod.rs
//! Test Utilities and Production-Ready Parsing Helpers
//! 
//! This module provides robust utilities for:
//! - Multi-strategy JSON parsing with fallback mechanisms
//! - Schema validation for structured LLM outputs
//! - Retry logic with exponential backoff
//! - Error categorization and analysis
//! 
//! Originally developed for test infrastructure, these utilities are
//! production-ready and used throughout the agent system for reliable
//! LLM output processing.

pub mod json_validator;
pub mod retry;

// Re-export commonly used types
pub use json_validator::{
    JSONValidator,
    ParsingStrategy,
    ParseResult,
    ProjectPlanSchema,
    TaskDecompositionSchema,
    SchemaValidator,
};

pub use retry::{
    RetryConfig,
    retry_with_validation,
    FailureCategory,
};
