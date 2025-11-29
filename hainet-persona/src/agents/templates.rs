//! # START OF FILE hainet-persona/src/agents/templates.rs
//! Worker Agent Templates
//! 
//! Defines specialized worker archetypes with specific capabilities and system prompts.
//! Each template represents a type of work that can be performed in the HAI-Net system.

use serde::{Serialize, Deserialize};

/// Worker agent template defining capabilities and behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerTemplate {
    /// Template name (e.g., "FileWorker", "CodeWorker")
    pub name: String,
    
    /// Human-readable description of the worker's role
    pub description: String,
    
    /// List of capabilities this worker provides
    pub capabilities: Vec<String>,
    
    /// MCP servers this worker uses
    pub mcp_servers: Vec<String>,
    
    /// System prompt that guides the worker's behavior
    pub system_prompt: String,
    
    /// Keywords that help identify when this worker is needed
    pub task_keywords: Vec<String>,
}

impl WorkerTemplate {
    /// Create a FileWorker template
    /// 
    /// Specializes in file system operations using the hainet-files MCP server
    pub fn file_worker() -> Self {
        Self {
            name: "FileWorker".to_string(),
            description: "Handles file system operations including reading, writing, searching, and organizing files".to_string(),
            capabilities: vec![
                "file_read".to_string(),
                "file_write".to_string(),
                "file_list".to_string(),
                "file_search".to_string(),
                "file_metadata".to_string(),
                "directory_operations".to_string(),
            ],
            mcp_servers: vec!["hainet-files".to_string()],
            system_prompt: r#"You are a FileWorker agent specialized in file system operations.

Your capabilities:
- Read files and directories
- Write and modify files
- Search for files and content
- Organize and manage file structures
- Get file metadata and permissions

When completing tasks:
1. Use hainet-files MCP server for all file operations
2. Verify file existence before operations (use file_list or file_read)
3. NEVER use directory_create for a file path. Only use it for folders.
4. ALWAYS provide the 'path' parameter for file tools.
5. If you need to write a file, just use file_write. It will auto-create parent directories.
6. Handle errors gracefully (permissions, not found, etc.)
7. Report detailed results including file paths and sizes
8. Maintain file system integrity and organization

Always prioritize data safety and user privacy."#.to_string(),
            task_keywords: vec![
                "file".to_string(),
                "read".to_string(),
                "write".to_string(),
                "create".to_string(),
                "delete".to_string(),
                "search".to_string(),
                "directory".to_string(),
                "folder".to_string(),
                "document".to_string(),
                "text".to_string(),
            ],
        }
    }
    
    /// Create a CodeWorker template
    /// 
    /// Specializes in development tasks using the hainet-dev MCP server
    pub fn code_worker() -> Self {
        Self {
            name: "CodeWorker".to_string(),
            description: "Handles software development tasks including git operations, building, testing, and code analysis".to_string(),
            capabilities: vec![
                "git_operations".to_string(),
                "cargo_build".to_string(),
                "cargo_test".to_string(),
                "code_search".to_string(),
                "code_analysis".to_string(),
                "dependency_management".to_string(),
            ],
            mcp_servers: vec!["hainet-dev".to_string(), "hainet-files".to_string()],
            system_prompt: r#"You are a CodeWorker agent specialized in software development tasks.

Your capabilities:
- Git operations (status, diff, commit, branch management)
- Cargo operations (build, test, check, clippy)
- Code search and analysis using ripgrep
- Reading and analyzing source code
- Dependency management
- Test execution and result interpretation

When completing tasks:
1. Use hainet-dev MCP server for git and cargo operations
2. Use hainet-files MCP server for reading/writing code files
3. NEVER use directory_create for a file path. Only use it for folders.
4. ALWAYS provide the 'path' parameter for file tools.
5. If you need to write a file, just use file_write. It will auto-create parent directories.
6. Always check git status before committing
7. Run tests before marking development tasks complete
8. Search code thoroughly before making changes (verify file existence first)
9. Provide clear commit messages and documentation
10. Report compilation errors and test failures in detail

Follow software engineering best practices and maintain code quality."#.to_string(),
            task_keywords: vec![
                "code".to_string(),
                "git".to_string(),
                "commit".to_string(),
                "build".to_string(),
                "compile".to_string(),
                "test".to_string(),
                "cargo".to_string(),
                "develop".to_string(),
                "implement".to_string(),
                "fix".to_string(),
                "bug".to_string(),
                "feature".to_string(),
                "refactor".to_string(),
                "debug".to_string(),
            ],
        }
    }
    
    /// Create a NetworkWorker template
    /// 
    /// Specializes in network operations and external integrations
    pub fn network_worker() -> Self {
        Self {
            name: "NetworkWorker".to_string(),
            description: "Handles network operations, HTTP requests, and external API integrations".to_string(),
            capabilities: vec![
                "http_requests".to_string(),
                "api_integration".to_string(),
                "data_fetching".to_string(),
                "web_scraping".to_string(),
                "webhook_handling".to_string(),
            ],
            mcp_servers: vec!["hainet-files".to_string()],
            system_prompt: r#"You are a NetworkWorker agent specialized in network operations.

Your capabilities:
- HTTP/HTTPS requests (GET, POST, PUT, DELETE)
- API integration and authentication
- Data fetching from external sources
- Web scraping (respecting robots.txt)
- Webhook handling and processing
- JSON/XML data parsing

When completing tasks:
1. Use hainet-files MCP server to cache fetched data
2. Respect rate limits and API quotas
3. Handle network errors gracefully (timeouts, 404, 500)
4. Validate SSL certificates
5. Parse and structure response data
6. Log all external requests for audit
7. Never expose API keys or credentials

Privacy considerations:
- Only connect to approved external services
- Request user permission for new external connections
- Minimize data exposure to external services
- Always use encrypted connections (HTTPS)"#.to_string(),
            task_keywords: vec![
                "api".to_string(),
                "http".to_string(),
                "https".to_string(),
                "fetch".to_string(),
                "request".to_string(),
                "download".to_string(),
                "upload".to_string(),
                "webhook".to_string(),
                "rest".to_string(),
                "endpoint".to_string(),
                "external".to_string(),
                "integration".to_string(),
            ],
        }
    }
    
    /// Create a ResearchWorker template
    /// 
    /// Specializes in knowledge gathering and information synthesis
    pub fn research_worker() -> Self {
        Self {
            name: "ResearchWorker".to_string(),
            description: "Handles research, documentation, knowledge gathering, and information synthesis".to_string(),
            capabilities: vec![
                "documentation_search".to_string(),
                "information_gathering".to_string(),
                "data_analysis".to_string(),
                "report_generation".to_string(),
                "knowledge_synthesis".to_string(),
            ],
            mcp_servers: vec!["hainet-files".to_string()],
            system_prompt: r#"You are a ResearchWorker agent specialized in knowledge gathering and analysis.

Your capabilities:
- Search documentation and knowledge bases
- Gather and synthesize information from multiple sources
- Analyze data and identify patterns
- Generate comprehensive reports
- Create structured documentation
- Fact-checking and verification

When completing tasks:
1. Use hainet-files MCP server to read documentation and save reports
2. Cross-reference multiple sources for accuracy
3. Cite sources and provide references
4. Structure information logically
5. Identify gaps in knowledge and recommend further research
6. Create clear, concise summaries
7. Maintain objectivity and avoid bias

Research methodology:
- Start with broad searches, then narrow down
- Verify facts from multiple reliable sources
- Document research methodology
- Distinguish between facts and opinions
- Provide confidence levels for findings"#.to_string(),
            task_keywords: vec![
                "research".to_string(),
                "documentation".to_string(),
                "analyze".to_string(),
                "investigate".to_string(),
                "study".to_string(),
                "report".to_string(),
                "document".to_string(),
                "gather".to_string(),
                "information".to_string(),
                "knowledge".to_string(),
                "learn".to_string(),
                "understand".to_string(),
            ],
        }
    }
    
    /// Get all available worker templates
    pub fn all_templates() -> Vec<Self> {
        vec![
            Self::file_worker(),
            Self::code_worker(),
            Self::network_worker(),
            Self::research_worker(),
        ]
    }
    
    /// Select the most appropriate worker template for a task description
    /// 
    /// Uses keyword matching to determine which worker type is best suited
    pub fn select_for_task(task_description: &str) -> Self {
        let description_lower = task_description.to_lowercase();
        let templates = Self::all_templates();
        
        // Score each template based on keyword matches
        let mut scores: Vec<(usize, &WorkerTemplate)> = templates
            .iter()
            .map(|template| {
                let score = template.task_keywords.iter()
                    .filter(|keyword| description_lower.contains(keyword.as_str()))
                    .count();
                (score, template)
            })
            .collect();
        
        // Sort by score (highest first)
        scores.sort_by(|a, b| b.0.cmp(&a.0));
        
        // Return template with highest score, or FileWorker as default
        scores.sort_by(|a, b| b.0.cmp(&a.0));

        if !scores.is_empty() && scores[0].0 > 0 {
            scores[0].1.clone()
        } else {
            // Default to FileWorker for generic tasks
            Self::file_worker()
        }
    }
    
    /// Check if a task description matches this template's capabilities
    pub fn matches_task(&self, task_description: &str) -> bool {
        let description_lower = task_description.to_lowercase();
        self.task_keywords.iter()
            .any(|keyword| description_lower.contains(keyword.as_str()))
    }
    
    /// Get required MCP servers for this worker
    pub fn required_mcp_servers(&self) -> &[String] {
        &self.mcp_servers
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_file_worker_template() {
        let template = WorkerTemplate::file_worker();
        assert_eq!(template.name, "FileWorker");
        assert!(template.capabilities.contains(&"file_read".to_string()));
        assert!(template.mcp_servers.contains(&"hainet-files".to_string()));
    }
    
    #[test]
    fn test_code_worker_template() {
        let template = WorkerTemplate::code_worker();
        assert_eq!(template.name, "CodeWorker");
        assert!(template.capabilities.contains(&"git_operations".to_string()));
        assert!(template.mcp_servers.contains(&"hainet-dev".to_string()));
    }
    
    #[test]
    fn test_all_templates() {
        let templates = WorkerTemplate::all_templates();
        assert_eq!(templates.len(), 4);
        assert_eq!(templates[0].name, "FileWorker");
        assert_eq!(templates[1].name, "CodeWorker");
        assert_eq!(templates[2].name, "NetworkWorker");
        assert_eq!(templates[3].name, "ResearchWorker");
    }
    
    #[test]
    fn test_select_for_task_file() {
        let task = "Read the config file and parse its contents";
        let template = WorkerTemplate::select_for_task(task);
        assert_eq!(template.name, "FileWorker");
    }
    
    #[test]
    fn test_select_for_task_code() {
        let task = "Build the project with cargo and run tests";
        let template = WorkerTemplate::select_for_task(task);
        assert_eq!(template.name, "CodeWorker");
    }
    
    #[test]
    fn test_select_for_task_network() {
        let task = "Fetch data from the API endpoint";
        let template = WorkerTemplate::select_for_task(task);
        assert_eq!(template.name, "NetworkWorker");
    }
    
    #[test]
    fn test_select_for_task_research() {
        let task = "Research best practices for documentation";
        let template = WorkerTemplate::select_for_task(task);
        assert_eq!(template.name, "ResearchWorker");
    }
    
    #[test]
    fn test_matches_task() {
        let template = WorkerTemplate::code_worker();
        assert!(template.matches_task("Fix the bug in the authentication code"));
        assert!(template.matches_task("Implement git commit functionality"));
        assert!(!template.matches_task("Read the user manual"));
    }
    
    #[test]
    fn test_default_fallback() {
        let task = "Do something unrelated to any specific domain";
        let template = WorkerTemplate::select_for_task(task);
        assert_eq!(template.name, "FileWorker"); // Falls back to FileWorker
    }
}
