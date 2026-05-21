//! # HAI-Net Web MCP Server
//!
//! Provides web search, URL fetching, and documentation search capabilities
//! for HAI-Net worker agents.

use anyhow::Result;
use rmcp::handler::server::ServerHandler;
use rmcp::model::*;
use rmcp::service::RequestContext;
use rmcp::RoleServer;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::future::Future;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

mod search;
mod fetch;
mod cache;

use search::{WebSearcher, SearchResult};
use fetch::UrlFetcher;
use cache::ResponseCache;

/// Search results response
#[derive(Debug, Serialize, Deserialize)]
struct SearchResponse {
    query: String,
    results: Vec<SearchResult>,
    count: usize,
}

/// URL fetch response
#[derive(Debug, Serialize, Deserialize)]
struct FetchResponse {
    url: String,
    content: String,
    length: usize,
}

/// HAI-Net Web Server
#[derive(Clone)]
pub(crate) struct WebServer {
    searcher: Arc<WebSearcher>,
    fetcher: Arc<UrlFetcher>,
    cache: Arc<RwLock<ResponseCache>>,
}

impl WebServer {
    pub(crate) fn new() -> Self {
        info!("🌐 Initializing HAI-Net Web Server");
        
        Self {
            searcher: Arc::new(WebSearcher::new()),
            fetcher: Arc::new(UrlFetcher::new()),
            cache: Arc::new(RwLock::new(ResponseCache::new(100))),
        }
    }

    async fn handle_web_search(&self, query: String, max_results: usize) -> Result<String> {
        debug!("Searching web for: {} (max: {})", query, max_results);

        // Check cache
        let cache_key = format!("search:{}", query);
        {
            let cache_read = self.cache.read().await;
            if let Some(cached) = cache_read.get(&cache_key) {
                info!("Cache hit for search: {}", query);
                return Ok(cached.as_str().unwrap_or("{}").to_string());
            }
        }

        // Perform search
        let results = self.searcher.search(&query, max_results).await?;
        
        let response = SearchResponse {
            query: query.clone(),
            count: results.len(),
            results,
        };

        let json = serde_json::to_string_pretty(&response)?;

        // Cache the result
        self.cache.write().await.insert(cache_key, serde_json::Value::String(json.clone()));

        Ok(json)
    }

    async fn handle_fetch_url(&self, url: String, max_length: usize) -> Result<String> {
        debug!("Fetching URL: {} (max: {})", url, max_length);

        // Check cache
        let cache_key = format!("fetch:{}", url);
        {
            let cache_read = self.cache.read().await;
            if let Some(cached) = cache_read.get(&cache_key) {
                info!("Cache hit for URL: {}", url);
                return Ok(cached.as_str().unwrap_or("{}").to_string());
            }
        }

        // Fetch content
        let content = self.fetcher.fetch(&url, max_length).await?;
        
        let response = FetchResponse {
            url: url.clone(),
            length: content.len(),
            content,
        };

        let json = serde_json::to_string_pretty(&response)?;

        // Cache the result
        self.cache.write().await.insert(cache_key, serde_json::Value::String(json.clone()));

        Ok(json)
    }

    async fn handle_search_docs(&self, library_name: String, ecosystem: String, topic: Option<String>) -> Result<String> {
        debug!("Searching docs for: {} ({})", library_name, ecosystem);
        
        // Build search query
        let query = if let Some(topic) = topic {
            format!("{} {} documentation {}", library_name, ecosystem, topic)
        } else {
            format!("{} {} documentation", library_name, ecosystem)
        };

        // Use web search to find documentation
        self.handle_web_search(query, 3).await
    }
}

impl ServerHandler for WebServer {
    fn list_tools(
        &self,
        _params: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, ErrorData>> + Send + '_ {
        async move {
            Ok(ListToolsResult {
                tools: vec![
                    Tool {
                        name: Cow::Borrowed("web_search"),
                        title: Some("Web Search".to_string()),
                        description: Some(Cow::Borrowed("Search the web using DuckDuckGo. Returns titles, URLs, and snippets.")),
                        input_schema: Arc::new({
                            let mut map = serde_json::Map::new();
                            map.insert("type".to_string(), serde_json::json!("object"));
                            let mut props = serde_json::Map::new();
                            props.insert("query".to_string(), serde_json::json!({
                                "type": "string",
                                "description": "The search query"
                            }));
                            props.insert("max_results".to_string(), serde_json::json!({
                                "type": "integer",
                                "description": "Maximum number of results (default: 5, max: 10)",
                                "default": 5
                            }));
                            map.insert("properties".to_string(), serde_json::Value::Object(props));
                            map.insert("required".to_string(), serde_json::json!(["query"]));
                            map
                        }),
                        output_schema: None,
                        annotations: None,
                        icons: None,
                    },
                    Tool {
                        name: Cow::Borrowed("fetch_url"),
                        title: Some("Fetch URL".to_string()),
                        description: Some(Cow::Borrowed("Fetch and parse content from a URL. Returns clean text extracted from HTML.")),
                        input_schema: Arc::new({
                            let mut map = serde_json::Map::new();
                            map.insert("type".to_string(), serde_json::json!("object"));
                            let mut props = serde_json::Map::new();
                            props.insert("url".to_string(), serde_json::json!({
                                "type": "string",
                                "description": "The URL to fetch"
                            }));
                            props.insert("max_length".to_string(), serde_json::json!({
                                "type": "integer",
                                "description": "Maximum content length in characters (default: 5000)",
                                "default": 5000
                            }));
                            map.insert("properties".to_string(), serde_json::Value::Object(props));
                            map.insert("required".to_string(), serde_json::json!(["url"]));
                            map
                        }),
                        output_schema: None,
                        annotations: None,
                        icons: None,
                    },
                    Tool {
                        name: Cow::Borrowed("search_docs"),
                        title: Some("Search Documentation".to_string()),
                        description: Some(Cow::Borrowed("Search documentation for a library or framework. Supports npm, crates.io, PyPI, and more.")),
                        input_schema: Arc::new({
                            let mut map = serde_json::Map::new();
                            map.insert("type".to_string(), serde_json::json!("object"));
                            let mut props = serde_json::Map::new();
                            props.insert("library_name".to_string(), serde_json::json!({
                                "type": "string",
                                "description": "Name of the library or framework"
                            }));
                            props.insert("ecosystem".to_string(), serde_json::json!({
                                "type": "string",
                                "description": "Package ecosystem (npm, cargo, pypi, etc.)",
                                "enum": ["npm", "cargo", "pypi", "maven", "auto"],
                                "default": "auto"
                            }));
                            props.insert("topic".to_string(), serde_json::json!({
                                "type": "string",
                                "description": "Specific topic or feature to search for (optional)"
                            }));
                            map.insert("properties".to_string(), serde_json::Value::Object(props));
                            map.insert("required".to_string(), serde_json::json!(["library_name"]));
                            map
                        }),
                        output_schema: None,
                        annotations: None,
                        icons: None,
                    },
                ],
                next_cursor: None,
            })
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<CallToolResult, ErrorData>> + Send + '_ {
        async move {
            let args = request.arguments.unwrap_or_else(|| serde_json::Map::new());

            let result_text = match request.name.as_ref() {
                "web_search" => {
                    let query = args.get("query")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| ErrorData {
                            code: ErrorCode::INVALID_PARAMS,
                            message: Cow::Borrowed("Missing 'query' parameter"),
                            data: None,
                        })?
                        .to_string();
                    
                    let max_results = args.get("max_results")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(5) as usize;
                    let max_results = max_results.min(10); // Cap at 10

                    self.handle_web_search(query, max_results).await
                        .map_err(|e| ErrorData {
                            code: ErrorCode::INTERNAL_ERROR,
                            message: Cow::Owned(format!("Web search error: {}", e)),
                            data: None,
                        })?
                }
                "fetch_url" => {
                    let url = args.get("url")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| ErrorData {
                            code: ErrorCode::INVALID_PARAMS,
                            message: Cow::Borrowed("Missing 'url' parameter"),
                            data: None,
                        })?
                        .to_string();
                    
                    let max_length = args.get("max_length")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(5000) as usize;

                    self.handle_fetch_url(url, max_length).await
                        .map_err(|e| ErrorData {
                            code: ErrorCode::INTERNAL_ERROR,
                            message: Cow::Owned(format!("URL fetch error: {}", e)),
                            data: None,
                        })?
                }
                "search_docs" => {
                    let library_name = args.get("library_name")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| ErrorData {
                            code: ErrorCode::INVALID_PARAMS,
                            message: Cow::Borrowed("Missing 'library_name' parameter"),
                            data: None,
                        })?
                        .to_string();
                    
                    let ecosystem = args.get("ecosystem")
                        .and_then(|v| v.as_str())
                        .unwrap_or("auto")
                        .to_string();
                    
                    let topic = args.get("topic")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());

                    self.handle_search_docs(library_name, ecosystem, topic).await
                        .map_err(|e| ErrorData {
                            code: ErrorCode::INTERNAL_ERROR,
                            message: Cow::Owned(format!("Documentation search error: {}", e)),
                            data: None,
                        })?
                }
                _ => {
                    return Err(ErrorData {
                        code: ErrorCode::METHOD_NOT_FOUND,
                        message: Cow::Owned(format!("Unknown tool: {}", request.name)),
                        data: None,
                    });
                }
            };

            Ok(CallToolResult {
                content: vec![Annotated::new(
                    RawContent::Text(RawTextContent {
                        text: result_text,
                        meta: None,
                    }),
                    None
                )],
                is_error: None,
                structured_content: None,
                meta: None,
            })
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    info!("🌐 Starting HAI-Net Web MCP Server");

    let server = WebServer::new();

    info!("📡 Starting MCP server on stdio transport...");

    // Run the server with stdio transport
    use rmcp::service::ServiceExt;
    let running_service = server.serve(rmcp::transport::io::stdio()).await?;

    // Keep the service running until it's terminated
    running_service.waiting().await?;

    info!("🛑 HAI-Net Web MCP Server shutting down");
    Ok(())
}
