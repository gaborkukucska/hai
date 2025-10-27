<!-- # START OF FILE helperfiles/PHASE_5_DEVELOPMENT_PLAN.md -->
# HAI-Net Phase 5: Agentic Self-Management Development Plan

**Date Created:** October 28, 2025  
**Status:** In Progress (29% complete - 2/7 sessions)  
**Goal:** Complete agent intelligence system for autonomous task execution

---

## Overview

Phase 5 focuses on making the agent system fully autonomous by implementing:
1. Development tools for Worker agents (MCP)
2. PM agent task decomposition intelligence
3. Worker agent execution engine with MCP routing
4. PM-Worker communication protocol
5. End-to-end integration and testing

---

## Session Progress

### ✅ Session 1: Mobile UI-Only Deployment (Complete)
- **Date:** October 27, 2025
- **File:** `hainet-seed/src/installer/deployment.rs`
- **Feature:** Mobile device support (< 2GB RAM → UIOnly role)
- **Status:** 8/8 tests passing, clean compilation
- **LOC:** 150

### ✅ Session 2: System Management MCP (Complete)
- **Date:** October 27, 2025
- **Files:** `mcp-servers/hainet-system/` (Cargo.toml + src/main.rs)
- **Tools:** 4 system management tools
  - `system_status` - CPU/RAM monitoring
  - `list_services` - List HAI-Net services
  - `restart_service` - Restart services (whitelisted)
  - `check_health` - Health checks (4 status levels)
- **Status:** Compilation successful, ready for use
- **LOC:** 450

### 🚧 Session 3: Development Tools MCP (Next - In Progress)
- **Estimated Tokens:** 15-20K
- **Estimated LOC:** 400-500
- **Objective:** Create `hainet-dev` MCP server for Worker agents

**Tools to Implement:**
1. **git_status** - Get git repository status
2. **git_diff** - View file changes
3. **git_commit** - Commit changes with message
4. **cargo_build** - Build Rust packages
5. **cargo_test** - Run tests with filters
6. **code_search** - Search codebase (ripgrep-based)
7. **read_file_lines** - Read specific line ranges

**Implementation Steps:**
1. Create directory structure: `mcp-servers/hainet-dev/src`
2. Create `Cargo.toml` with rmcp dependencies
3. Add to workspace in root `Cargo.toml`
4. Implement MCP server using `rmcp::handler::server::ServerHandler`
5. Implement git operations using `std::process::Command`
6. Implement cargo operations (build, test)
7. Implement code search (ripgrep wrapper)
8. Test with Worker agents

**Reference Implementation:** `mcp-servers/hainet-system/src/main.rs`

### 📋 Session 4: PM Agent Task Decomposition (Planned)
- **Estimated Tokens:** 25-30K
- **Estimated LOC:** 600-700
- **Objective:** Implement LLM-powered task decomposition in PM agent

**Features:**
1. LLM-based task breakdown (use Ollama via AI providers)
2. Worker template selection based on task requirements
   - FileWorker (file operations)
   - CodeWorker (development tasks)
   - NetworkWorker (network operations)
   - ResearchWorker (knowledge gathering)
3. Task dependency graph generation
4. Worker agent spawning with specialized prompts
5. Task assignment logic

**Files to Modify:**
- `hainet-persona/src/agents/pm.rs` (+300 LOC)
- `hainet-persona/src/agents/worker.rs` (+200 LOC)
- `hainet-persona/src/agents/templates.rs` (new, 200 LOC)

**Implementation Strategy:**
1. Enhance PM agent `Planning` state with LLM task decomposition
2. Create worker templates with specialized system prompts
3. Implement task-to-worker matching algorithm
4. Build dependency graph (DAG) for task ordering
5. Implement worker spawning with project context

### 📋 Session 5: Worker Execution Engine (Planned)
- **Estimated Tokens:** 25-30K
- **Estimated LOC:** 500-600
- **Objective:** Worker task execution with MCP tool routing

**Features:**
1. Worker execution loop (`Working` state implementation)
2. MCP tool selection based on task requirements
   - File tasks → hainet-files server
   - System tasks → hainet-system server
   - Dev tasks → hainet-dev server
3. Tool result processing and error handling
4. Retry logic with exponential backoff
5. Progress reporting to PM agent

**Files to Modify:**
- `hainet-persona/src/agents/worker.rs` (+300 LOC)
- `hainet-persona/src/tools/mcp/client.rs` (+100 LOC)
- `hainet-persona/src/agents/mod.rs` (+100 LOC)

**Implementation Strategy:**
1. Implement task parsing from PM assignments
2. Build MCP tool routing logic (task type → MCP server)
3. Add error handling and retry mechanism
4. Implement progress tracking and reporting
5. Handle partial task completion

### 📋 Session 6: PM-Worker Communication & Validation (Planned)
- **Estimated Tokens:** 20-25K
- **Estimated LOC:** 400-500
- **Objective:** PM validates Worker results and manages project lifecycle

**Features:**
1. Result validation by PM (LLM-based quality check)
2. Task approval/rejection workflow
3. Dynamic re-planning on task failures
4. Milestone progress tracking
5. Project completion detection

**Files to Modify:**
- `hainet-persona/src/agents/pm.rs` (+200 LOC)
- `hainet-persona/src/projects/manager.rs` (+100 LOC)
- `hainet-persona/src/messaging/types.rs` (+50 LOC)

**Message Protocol:**
```rust
// Worker → PM: Task result
Message {
    from: worker_id,
    to: pm_id,
    content: TaskResult {
        task_id,
        status: Completed | Failed,
        deliverables: Vec<Artifact>,
        notes: String,
    }
}

// PM → Worker: Validation result
Message {
    from: pm_id,
    to: worker_id,
    content: ValidationResult {
        task_id,
        approved: bool,
        feedback: String,
        next_action: Continue | Retry | Reassign,
    }
}
```

### 📋 Session 7: End-to-End Integration & Testing (Planned)
- **Estimated Tokens:** 15-20K
- **Estimated LOC:** 300-400 (mostly tests)
- **Objective:** Full system integration and performance testing

**Features:**
1. End-to-end workflow tests
2. Multi-agent task example: "Build a TODO app"
   - User request → Admin AI
   - Admin creates project → Spawns PM
   - PM decomposes → Spawns CodeWorker, FileWorker
   - Workers execute → PM validates
   - Project completes → User notified
3. Performance benchmarks
4. Error recovery testing
5. Documentation updates

**Files to Create:**
- `hainet-persona/tests/phase5_integration_test.rs` (400 LOC)
- `helperfiles/PHASE_5_COMPLETION_SUMMARY.md`

**Test Scenarios:**
1. Simple task (single worker)
2. Complex task (multiple workers, dependencies)
3. Task failure and retry
4. Worker timeout handling
5. Parallel task execution
6. Project cancellation
7. Agent hibernation and wake

---

## Success Criteria

**Phase 5 will be considered complete when:**
1. ✅ All 3 MCP servers operational (files, system, dev)
2. ⬜ PM agent can decompose complex tasks into subtasks
3. ⬜ Worker agents can execute tasks using appropriate MCP tools
4. ⬜ PM-Worker communication protocol working end-to-end
5. ⬜ Full workflow test passing: User request → Project completion
6. ⬜ Performance benchmarks meet targets:
   - Task decomposition: < 5 seconds
   - Worker execution: < 30 seconds per simple task
   - PM validation: < 3 seconds
7. ⬜ Error recovery tested and working
8. ⬜ Documentation complete

---

## Architecture Principles

**Constitutional Compliance:**
- All agent actions monitored by Guardian system
- User consent required for system-level operations
- Privacy-first: No external data sharing without permission

**Resource Efficiency:**
- LLM calls minimized (cache results where possible)
- Worker agents hibernate when idle
- MCP connections reused across tasks

**Fault Tolerance:**
- Retry logic with exponential backoff
- Graceful degradation on tool failures
- Dead letter queue for failed tasks

**Scalability:**
- Multiple parallel projects supported
- Dynamic worker pool sizing
- Load balancing across mesh network (future)

---

## Dependencies & Prerequisites

**Before Starting Session 3:**
- ✅ hainet-files MCP server (Phase 4)
- ✅ hainet-system MCP server (Session 2)
- ✅ MCP client infrastructure (Phase 4)
- ✅ Worker agent skeleton (Phase 1)
- ✅ PM agent skeleton (Phase 1)

**External Tools Required:**
- Git (for hainet-dev server)
- Cargo (for hainet-dev server)
- Ripgrep (`rg` command, optional for code search)

---

## Estimated Timeline

**Total Estimated Tokens:** 100-130K across 5 remaining sessions  
**Total Estimated LOC:** 2,300-2,900  
**Completion Target:** Phase 5 should be complete after Session 7

**Session-by-Session:**
- Session 3: 15-20K tokens (1-2 hours AI time)
- Session 4: 25-30K tokens (2-3 hours AI time)
- Session 5: 25-30K tokens (2-3 hours AI time)
- Session 6: 20-25K tokens (1-2 hours AI time)
- Session 7: 15-20K tokens (1-2 hours AI time)

---

## Next Steps

**Immediate (Session 3):**
1. ✅ Document plan in helperfiles
2. ⬜ Create `mcp-servers/hainet-dev/` directory
3. ⬜ Implement 7 development tools
4. ⬜ Test with Worker agents
5. ⬜ Update PROJECT_STATUS.toml

**After Session 3:**
Follow the plan sequentially through Sessions 4-7, updating this document after each session.

---

**Plan Approved By:** User  
**Plan Created By:** Claude (Cline Agent)  
**Last Updated:** October 28, 2025, 1:55 AM (Australia/Perth)
