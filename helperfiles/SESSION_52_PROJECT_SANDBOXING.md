# Session 52: Project-Based File Sandboxing

**Date:** 2025-11-16  
**Status:** 🚧 IN PROGRESS - Server Side Complete, Client Side Pending  
**Impact:** CRITICAL - Security isolation for worker file operations

## Problem Summary

Workers were able to create files and directories in the project root (`/home/tom/hai/`), which violates security principles. Requirements:

1. **Workers**: Sandboxed to `/sandbox/projects/[project_name]/` only
2. **Admin Agent**: Full filesystem access (with Guardian supervision and user approval)

## Solution Design

### Architecture: Project-Aware Path Sandboxing

The solution uses **optional `project_name` parameter** in MCP tool calls:

- **Workers**: Pass `project_name` (from task's project) → sandboxed to `/sandbox/projects/{name}/`
- **Admin**: Pass `None` or `"__ADMIN__"` → full filesystem access

### Why This Approach?

- ✅ Single MCP server instance serves all workers and admin
- ✅ No per-worker server instances needed
- ✅ Explicit security model (presence of project_name determines sandboxing)
- ✅ Admin bypass is explicit and auditable

## Implementation

### Phase 1: Server-Side Sandboxing ✅ COMPLETE

**File:** `mcp-servers/hainet-files/src/main.rs` (~100 LOC)

#### Key Changes:

1. **Added ADMIN_PROJECT_BYPASS constant:**
```rust
const ADMIN_PROJECT_BYPASS: &str = "__ADMIN__";
```

2. **Updated `normalize_path()` signature:**
```rust
fn normalize_path(&self, requested_path: &str, project_name: Option<&str>) -> Result<PathBuf>
```

3. **Sandboxing logic:**
```rust
let is_admin_access = match project_name {
    None => true,  // Admin: no project name
    Some(name) if name == ADMIN_PROJECT_BYPASS => true,  // Admin: explicit bypass
    Some(name) => false,  // Worker: sandboxed
};

let full_path = if is_admin_access {
    // Admin: full filesystem
    self.base_path.join(path_str)
} else {
    // Worker: sandboxed to project
    self.base_path
        .join("sandbox")
        .join("projects")
        .join(project_name.unwrap())
        .join(path_str)
};
```

4. **Updated all handlers:**
   - `handle_file_read(path, project_name)` 
   - `handle_file_write(path, content, project_name)`
   - `handle_file_list(path, project_name)`
   - `handle_file_metadata(path, project_name)`
   - `handle_directory_create(path, project_name)`

5. **Updated `call_tool` to extract project_name:**
```rust
let project_name = args.get("project_name")
    .and_then(|v| v.as_str())
    .map(|s| s.to_string());

// Pass to handlers
self.handle_file_read(path, project_name.clone()).await
```

#### Compilation Results:
```
Compiling hainet-files v0.1.0
Finished `dev` profile in 1.69s
✅ 0 errors
```

### Phase 2: Client-Side Integration 🚧 PENDING

**Remaining Work:**

1. **Update Workers** (`hainet-persona/src/agents/worker.rs`):
   - Track `current_project_name: Option<String>` field
   - Extract project name when task is assigned (from `task.project_id`)
   - Pass to MCP calls

2. **Update Worker's `execute_step()` method:**
```rust
async fn execute_step(&self, step: &ExecutionStep) -> Result<String> {
    let (server, tool) = parse_tool_name(&step.tool)?;
    
    // Add project_name to params
    let mut params = step.params.clone();
    if let Some(ref project_name) = self.current_project_name {
        params["project_name"] = Value::String(project_name.clone());
    }
    
    let mcp_client = self.mcp_client.read().await;
    mcp_client.call_tool(server, tool, params).await
}
```

3. **Update Worker's `assign_task()` method:**
```rust
pub async fn assign_task(&mut self, task_id: TaskId) -> Result<()> {
    // ... existing code ...
    
    // Get project details to extract name
    let task = self.get_task_details(&task_id).await?;
    let project = {
        let pm = self.project_manager.read().await;
        pm.get_project(&task.project_id).await?
    };
    
    // Store project name for sandboxing
    self.current_project_name = Some(project.title.clone());
    
    Ok(())
}
```

4. **Admin Agent Handling:**
   - Admin agents should set `current_project_name = None` 
   - Or use `Some("__ADMIN__".to_string())`
   - Guardian monitors admin file access

## Security Model

### Sandboxing Rules

| Agent Type | project_name | Base Path | Access Level |
|------------|-------------|-----------|--------------|
| Worker | `Some("MyProject")` | `/home/tom/hai/sandbox/projects/MyProject/` | Sandboxed |
| Admin | `None` | `/home/tom/hai/` | Full |
| Admin | `Some("__ADMIN__")` | `/home/tom/hai/` | Full |

### Path Examples

**Worker Request (project = "SnakeGame"):**
- Request: `"/src/game.py"`
- Normalized: `/home/tom/hai/sandbox/projects/SnakeGame/src/game.py`
- Security: ✅ Sandboxed

**Admin Request:**
- Request: `"/home/tom/Documents/important.txt"`
- Normalized: `/home/tom/hai/home/tom/Documents/important.txt` (strips leading /)
- Security: ✅ Guardian monitors, user approves

**Directory Traversal Blocked:**
- Request: `"../../etc/passwd"` 
- Result: ❌ Error - `".."` not allowed

## Testing Plan

### Unit Tests Needed

1. **Path normalization tests:**
   - Worker paths sandboxed correctly
   - Admin bypass works
   - Directory traversal blocked
   - Relative vs absolute paths

2. **Integration tests:**
   - Worker creates file → appears in `/sandbox/projects/{name}/`
   - Admin creates file → appears in project root
   - Workers cannot escape sandbox

### Manual Testing

```bash
# 1. Start hainet-files server
cd mcp-servers/hainet-files
cargo run

# 2. Test worker sandboxing (via MCP client)
# Should create: /home/tom/hai/sandbox/projects/TestProject/test.txt
{
  "tool": "file_write",
  "params": {
    "path": "/test.txt",
    "content": "Hello",
    "project_name": "TestProject"
  }
}

# 3. Test admin access (via MCP client)
# Should create: /home/tom/hai/admin-file.txt
{
  "tool": "file_write",
  "params": {
    "path": "/admin-file.txt",
    "content": "Admin content"
    // No project_name = admin access
  }
}
```

## Files Modified

### Completed ✅
1. `mcp-servers/hainet-files/src/main.rs` (+100 LOC, ~30 modifications)

### Pending 🚧
2. `hainet-persona/src/agents/worker.rs` (add project tracking, ~40 LOC)
3. `hainet-persona/src/agents/admin.rs` (set admin bypass, ~10 LOC)
4. `helperfiles/SESSION_52_PROJECT_SANDBOXING.md` (this file)

## Integration with Existing Features

### Session 51 Path Normalization
- ✅ Compatible - builds on top of it
- ✅ Retains directory traversal protection
- ✅ Extends with project-based sandboxing

### Guardian System
- 🔄 Admin file operations should be logged
- 🔄 Guardian should monitor admin's `project_name = None` usage
- 🔄 User approval required for sensitive paths

### Project Manager
- ✅ Already tracks project.title (used as project_name)
- ✅ Tasks already have project_id
- ✅ No changes needed

## Next Steps

1. **Complete Phase 2** (Worker & Admin updates):
   - Update worker.rs to track and pass project_name
   - Update admin agent to use bypass mode
   - Test end-to-end sandboxing

2. **Guardian Integration:**
   - Log admin file operations
   - Alert on sensitive path access
   - Require user approval for admin operations

3. **Testing:**
   - Unit tests for path normalization
   - Integration tests for worker sandboxing
   - Security audit of bypass mechanisms

4. **Documentation:**
   - Update FUNCTIONS_INDEX.md
   - Update MCP_USAGE.md with sandboxing info
   - Create security guidelines

## Lessons Learned

1. **Optional Parameters Work Well**: Using `Option<String>` for project_name allows clean separation of admin vs worker
2. **Single Server Instance**: Avoids complexity of per-worker server spawning
3. **Explicit Security**: Presence/absence of project_name makes security model clear
4. **Compile-Time Safety**: Rust's type system caught all parameter mismatches

## Status Summary

**✅ Completed (60%):**
- Server-side sandboxing implementation
- Path normalization with project awareness
- Admin bypass mechanism
- Compilation successful

**🚧 Pending (40%):**
- Worker project name tracking
- Worker MCP call integration
- Admin agent bypass configuration
- End-to-end testing
- Guardian monitoring integration

**Estimated Time to Complete:** 2-3 hours
