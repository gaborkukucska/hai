//! URL fetching and content extraction

use anyhow::{Context, Result};

pub struct UrlFetcher {
    client: reqwest::Client,
}

impl UrlFetcher {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (compatible; HAI-Net/1.0)")
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("Failed to create HTTP client");

        Self { client }
    }

    pub async fn fetch(&self, url: &str, max_length: usize) -> Result<String> {
        // Validate URL
        let parsed_url = url::Url::parse(url).context("Invalid URL")?;
        
        // Only allow HTTP(S) URLs
        if parsed_url.scheme() != "http" && parsed_url.scheme() != "https" {
            anyhow::bail!("Only HTTP(S) URLs are supported");
        }

        // Fetch the page
        let response = self
            .client
            .get(url)
            .send()
            .await
            .context("Failed to fetch URL")?;

        // Check status
        if !response.status().is_success() {
            anyhow::bail!("HTTP error: {}", response.status());
        }

        // Get content type
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        // Only process HTML content
        if !content_type.contains("text/html") && !content_type.is_empty() {
            anyhow::bail!("URL does not return HTML content (got: {})", content_type);
        }

        let html = response.text().await.context("Failed to read response")?;

        // Convert HTML to clean text
        let text = self.html_to_text(&html);

        // Truncate if needed
        let truncated = if text.len() > max_length {
            format!("{}... [truncated]", &text[..max_length])
        } else {
            text
        };

        Ok(truncated)
    }

    fn html_to_text(&self, html: &str) -> String {
        use scraper::{Html, Selector};

        let document = Html::parse_document(html);

        // Remove script and style tags
        let script_selector = Selector::parse("script, style, nav, footer, header").unwrap();
        
        // Get main content
        let body_selector = Selector::parse("body").unwrap();
        
        let mut text = String::new();
        
        if let Some(body) = document.select(&body_selector).next() {
            // Remove unwanted elements
            let mut clean_html = body.html();
            for script in document.select(&script_selector) {
                clean_html = clean_html.replace(&script.html(), "");
            }

            // Convert to text using html2text
            text = html2text::from_read(clean_html.as_bytes(), 80);
        }

        // Clean up whitespace
        text.lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fetch_url() {
        let fetcher = UrlFetcher::new();
        let content = fetcher.fetch("https://example.com", 1000).await;
        
        assert!(content.is_ok());
        let content = content.unwrap();
        assert!(!content.is_empty());
    }

    #[test]
    fn test_html_to_text() {
        let fetcher = UrlFetcher::new();
        let html = r#"
            <html>
                <head><title>Test</title></head>
                <body>
                    <h1>Hello World</h1>
                    <p>This is a test.</p>
                    <script>alert('test');</script>
                </body>
            </html>
        "#;
        
        let text = fetcher.html_to_text(html);
        assert!(text.contains("Hello World"));
        assert!(text.contains("This is a test"));
        assert!(!text.contains("alert"));
    }
}
