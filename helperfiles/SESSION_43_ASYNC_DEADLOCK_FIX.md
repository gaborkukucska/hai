# Session 43: Async Deadlock Fix - Worker Task Assignment Stalling

**Date**: 2025-11-15  
**Status**: Implementation Ready  
**Priority**: CRITICAL - Blocks all project execution

## Problem Summary

Projects stall immediately after PM spawns workers. The system successfully:
1. ✅ UI starts
2. ✅ Admin creates project  
3. ✅ PM agent spawns with task decomposition
4. ✅ PM begins worker spawning
5. ❌ **STALLS at `worker.assign_task()` call**

### Symptoms from Terminal Logs

```
[19:08:31] Spawning CodeWorker for task: Create Food Generation
[19:08:31] Worker CodeWorker created with 0 MCP servers: []
[19:08:31] [DIAGNOSTIC] PM ... about to call assign_task
<SYSTEM HANGS HERE - NO FURTHER LOGS>
```

**Key Observation**: The diagnostic log "assign_task completed successfully" (line 864 in pm.rs) **never appears**, indicating the async call never returns.

## Root Cause Analysis

### The Async Deadlock

**File**: `hainet-persona/src/agents/worker.rs`  
**Method**: `assign_task()` (lines 280-308)  
**Issue**: Classic async RwLock deadlock

### Code Flow Analysis

```rust
// Line 280: assign_task() method begins
pub async fn assign_task(&mut self, task_id: TaskId) -> Result<()> {
    // ... state validation ...
    
    self.current_task = Some(task_id.clone());
    
    // Line 291: ACQUIRE WRITE LOCK
    let project_manager = self.project_manager.write().await;
    
    // Line 292: Use write lock
    project_manager.assign_task(&task_id, self.id.clone()).await?;
    
    // Line 295: Call async method WHILE HOLDING WRITE LOCK
    let task = self.get_task_details(&task_id).await?;
    // ^^^^^^^ THIS CAUSES DEADLOCK!
    
    // ... rest of method ...
}
```

### Why It Deadlocks

The `get_task_details()` method (lines 583-596) attempts:

```rust
async fn get_task_details(&self, task_id: &TaskId) -> Result<Task> {
    // Line 584: Try to acquire READ LOCK
    let pm = self.project_manager.read().await;
    // ^^^^ BLOCKS FOREVER because write lock from line 291 is still held
    
    // ... method body ...
}
```

### RwLock Semantics

Rust's `RwLock` guarantees:
- **Multiple readers** can hold lock simultaneously, OR
- **One writer** holds lock exclusively (blocks all readers and writers)

The deadlock occurs because:
1. `assign_task` acquires **WRITE LOCK** (line 291)
2. Write lock is **NOT dropped** before line 295
3. `get_task_details` tries to acquire **READ LOCK** (line 584)
4. Read lock request **BLOCKS** waiting for write lock to be released
5. Write lock **NEVER releases** because waiting for `get_task_details` to return
6. **DEADLOCK**: Circular wait condition

### Why This Wasn't Caught Earlier

- **Async nature hides the issue**: No compiler error, runtime deadlock
- **Lock lifetime extends across await**: Rust's async transforms create hidden state machines
- **Integration testing gap**: Unit tests may not trigger this specific async path
- **No timeout on lock acquisition**: System waits indefinitely

## The Fix

### Solution: Explicit Lock Scoping

**Principle**: Drop write lock before calling any method that needs read lock

**Implementation**: Wrap write lock usage in explicit scope block

### Code Changes

**File**: `hainet-persona/src/agents/worker.rs`  
**Method**: `assign_task()` (lines 280-308)

#### Before (DEADLOCKS)

```rust
pub async fn assign_task(&mut self, task_id: TaskId) -> Result<()> {
    // ... validation ...
    
    self.current_task = Some(task_id.clone());
    
    // Update task status to Assigned
    let project_manager = self.project_manager.write().await;
    project_manager.assign_task(&task_id, self.id.clone()).await?;
    
    // Get task details and add to session task list
    let task = self.get_task_details(&task_id).await?;
    // ^^^^ DEADLOCK HERE
    
    self.session_tasks.add_task(
        task.title.clone(), 
        Some(task.description.clone())
    );
    
    // ... logging ...
    
    Ok(())
}
```

#### After (FIXED)

```rust
pub async fn assign_task(&mut self, task_id: TaskId) -> Result<()> {
    // ... validation ...
    
    self.current_task = Some(task_id.clone());
    
    // Update task status to Assigned
    {
        let project_manager = self.project_manager.write().await;
        project_manager.assign_task(&task_id, self.id.clone()).await?;
    } // ← Write lock EXPLICITLY DROPPED here
    
    // Get task details and add to session task list
    let task = self.get_task_details(&task_id).await?;
    // ^^^^ NOW SAFE - write lock released above
    
    self.session_tasks.add_task(
        task.title.clone(), 
        Some(task.description.clone())
    );
    
    // ... logging ...
    
    Ok(())
}
```

### Change Summary

**Lines Changed**: 1 modification (add scope block)  
**Lines Added**: 2 (opening and closing braces)  
**Risk Level**: MINIMAL - Pure scoping change, no logic modified

### Technical Explanation

**Scope Block Behavior**:
```rust
{
    let lock = resource.write().await;
    // Use lock
} // ← Lock's Drop trait called here, releases lock
```

**Why This Works**:
- Rust's RAII (Resource Acquisition Is Initialization) pattern
- `RwLock` guard implements `Drop` trait
- Scope exit triggers automatic drop
- Drop releases lock immediately
- Subsequent `read()` call can now succeed

## Expected Outcomes

### Immediate Effects

1. **Worker spawning completes** ✅
   - PM's "assign_task completed successfully" log appears
   - Worker transitions to assigned state
   
2. **Worker execution begins** ✅
   - Worker enters Planning state
   - Tool discovery runs
   - Task execution starts

3. **Projects progress** ✅
   - PM manage_loop detects task completion
   - Validation workflow triggers
   - Tasks transition through states

### System Behavior Restoration

- **No hanging operations**: All async calls complete
- **Proper state transitions**: Workers move through FSM
- **Task completion**: End-to-end workflow functions
- **PM validation**: Results submitted and reviewed

## Testing Strategy

### Unit Test Verification

**Existing Test**: `test_worker_assign_task` (worker.rs line 1833)  
**Status**: Should now pass without hanging

### Integration Test

**Simple Project Test**:
1. Create project with single file-writing task
2. Verify worker assignment completes
3. Confirm task execution begins
4. Check deliverables submitted for review

### Full E2E Test

**Snake Game Project** (from original terminal logs):
1. User requests "cool browser based snake game"
2. Admin creates "Neon Snake" project
3. PM decomposes into 13 tasks
4. Workers execute tasks sequentially
5. PM validates results
6. Project completes successfully

## Implementation Plan

### Phase 1: Apply Fix ✅

1. Document plan in SESSION_43_ASYNC_DEADLOCK_FIX.md
2. Apply scope block to `worker.rs` line 287-292
3. Compile and verify no errors
4. Run existing unit tests

### Phase 2: Verification

1. Start HAI-Net Portal
2. Create simple test project
3. Monitor logs for completion
4. Verify no stalling

### Phase 3: Documentation

1. Update SESSION_39_PROJECT_STALLING_FIX.md with resolution
2. Update PROJECT_STATUS.toml with fix details
3. Add to recent_completions

## Related Issues

### Session 39 Investigation

Session 39 identified **MCP client sharing** as root cause, which was fixed in Phase 1B. However, the deadlock issue is a **separate, pre-existing bug** that was masked by the MCP issue.

### Fix Dependencies

- ✅ **Session 39 Phase 1B**: MCP client sharing (prerequisite)
- ⏳ **Session 43**: Async deadlock fix (this session)

Both fixes are required for full functionality.

## Lessons Learned

### Async Lock Patterns

**Anti-pattern** (causes deadlock):
```rust
let lock = resource.write().await;
// ... use lock ...
self.method_that_needs_lock().await; // ❌ DEADLOCK
```

**Correct pattern**:
```rust
{
    let lock = resource.write().await;
    // ... use lock ...
} // Lock dropped
self.method_that_needs_lock().await; // ✅ SAFE
```

### Best Practices

1. **Minimize lock scope**: Hold locks for shortest time possible
2. **Explicit scoping**: Use blocks to control lock lifetime
3. **Avoid await under locks**: Never call async functions while holding locks
4. **Document lock ordering**: If multiple locks needed, document acquisition order
5. **Add timeouts**: Consider `tokio::time::timeout` for lock acquisition

### Detection Strategies

1. **Static analysis**: Clippy lint for locks held across await points
2. **Async tracing**: Use `tokio-console` to visualize async tasks
3. **Integration tests**: Test full async workflows, not just individual methods
4. **Deadlock detection**: Add timeout assertions in tests

## Success Criteria

### Must Have

- [ ] Worker `assign_task` completes without hanging
- [ ] PM manage_loop progresses through tasks
- [ ] Simple single-task project completes end-to-end

### Should Have

- [ ] Multi-task project (3+ tasks) completes
- [ ] Worker execution logs show proper state transitions
- [ ] PM validation workflow functions correctly

### Nice to Have

- [ ] Snake game project (13 tasks) completes
- [ ] No timeout errors in logs
- [ ] Clean worker termination after task completion

## References

- **File**: `hainet-persona/src/agents/worker.rs` (lines 280-308, 583-596)
- **Related**: `hainet-persona/src/agents/pm.rs` (lines 836-864)
- **Documentation**: SESSION_39_PROJECT_STALLING_FIX.md
- **Rust RwLock**: https://doc.rust-lang.org/std/sync/struct.RwLock.html
- **Async book**: https://rust-lang.github.io/async-book/

## Appendix: Full Stack Trace Context

### Call Stack at Deadlock Point

```
1. User creates project via UI
2. Admin.process_user_input() → creates project
3. Admin.spawn_pm_agent() → starts PM
4. PM.initialize_and_plan() → decomposes tasks
5. PM.manage_loop() → finds executable tasks
6. PM.spawn_worker_for_task() → creates worker
7. Worker.state_machine.transition(Idle) → ready
8. PM calls worker.assign_task() ← ENTERS METHOD
9. worker.assign_task() acquires write lock
10. worker.assign_task() calls get_task_details()
11. get_task_details() tries read lock
12. ❌ DEADLOCK - waiting forever
```

### Async Transform (Conceptual)

```rust
// What the code looks like:
async fn assign_task(...) {
    let lock = self.pm.write().await;
    self.get_task_details().await;
}

// What async transform creates (simplified):
enum AssignTaskFuture {
    AcquireLock,
    CallGetDetails { lock: WriteGuard },
    Done,
}

// State machine holds lock across states!
// get_task_details() cannot proceed while lock held
```

---

**Status**: Ready for implementation  
**Estimated Fix Time**: < 5 minutes  
**Risk**: Minimal  
**Impact**: Unblocks entire project execution system
