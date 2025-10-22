#### New Feature Request (Cycle 0.5):

**Auto-Install Ollama & Default Model:**
- Detect if Ollama is installed on system
- If not found, automatically install Ollama (platform-specific)
- Download default model based on system specs:
  - Tier 1 (low RAM): gemma2:2b
  - Tier 2+ (4GB+ RAM): gemma3:4b-it
- Start Ollama service if not running
- Fallback to rule-based detection if download fails
- **Implementation Location:** `hainet-seed` (installer) or `hainet-core` (bootstrap)

## Cycle 0.6: MCP Tool Ecosystem - DETAILED PLAN (2025-10-21)

**Status:** 🚧 Ready to Start  
**Estimated Time:** 4-6 hours (1 development cycle)  
**Estimated Tokens:** ~90,000 / 200,000 (45% of context window)  
**Priority:** Bridge infrastructure to agent intelligence  
**Target Completion:** 2025-10-22

### Objective

Implement the Model Context Protocol (MCP) infrastructure to enable AI agents to interact with the system and external world through standardized tool servers. This cycle completes Phase 0 and creates the foundation for Phase 1 (AI Agent Intelligence).

### Architecture Overview

```
hainet-persona/src/tools/
├── mcp/
│   ├── mod.rs              # MCP module exports (~50 LOC)
│   ├── types.rs            # Protocol types (~150 LOC)
│   ├── client.rs           # MCP client (~400 LOC)
│   └── server_manager.rs   # Lifecycle management (~200 LOC)

mcp-servers/                # External MCP server binaries
├── hainet-files/           # File operations (~500 LOC)
│   ├── Cargo.toml
│   └── src/main.rs
├── hainet-network/         # HTTP/WebSocket (~500 LOC)
│   ├── Cargo.toml
│   └── src/main.rs
├── hainet-compute/         # Task execution (~500 LOC)
│   ├── Cargo.toml
│   └── src/main.rs
├── hainet-chain/           # DID operations (~400 LOC)
│   ├── Cargo.toml
│   └── src/main.rs
└── hainet-system/          # System info (~350 LOC)
    ├── Cargo.toml
    └── src/main.rs
```

### Implementation Breakdown

#### **Part 1: MCP Protocol Types (~15K tokens, 150 LOC)**

**Module:** `hainet-persona/src/tools/mcp/types.rs`

**Core Types:**
```rust
/// JSON-RPC 2.0 Request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPRequest {
    pub jsonrpc: String,      // Always "2.0"
    pub id: u64,              // Request ID
    pub method: String,       // Tool name or "initialize"
    pub params: Value,        // Tool parameters
}

/// JSON-RPC 2.0 Response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPResponse {
    pub jsonrpc: String,
    pub id: u64,
    pub result: Option<Value>,
    pub error: Option<MCPError>,
}

/// MCP Error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPError {
    pub code: i32,
    pub message: String,
    pub data: Option<Value>,
}

/// Tool Definition (from initialize)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: ParameterSchema,
}

/// JSON Schema for parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterSchema {
    #[serde(rename = "type")]
    pub schema_type: String,
    pub properties: HashMap<String, PropertySchema>,
    pub required: Vec<String>,
}
```

**Constitutional compliance hooks:**
- All tool calls tracked for audit
- Guardian validation before execution

---

#### **Part 2: MCP Client (~25K tokens, 400 LOC)**

**Module:** `hainet-persona/src/tools/mcp/client.rs`

**Key Functionality:**

1. **Server Process Management**
   - Spawn MCP server processes (stdio transport)
   - Maintain stdin/stdout communication
   - Process lifecycle monitoring
   - Clean shutdown on exit

2. **JSON-RPC Communication**
   - Send requests to server stdin
   - Read responses from server stdout
   - Request ID tracking
   - Timeout handling (30s default)

3. **Tool Discovery**
   - Send `initialize` method on startup
   - Parse tool definitions from response
   - Cache tool schemas
   - Validate tool availability

4. **Tool Invocation**
   - Validate parameters against schema
   - Send tool call request
   - Parse tool response
   - Error handling with retries (max 3)

**Implementation:**
```rust
pub struct MCPClient {
    servers: HashMap<String, MCPServer>,
    request_counter: Arc<AtomicU64>,
}

struct MCPServer {
    name: String,
    process: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    tools: Vec<ToolDefinition>,
}

impl MCPClient {
    pub async fn new() -> Result<Self> { ... }
    
    pub async fn start_server(&mut self, name: &str, path: &str) -> Result<()> { ... }
    
    pub async fn call_tool(
        &mut self,
        server_name: &str,
        tool_name: &str,
        params: Value,
    ) -> Result<Value> { ... }
    
    pub async fn list_tools(&self, server_name: &str) -> Result<Vec<ToolDefinition>> { ... }
    
    pub async fn shutdown(&mut self) -> Result<()> { ... }
}
```

**Tests:**
- Server spawn/shutdown
- Tool discovery
- Tool invocation (success/failure)
- JSON-RPC parsing
- Timeout handling

---

#### **Part 3: MCP Server - hainet-files (~15K tokens, 500 LOC)**

**Binary:** `mcp-servers/hainet-files/src/main.rs`

**Tools:**
1. `hainet_file_read` - Read file with permission check
2. `hainet_file_write` - Write file with Guardian validation
3. `hainet_file_list` - List directory contents
4. `hainet_file_search` - Regex search across files
5. `hainet_file_delete` - Delete with confirmation
6. `hainet_file_metadata` - Get file stats

**Integration with hainet-core:**
- Uses content-addressed storage for deduplication
- BLAKE3 hashing for integrity
- Metadata tracking

**Permission System:**
- Read: User's home directory + shared folders only
- Write: Explicit whitelist (user approval required)
- Delete: Confirmation required via Guardian
- Search: Respects .gitignore patterns

**Example Tool Schema:**
```json
{
  "name": "hainet_file_read",
  "description": "Read a file from the local file system",
  "parameters": {
    "type": "object",
    "properties": {
      "path": {
        "type": "string",
        "description": "Path to the file (relative or absolute)"
      }
    },
    "required": ["path"]
  }
}
```

**Constitutional Compliance:**
- Article I (Privacy): No files leave local system without consent
- Guardian validates all write operations
- Audit trail for all file operations

---

#### **Part 4: MCP Server - hainet-network (~15K tokens, 500 LOC)**

**Binary:** `mcp-servers/hainet-network/src/main.rs`

**Tools:**
1. `hainet_http_get` - HTTP GET with privacy controls
2. `hainet_http_post` - HTTP POST with user consent
3. `hainet_websocket_connect` - WebSocket connections
4. `hainet_dns_lookup` - DNS resolution
5. `hainet_api_call` - Generic REST API wrapper

**Privacy Controls:**
- Domain whitelist (initially empty)
- User consent for new domains
- No cookies/tracking by default
- Guardian monitors all requests
- TLS/HTTPS enforced

**Rate Limiting:**
- Per-domain request limits (10/minute default)
- Exponential backoff on failures
- Resource usage tracking
- Constitutional Guardian approval for high-volume requests

**Constitutional Compliance:**
- Article I (Privacy): External requests require explicit consent
- Article II (Human Agency): User can approve/deny each domain
- Guardian blocks harmful/suspicious requests

---

#### **Part 5: MCP Server - hainet-compute (~15K tokens, 500 LOC)**

**Binary:** `mcp-servers/hainet-compute/src/main.rs`

**Tools:**
1. `hainet_execute_command` - Sandboxed shell execution
2. `hainet_run_script` - Python/Node.js script execution
3. `hainet_compile_code` - Code compilation (Rust, C, etc.)
4. `hainet_cache_result` - Result caching with content addressing

**Sandboxing:**
- Resource limits: CPU (50% max), Memory (1GB), Time (30s)
- Filesystem isolation (read-only except /tmp)
- Network restrictions (no network access by default)
- Process monitoring with auto-kill on timeout

**Integration:**
- Uses hainet-core for distributed computation (future)
- Result caching with BLAKE3 content addressing
- Guardian approval for resource-intensive tasks

**Constitutional Compliance:**
- Article II (Human Agency): User approves all command execution
- Resource limits prevent system abuse
- Audit trail for all executions

---

#### **Part 6: MCP Server - hainet-chain (~10K tokens, 400 LOC)**

**Binary:** `mcp-servers/hainet-chain/src/main.rs`

**Tools:**
1. `hainet_did_create` - Generate new DID
2. `hainet_did_verify` - Verify DID signature
3. `hainet_link_create` - Create human-AI link
4. `hainet_link_query` - Query link status
5. `hainet_identity_lookup` - Resolve DID to identity

**Integration with hainet-chain:**
- Uses existing DID system (Cycle 0.5 Phase C)
- Ed25519 cryptographic operations
- Blockchain-ready LinkRecord format

**Constitutional Compliance:**
- Article III (Decentralization): DIDs eliminate central authority
- Article V (Enforcement): Cryptographic binding verification

---

#### **Part 7: MCP Server - hainet-system (~10K tokens, 350 LOC)**

**Binary:** `mcp-servers/hainet-system/src/main.rs`

**Tools:**
1. `hainet_system_info` - OS, architecture, RAM, CPU
2. `hainet_process_list` - Running processes (sanitized)
3. `hainet_resource_usage` - CPU/memory/disk stats
4. `hainet_platform_detect` - Detect platform tier
5. `hainet_ai_providers` - List available AI providers

**Integration:**
- Uses hainet-seed platform detection
- Integrates with AI provider discovery (Cycle 0.4)
- Resource monitoring for load balancing

**Constitutional Compliance:**
- Article I (Privacy): No personal data in system info
- Safe process listing (no PII in command-line args)

---

### Testing Strategy

**Unit Tests (~15K tokens):**
- MCP protocol types serialization/deserialization
- Client request/response parsing
- Server tool schema validation
- Permission checking logic
- Resource limit enforcement

**Integration Tests (~15K tokens):**
- Client ↔ Server communication (all 5 servers)
- Multi-server orchestration
- Guardian interception of tool calls
- Error handling and retries
- Timeout scenarios

**Expected Test Count:** +40 tests (210 total)

---

### Dependencies to Add

```toml
# hainet-persona/Cargo.toml
[dependencies]
tokio-process = "0.2"   # Process spawning
nix = "0.27"            # Unix process control (for sandboxing)

# mcp-servers/*/Cargo.toml (shared)
clap = { workspace = true }
serde_json = { workspace = true }
tokio = { workspace = true }
anyhow = { workspace = true }
```

---

### Deliverables

1. ✅ MCP client infrastructure in `hainet-persona/src/tools/mcp/`
2. ✅ 5 core MCP servers as separate binaries
3. ✅ Constitutional Guardian integration for tool calls
4. ✅ Permission and consent workflow
5. ✅ Comprehensive test suite (+40 tests)
6. ✅ Documentation updates (FUNCTIONS_INDEX, PROJECT_PLAN, README)

### Success Criteria

- [ ] `cargo build --workspace` succeeds
- [ ] All 210 tests pass
- [ ] MCP client can spawn and communicate with all 5 servers
- [ ] Guardian intercepts and validates all tool calls
- [ ] File operations work with CAS integration
- [ ] Network requests require user consent
- [ ] System info tools provide accurate data
- [ ] Clean architecture with no circular dependencies

---