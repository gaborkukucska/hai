# Session 39: Fix Project Stalling Issue

**Date**: 2025-11-13
**Status**: Phase 1 Complete - Ready for Testing

## Problem Summary

Projects stall after Admin initiates them and PM spawns workers. The system creates projects correctly but tasks never progress beyond initial worker spawning.

### Root Cause Analysis (UPDATED - Comprehensive Analysis)

After deep code analysis, the **REAL root cause** has been identified:

**CRITICAL ISSUE: Workers are created with EMPTY MCP clients**

In `hainet-persona/src/agents/pm.rs` line 1019, workers are spawned with:
```rust
Arc::new(RwLock::new(crate::tools::mcp::MCPClientManager::new()))
```

This creates a **NEW, EMPTY MCP client** with **NO SERVERS CONNECTED**. The initialization flow:

1. **PM spawns worker** → Creates new MCPClientManager
2. **MCPClientManager::new()** → Returns empty HashMap (no servers)
3. **Worker calls discover_tools()** → `list_servers()` returns empty Vec
4. **Worker has NO TOOLS** → Planning fails or produces invalid plans
5. **Worker execution stalls** → PM never sees UnderReview status

**Secondary Issues**:
1. **Race Condition**: PM checks for tasks under review too quickly (100ms interval)
2. **No MCP operation timeouts**: Operations can hang indefinitely
3. **No error handling for empty tool list**: Worker doesn't fail gracefully
4. **No worker failure state transitions**: Workers silently hang
5. **Lack of PM-Worker coordination**: No message-based communication

### Evidence from Logs

```
[09:29:34] Spawning FileWorker for task: Design the game layout
[09:29:34] Initializing MCP Client Manager (rmcp SDK)  ← NEW EMPTY CLIENT CREATED
```

After this point - **no further progress**. Worker has no tools to execute with.

### Evidence from Logs

```
[2025-11-13][06:54:41] PM completed planning: 10 tasks with 10 dependencies
[2025-11-13][06:54:41] Spawned PM agent PM-b044641d for project b044641d
[2025-11-13][06:54:41] Spawning CodeWorker for task: Add Scoring System
[2025-11-13][06:54:41] Initializing MCP Client Manager (rmcp SDK)
```

After worker spawning, no further progress occurs. Workers execute but PM never validates their submissions.

## Solution Design - Comprehensive Fix (Option B)

### Phase 1: Fix MCP Client Initialization (CRITICAL - Must Do First) ✅

**Objective**: Share the initialized MCP client from Admin/Portal across all workers

**Root Problem**: Workers are created with empty MCP client that has no servers connected

**Solution**: Share the initialized MCP client reference across the agent hierarchy:
- Admin/Portal → PM → Workers

**Changes**:
1. Create shared MCP client in Admin or Portal initialization
2. Store MCP client reference in AgentContext or similar shared structure
3. PM receives MCP client reference from context (not from constructor directly, but from shared state)
4. PM passes **shared** MCP client to workers instead of creating new empty one
5. Workers use shared client to discover and use tools

**Files Modified**:
- `hainet-persona/src/agents/admin.rs` - Store/access shared MCP client
- `hainet-persona/src/agents/mod.rs` - Add mcp_client to shared context
- `hainet-persona/src/agents/pm.rs` - Pass shared MCP client to workers (line ~1019)
- `hainet-portal/src-tauri/src/admin_bridge.rs` - Initialize MCP client once at startup

**Expected Outcome**: Workers have access to connected MCP servers with available tools

**Priority**: CRITICAL - Nothing works without this fix

---

### Phase 2: Add MCP Operation Timeouts (Safety)

**Objective**: Prevent MCP operations from hanging indefinitely

**Changes**:
1. Wrap `discover_tools()` in tokio timeout (10 seconds)
2. Add timeout to `list_servers()` (5 seconds)
3. Return clear error messages on timeout
4. Worker transitions to Error state on timeout

**Files Modified**:
- `hainet-persona/src/agents/worker.rs` - Add timeout wrapper to discover_tools
- `hainet-persona/src/tools/mcp/client.rs` - Optional timeout parameter for operations

**Expected Outcome**: Workers fail fast with clear errors instead of hanging

**Priority**: HIGH - Prevents silent hangs

---

### Phase 3: Add Worker Failure State Handling

**Objective**: Workers properly transition to Error state and set task status to Failed

**Changes**:
1. Enhanced error handling in `execute_task()` and `execute_task_with_discovery()`
2. Transition to Error state on MCP initialization failures
3. Call `ProjectManager::fail_task()` with detailed error message
4. PM detects Failed task status in manage_loop
5. Log worker failures with full context

**Files Modified**:
- `hainet-persona/src/agents/worker.rs` - Error state transitions in execute methods
- `hainet-persona/src/agents/pm.rs` - Handle Failed status in manage_loop

**Expected Outcome**: Failed workers don't silently hang, PM can detect and report failures

**Priority**: HIGH - Enables proper error handling

---

### Phase 4: Improve PM Manage Loop (Performance)

**Objective**: PM efficiently detects completed work and validates results

**Changes**:
1. Keep increased 500ms sleep interval (already done)
2. Add comprehensive task status logging (already done)
3. Add PM-Worker message coordination (optional enhancement):
   - Worker sends `TaskCompleted` message via MessageBus on completion
   - PM listens for completion messages in manage_loop
   - PM triggers immediate validation on notification
   - Keep polling as fallback mechanism

**Files Modified**:
- `hainet-persona/src/agents/pm.rs` - Message subscription in manage_loop
- `hainet-persona/src/agents/worker.rs` - Send TaskCompleted message after complete_task
- `hainet-persona/src/messaging/mod.rs` - Define TaskCompleted message type

**Expected Outcome**: Faster task validation, better system responsiveness

**Priority**: MEDIUM - Performance improvement, not critical for functionality

---

### Phase 5: Enhanced Logging & Diagnostics

**Objective**: Make debugging easier for future issues

**Changes**:
1. Log MCP server list when worker spawns
2. Log discovered tools before worker planning
3. Log worker state transitions with timestamps
4. Add PM iteration metrics (tasks by status - already done)
5. Log MCP client initialization success/failure

**Files Modified**:
- All agent files - Enhanced tracing::info/debug calls
- `hainet-persona/src/tools/mcp/client.rs` - Log server connection status

**Expected Outcome**: Clear visibility into system behavior for debugging

**Priority**: MEDIUM - Quality of life improvement

## Implementation Order (Iterative with Testing)

### Completed
1. ✅ Document initial plan
2. ✅ Phase 1A: Quick timing fixes and logging (100ms → 500ms)
3. ✅ Bug fix: Filter embedding models from catalog

### Completed
4. ✅ **Phase 1B: Fix MCP Client Sharing** (CRITICAL - COMPLETED)
   - ✅ Step 1: Read admin initialization code
   - ✅ Step 2: Identify where MCP client is first created
   - ✅ Step 3: Create shared context for MCP client
   - ✅ Step 4: Modify PM to receive shared MCP client
   - ✅ Step 5: Modify PM to pass shared MCP client to workers
   - ⏳ **Test**: Verify workers can discover tools (READY FOR TESTING)

### In Progress

### Remaining Phases
5. ⏳ Phase 2: Add MCP operation timeouts
   - **Test**: Verify fast failure on timeout
6. ⏳ Phase 3: Worker error state handling
   - **Test**: Verify error states and Failed task status
7. ⏳ Phase 4: PM-Worker message coordination (optional)
   - **Test**: Verify message-based coordination
8. ⏳ Phase 5: Enhanced logging
   - **Test**: Verify log visibility
9. ⏳ Final integration test: Snake game project end-to-end

## Testing Strategy

### Test Cases

1. **Simple Single Task**: Create project with one file-writing task
2. **Multi-Task Sequential**: Create project with dependent tasks
3. **Multi-Task Parallel**: Create project with independent tasks
4. **Error Handling**: Test worker failure scenarios

### Success Criteria

- Workers execute tasks and submit for review
- PM validates submitted tasks within reasonable time
- Projects progress to completion
- All task state transitions logged clearly
- No hanging or stalled projects

## Code Changes

### Change 1: PM manage_loop sleep interval

**File**: `hainet-persona/src/agents/pm.rs`, line ~290

```rust
// Increase sleep interval for worker execution time
tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
```

### Change 2: Add task status logging

**File**: `hainet-persona/src/agents/pm.rs`, beginning of `manage_loop()`

Add comprehensive logging to track:
- Task status distribution per iteration
- Number of executable tasks found
- Number of tasks under review
- Worker assignments

### Change 3: Worker completion notification

**File**: `hainet-persona/src/agents/worker.rs`, after `complete_task()`

Send message to PM via MessageBus:
```rust
Message {
    type: "TaskCompleted",
    task_id: "...",
    worker_id: "..."
}
```

### Change 4: PM message handling

**File**: `hainet-persona/src/agents/pm.rs`, in `manage_loop()`

Add non-blocking message check:
- Subscribe to MessageBus
- Check for TaskCompleted messages
- Trigger immediate validation on notification

## Related Files

- `hainet-persona/src/agents/admin.rs` - Admin spawning PM
- `hainet-persona/src/agents/pm.rs` - PM manage loop
- `hainet-persona/src/agents/worker.rs` - Worker execution
- `hainet-persona/src/projects/mod.rs` - Task status management
- `hainet-persona/src/messaging/mod.rs` - MessageBus implementation

## Implementation Summary

### Phase 1 Changes (Completed)

**File**: `hainet-persona/src/agents/pm.rs`

1. **Increased manage_loop sleep interval**: 100ms → 500ms (line ~330)
   - Gives workers adequate time to execute tasks before PM checks again
   
2. **Added task status distribution logging**: Lines ~270-285
   - Logs HashMap of task statuses per iteration
   - Format: `{"Unassigned": 3, "InProgress": 2, "UnderReview": 1}`
   
3. **Added executable tasks count logging**: Lines ~290-295
   - Logs number of tasks ready for worker assignment
   
4. **Added tasks under review count logging**: Lines ~305-310
   - Logs number of tasks waiting for PM validation

**File**: `hainet-persona/src/ai_providers/discovery.rs`

5. **Filtered out embedding models**: Lines ~290-295
   - Embedding models (mxbai-embed-large, bge-*, nomic-embed-*) don't support the `generate` API
   - They only support the `embeddings` API endpoint
   - Filter prevents them from being cataloged as generation models
   - Fixes error: `"mxbai-embed-large:latest" does not support generate`

**Compilation**: ✅ Success (warnings only, no errors)

### Bug Fix (Completed)

**Issue**: PM selected embedding model `mxbai-embed-large` which doesn't support `generate` API
**Root Cause**: Model discovery was including embedding models in the catalog
**Solution**: Filter embedding models during Ollama model discovery based on model name patterns

### Phase 1B Implementation (COMPLETED ✅)

**Latest Update**: 2025-11-14 11:18

**Status**: ✅ **COMPLETED AND COMPILED SUCCESSFULLY**

**Implementation Details**:

1. **Portal Startup** (`hainet-portal/src-tauri/src/admin_bridge.rs` lines 118-138):
   - ✅ Initialize MCP servers from default config at startup
   - ✅ Log each server's initialization status (success/failure)
   - ✅ Log available servers list for diagnostics
   - ✅ Pass populated MCP client to Admin via AgentContext

2. **PM Worker Spawning** (`hainet-persona/src/agents/pm.rs` line 836):
   ```rust
   // BEFORE (created empty client):
   Arc::new(RwLock::new(crate::tools::mcp::MCPClientManager::new()))
   
   // AFTER (shares initialized client):
   self.mcp_client.clone() // ✅ Use shared MCP client with connected servers
   ```

3. **Diagnostic Logging Added**:
   - Portal logs each MCP server initialization result
   - Portal logs list of available servers
   - PM logs MCP server count when spawning workers (line 848-853)
   - Workers will now have access to all configured MCP tools

**Compilation**: ✅ Success
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.49s
```

**Next Immediate Steps**:
1. Test with simple project to verify tool discovery works
2. Continue with remaining phases iteratively

### Testing Checklist

After each phase:
- [ ] Phase 1B: Workers can list MCP servers and discover tools
- [ ] Phase 2: Workers fail fast with timeout errors (not hang)
- [ ] Phase 3: Failed workers set task status to Failed
- [ ] Phase 4: PM validates tasks faster with message coordination
- [ ] Phase 5: Logs show clear system behavior
- [ ] Final: Snake game project completes successfully

## Notes

- This fix maintains backward compatibility
- No breaking changes to agent interfaces
- Incremental improvement approach allows testing at each phase
- Message-based coordination provides future extensibility
- **Phase 1 provides improved observability and timing** - should resolve most stalling issues
