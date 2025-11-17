//! Test helpers for hainet-persona integration tests

use hainet_persona::ai_providers::providers::ollama::OllamaClient;

pub const TEST_OLLAMA_ENDPOINT: &str = "http://localhost:11434";

/// Creates a new OllamaClient for testing purposes.
pub fn create_test_ollama_client() -> OllamaClient {
    OllamaClient::new(TEST_OLLAMA_ENDPOINT.to_string())
}
