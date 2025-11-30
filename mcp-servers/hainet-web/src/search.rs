//! Web search implementation using DuckDuckGo

use anyhow::{Context, Result};
use governor::{Quota, RateLimiter};
use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

pub struct WebSearcher {
    client: reqwest::Client,
    rate_limiter: Arc<RateLimiter<governor::state::direct::NotKeyed, governor::state::InMemoryState, governor::clock::DefaultClock>>,
}

impl WebSearcher {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (compatible; HAI-Net/1.0)")
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");

        // Rate limit: 10 requests per minute
        let quota = Quota::per_minute(NonZeroU32::new(10).unwrap());
        let rate_limiter = Arc::new(RateLimiter::direct(quota));

        Self {
            client,
            rate_limiter,
        }
    }

    pub async fn search(&self, query: &str, max_results: usize) -> Result<Vec<SearchResult>> {
        // Wait for rate limiter
        self.rate_limiter.until_ready().await;

        // Use DuckDuckGo HTML search (no API key required)
        let url = format!(
            "https://html.duckduckgo.com/html/?q={}",
            urlencoding::encode(query)
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to send search request")?;

        let html = response
            .text()
            .await
            .context("Failed to read search response")?;

        // Parse HTML results
        let results = self.parse_duckduckgo_html(&html, max_results)?;

        Ok(results)
    }

    fn parse_duckduckgo_html(&self, html: &str, max_results: usize) -> Result<Vec<SearchResult>> {
        use scraper::{Html, Selector};

        let document = Html::parse_document(html);
        
        // DuckDuckGo HTML result selectors
        let result_selector = Selector::parse(".result").unwrap();
        let title_selector = Selector::parse(".result__a").unwrap();
        let snippet_selector = Selector::parse(".result__snippet").unwrap();
        let url_selector = Selector::parse(".result__url").unwrap();

        let mut results = Vec::new();

        for result in document.select(&result_selector).take(max_results) {
            let title = result
                .select(&title_selector)
                .next()
                .map(|el| el.text().collect::<String>())
                .unwrap_or_default()
                .trim()
                .to_string();

            let snippet = result
                .select(&snippet_selector)
                .next()
                .map(|el| el.text().collect::<String>())
                .unwrap_or_default()
                .trim()
                .to_string();

            let url = result
                .select(&url_selector)
                .next()
                .map(|el| {
                    // Extract URL from display text
                    let text = el.text().collect::<String>();
                    // Clean up DuckDuckGo's URL display format
                    if text.starts_with("http") {
                        text.split_whitespace().next().unwrap_or(&text).to_string()
                    } else {
                        format!("https://{}", text.split_whitespace().next().unwrap_or(&text))
                    }
                })
                .unwrap_or_default();

            if !title.is_empty() && !url.is_empty() {
                results.push(SearchResult {
                    title,
                    url,
                    snippet,
                });
            }
        }

        if results.is_empty() {
            anyhow::bail!("No search results found for query");
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_search() {
        let searcher = WebSearcher::new();
        let results = searcher.search("Rust programming language", 3).await;
        
        assert!(results.is_ok());
        let results = results.unwrap();
        assert!(!results.is_empty());
        assert!(results.len() <= 3);
    }
}
