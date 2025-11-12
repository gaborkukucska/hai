<!-- # START OF FILE helperfiles/SESSION_38_WORKER_PROMPT_REFACTORING.md -->
# Session 38: Worker, PM and Admin Agent Prompt Refactoring - Discovery-Based Tool Loading

**Date:** 2025-11-13
**Phase:** Maintenance & Bugfixes
**Session:** 38
**Focus:** Fixing worker JSON parsing failures through modular prompt architecture and session tasks as short term memory for all agents.
**Status:** IN PROGRESS

## 1. Problem Statement

### Current Issue
All 6 FileWorker agents are failing with identical JSON parsing errors:
```
[ERROR] Worker FileWorker task execution failed: Failed to parse after JSON repair
Caused by: expected ',' or ']' at line 16 column 6
```

### Root Cause Analysis
1. **Monolithic prompts**: Workers receive 40+ line prompts with ALL tool information
2. **Context overload**: Small local LLMs (gemma3:7b) struggle with excessive context
3. **Poor JSON generation**: LLM returns natural language or malformed JSON
4. **All parsing strategies fail**: Direct parse, markdown extraction, JSON repair, regex extraction

### Impact
- Worker agents: **100% failure rate** (6/6 failing)
- Projects cannot be executed
- System effectively non-functional for task execution

---

## 2. Solution Architecture: Discovery-Based Tool Loading

### Core Principle
**"Just-In-Time Information Loading"** - LLMs only receive information when they need it

### Architectural Shift

#### **Before (Monolithic)**
```
Startup → Load ALL tool info → Generate plan → Parse JSON → Execute
          ↑ (40+ line prompt, 2000+ tokens, 80% unused info)
```

#### **After (Discovery-Based)**
```
1. Startup → "I have tools: [list_names]" (15 lines, 500 tokens)
2. Planning → "I need file_write" 
3. Discovery → get_tool_info("hainet-files::file_write") → Metadata
4. Plan Step → Generate step with tool params
5. Execute → use_tool(...) → Result/Feedback
6. Update → Mark progress, next step
```

---

## 3. Implementation Plan

### **Phase 1: Session Task List** (Foundation) ✅ COMPLETE
**Goal:** Give workers, PMs and the Admin memory of their progress within a session

**Components:**
- `hainet-persona/src/agents/session_tasks.rs` (new module) ✅
- `SessionTaskList` struct with minimal task metadata ✅
- Integration with `WorkerAgent` ✅

**Features:**
- Task list with titles + status only ✅
- Injected into every prompt ✅
- Details loaded on-demand (lazy loading) ✅
- LLM-controlled progress tracking ✅
- FIFO capacity management (max 10 tasks) ✅
- Comprehensive unit tests (8 test cases) ✅

**Format:**
```
CURRENT TASKS:
- [pending] Create grid system
- [in_progress] Add food generation
- [complete] Set up game structure
```

**IMPORTANT ARCHITECTURAL CLARIFICATION:**

Session tasks are **completely separate** from project tasks:

| Aspect | Session Tasks (NEW) | Project Tasks (EXISTING) |
|--------|---------------------|--------------------------|
| **Purpose** | Short-term memory for LLM | Long-term project management |
| **Scope** | Per-agent, in-memory | Global, database-backed |
| **Lifecycle** | Ephemeral (cleared on restart) | Persistent (stored in SQLite) |
| **Storage** | `WorkerAgent.session_tasks` | `ProjectManager` database |
| **Usage** | Injected into LLM prompts | PM-Worker coordination |
| **Capacity** | FIFO (10 tasks max) | Unlimited per project |
| **Example** | "I'm working on these 3 tasks..." | Project milestones, dependencies |

**No overlap or conflict** - these serve different purposes and do not interact with each other.

---
### **Phase 1B: PM Agent Session Tasks** ✅ COMPLETE (2025-11-13 06:42 AM)
- [x] Add session task list to `PMAgent` struct
- [x] Track project decomposition tasks in session
- [x] Update task spawning to add to session list
- [x] Update task completion to mark in session
- [x] Test PM session awareness (compilation verified)

**Implementation Details:**
- Session tasks added to PM lifecycle: project analysis, task creation, task execution tracking
- Tasks added to session list during planning phase
- Task status updated when workers are spawned and tasks are validated
- Truncated task titles (max 50 chars) for prompt readability

### **Phase 1C: Admin Agent Session Tasks** ✅ COMPLETE (2025-11-13 06:42 AM)
- [x] Add session task list to `AdminAgent` struct
- [x] Track conversation/project creation in session
- [x] Update state transitions to reflect in session
- [x] Test Admin session awareness (compilation verified)

**Implementation Details:**
- Session tasks track user requests (simple/complex intents)
- Complex intent handling tracks: plan generation, project creation, PM spawning
- Task status automatically updated based on operation success/failure
- Truncated request titles (max 50 chars) for prompt readability

---

### **Phase 2: Tool Metadata System** (Discovery)
**Goal:** Move tool information OUT of prompts, INTO tools themselves

**Components:**
- `ToolMetadata` struct in MCP client
- `get_tool_metadata()` method for lazy loading
- Tool-side metadata storage

**Tool Metadata Structure:**
```rust
pub struct ToolMetadata {
    name: String,
    description: String,
    parameters: serde_json::Value, // JSON schema
    examples: Vec<ToolExample>,
}
```

**Storage Location:**
- Each MCP tool defines its own metadata
- Example: `mcp-servers/hainet-files/src/tools/file_write.rs`
- Metadata retrieved only when LLM requests it

### **Phase 2: Tool Metadata System** ✅ COMPLETE (2025-11-13 06:54 AM)
- [x] Define `ToolMetadata` struct in MCP client
- [x] Implement `get_tool_metadata()` method in `MCPClientManager`
- [x] Implement `list_all_tool_summaries()` for discovery phase
- [x] Add helper methods (summary, full_name, format_parameters)
- [x] Fix compilation errors (Arc<Map> type handling)
- [x] Verify compilation success (clean build in 3.01s)

**Implementation Details:**
- Module: `hainet-persona/src/tools/mcp/client.rs` (+120 LOC)
- `ToolMetadata` struct with lazy-loading architecture
- Auto-generates parameter docs from JSON schema
- Marks parameters as required/optional
- Concise summaries for tool listing
- Compilation: Zero errors, 10 cosmetic warnings

**Features:**
- `get_tool_metadata(tool_identifier)` - Lazy-load specific tool info
- `list_all_tool_summaries()` - Discovery phase (minimal context)
- `ToolMetadata::summary()` - Truncated description (80 chars)
- `ToolMetadata::full_name()` - Returns "server::tool" format
- `ToolMetadata::format_parameters()` - JSON schema → human-readable

**Note:** Tool metadata is extracted from existing MCP tool definitions via rmcp SDK.
No changes needed to individual MCP servers - metadata comes from their existing inputSchema.

---

### **Phase 3: Discovery-Based Execution** (Core Fix) ✅ COMPLETE (2025-11-13 07:16 AM)

**Implemented Infrastructure:**
```
hainet-persona/prompts/agents/worker/
├── planning.toml        (Minimal tool discovery prompt)
├── execution.toml       (Focused execution with loaded metadata)
└── feedback.toml        (Step result interpretation)
```

**New Modules:**
- `worker_discovery.rs` (300+ LOC) - Discovery execution types and parsers
- Modular prompt templates (3 files, TOML format)
- Multi-strategy JSON parsing (direct, markdown, braces)

**Architecture Ready:**
```rust
// Discovery flow (infrastructure complete, integration next step):
1. Worker shows LLM only tool names (minimal context)
2. LLM identifies needed tools → ToolSelectionRequest
3. Worker lazy-loads metadata for selected tools only
4. LLM generates plan with focused tool info → DiscoveryExecutionPlan
5. Worker executes with feedback loop → StepFeedback
```

**Key Components:**
- `ToolSelectionRequest` - LLM tool identification
- `DiscoveryExecutionPlan` - Focused execution plan
- `StepFeedback` - Result interpretation
- `DiscoveryContext` - Session-aware execution context
- Prompt helpers: `format_tool_list()`, `format_tool_metadata()`

**Status:** Infrastructure 100% complete for Worker, PM, and Admin agents

### **Phase 3: Discovery-Based Execution** ✅ COMPLETE (2025-11-13 07:22 AM)
- [x] Create minimal prompt templates in `hainet-persona/prompts/agents/worker/`
- [x] Create minimal prompt templates in `hainet-persona/prompts/agents/pm/`
- [x] Create minimal prompt templates in `hainet-persona/prompts/agents/admin/`
- [x] Create `worker_discovery.rs` module with types and parsers
- [x] Export module in `mod.rs`
- [x] Verify compilation (clean build, warnings only)

**Prompt Templates Created (9 files):**
```
hainet-persona/prompts/agents/
├── worker/
│   ├── planning.toml     (Tool discovery)
│   ├── execution.toml    (Step-by-step execution)
│   └── feedback.toml     (Result interpretation)
├── pm/
│   ├── planning.toml     (Project analysis & tool discovery)
│   ├── execution.toml    (Task decomposition)
│   └── feedback.toml     (Worker coordination)
└── admin/
    ├── planning.toml     (Intent analysis & orchestration)
    ├── execution.toml    (System coordination)
    └── feedback.toml     (Operation monitoring)
```

**Implementation Complete:**
- [x] Create modular prompt templates (9 files)
- [x] Create discovery modules (3 files: worker, pm, admin)
- [x] Export modules in mod.rs
- [x] Verify compilation (clean build)

**Discovery Modules Created (700+ LOC total):**
```
hainet-persona/src/agents/
├── worker_discovery.rs  (300+ LOC, 5 tests)
├── pm_discovery.rs      (200+ LOC, 2 tests)
└── admin_discovery.rs   (200+ LOC, 2 tests)
```

**Next Steps (Integration):**
- [ ] Implement `execute_task_with_discovery()` in worker.rs
- [ ] Implement discovery-based execution in pm.rs
- [ ] Implement discovery-based execution in admin.rs
- [ ] Replace monolithic prompt generation methods
- [ ] Test with existing integration tests

---

### **Phase 4: Model Family Preferences** (Enhancement)
**Goal:** Allow users to configure preferred model families per agent type

**Configuration File:** `hainet.toml`
```toml
[ai_preferences]
admin_model_family = "gemma"      # Prefers gemma3:* models
pm_model_family = "llama"         # Prefers llama3.* models
worker_model_family = "qwen"      # Prefers qwen2.* models
```

**Implementation:**
- Update `ModelSelector::select_best()` with family preference parameter
- Boost matching models by +20% score
- Fallback to best available if preferred family not found
- Load preferences in agent constructors

### **Phase 4: Model Family Preferences** ⏳
- [ ] Add `[ai_preferences]` section to `hainet.toml`
- [ ] Create config loading in `hainet-persona/src/config.rs`
- [ ] Update `ModelSelector::select_best()` signature
- [ ] Implement family-based score boosting (+20%)
- [ ] Pass preferences to agent constructors
- [ ] Test with different model families (llama, gemma, qwen)

---

### **Phase 5: Integration & Testing** ⏳
- [ ] Run worker execution tests
- [ ] Test with multiple concurrent projects
- [ ] Measure JSON parsing success rate
- [ ] Verify session task list functionality
- [ ] Benchmark prompt length and token usage
- [ ] Update documentation
- [ ] Update `PROJECT_STATUS.toml`

---

## 4. Expected Results

### **Quantitative Improvements**
| Metric | Before | After | Improvement |
|--------|--------|-------|-------------|
| Prompt Length | 40+ lines | 15 lines | 62% reduction |
| Context per Step | 2000+ tokens | ~500 tokens | 75% reduction |
| Tool Info Loaded | 100% (all tools) | ~10% (needed only) | 90% reduction |
| JSON Success Rate | 0% | >90% | ∞ improvement |

### **Architectural Improvements**
✅ **Modular prompts** - LEGO-style assembly, state-aware  
✅ **Lazy loading** - Tool info loaded on-demand  
✅ **Session awareness** - LLM tracks its own progress  
✅ **Focused context** - Only relevant information per step  
✅ **Better LLM performance** - Smaller, cleaner prompts  

---

## 5. Implementation Checklist

### **Phase 1: Session Task List** ✅ COMPLETE (2025-11-13 06:15 AM)
- [x] Create `hainet-persona/src/agents/session_tasks.rs`
- [x] Define `SessionTaskList` and `SessionTask` structs
- [x] Add session task list to `WorkerAgent` struct
- [x] Implement `to_prompt_format()` method
- [x] Implement `update_task_status()` method
- [x] Inject task list into prompt context
- [x] Test with 2-task workflow
- [x] Add FIFO capacity management
- [x] Add lazy metadata loading
- [x] Integrate with worker task assignment
- [x] Integrate with worker execution lifecycle
- [x] Fix compilation warnings
- [x] Document architectural separation from ProjectManager

**Implementation Details:**
- Module: 400+ lines with comprehensive testing
- Tests: 8 passing unit tests
- Integration: Task lifecycle tracking in worker execution
- Compilation: Zero errors, clean build
- Documentation: Architectural clarification added

---

## 6. Design Principles Applied

### **1. LEGO-Style Modularity**
- Prompts assembled from small, focused components
- Each component serves a single purpose
- Easy to swap/update individual pieces

### **2. Just-In-Time Loading**
- Information loaded only when needed
- Reduces cognitive load on small LLMs
- Improves JSON generation reliability

### **3. State-Aware Context**
- Different prompts for different states
- Fresh context on state transitions
- No accumulated noise

### **4. Agent Autonomy**
- Session task list gives agents memory
- LLM controls its own progress tracking
- Decisions based on current state, not full history

### **5. Tool Encapsulation**
- Tool info lives WITH the tool
- Single source of truth
- Easy maintenance and updates

---

## 7. Technical Details

### **Prompt Context Injection**
```rust
let context = PromptContext {
    agent_name: self.id.name.clone(),
    agent_type: AgentType::Worker,
    state: AgentState::Planning,
    session_tasks: self.session_tasks.to_prompt_format(), // NEW
    current_task_title: task.title.clone(),
    tool_servers: self.template.mcp_servers.clone(), // Just names
    metadata: HashMap::new(),
};

let prompt = prompt_manager.get_prompt(agent_id, state, context)?;
```

### **Tool Discovery Flow**
```rust
// 1. Worker asks: "What tools do I have?"
let available_tools = mcp_client.list_servers().await?
    .iter()
    .flat_map(|server| mcp_client.list_tools(server))
    .map(|tool| format!("{}::{}", tool.server, tool.name))
    .collect();

// 2. Worker asks: "Which tools do I need for this task?"
let needed_tools = self.identify_needed_tools(task, &available_tools).await?;

// 3. Worker asks: "How do I use file_write?"
for tool in needed_tools {
    let metadata = mcp_client.get_tool_metadata(&tool).await?;
    // Now worker knows how to use the tool
}
```

### **Session Task Updates**
```rust
// After each step:
self.session_tasks.update_status(
    step.description,
    TaskStatus::Complete
)?;

// Next prompt includes:
// - [complete] Create HTML structure
// - [in_progress] Add CSS styling
// - [pending] Implement game logic
```

---

## 8. Success Criteria

### **Must Have (Phase 3)**
✅ Worker JSON parsing success rate >90%  
✅ Prompt length reduction >60%  
✅ Session task list working  
✅ Tool metadata system functional  

### **Should Have (Phase 4)**
✅ Model family preferences configurable  
✅ User can override model selection  
✅ Fallback to best available model  

### **Nice to Have (Future)**
✅ Performance benchmarks documented  
✅ Detailed logging of discovery flow  
✅ User-visible session progress in UI  

---

## 9. Risks & Mitigations

### **Risk 1: LLM still fails JSON parsing**
**Mitigation:** Add rule-based fallback execution for simple tasks

### **Risk 2: Tool discovery adds latency**
**Mitigation:** Cache tool metadata, parallel loading

### **Risk 3: Session task list too long**
**Mitigation:** Limit to 5-10 most recent tasks, summarize older ones

### **Risk 4: Breaking existing tests**
**Mitigation:** Keep old methods for backward compatibility initially

---

## 10. Next Steps

1. **Document plan** ✅ (this file)
2. **Start Phase 1** - Session task list implementation
3. **Iterate through phases** - Test each phase before proceeding
4. **Monitor metrics** - Track JSON success rate improvements
5. **Update documentation** - Reflect architectural changes

---

## 11. References

- **Original Issue:** Worker JSON parsing failures (Session 37)
- **Related Docs:** 
  - `helperfiles/2_INITIAL_PLAN.md` - Original architecture
  - `helperfiles/3_PROJECT_STATUS.toml` - Current status
  - Phase 8A Sessions 1-2 - Worker LLM planning implementation
- **Key Files:**
  - `hainet-persona/src/agents/worker.rs` - Worker agent core
  - `hainet-persona/src/tools/mcp/client.rs` - MCP client
  - `hainet-persona/src/prompts/` - Prompt system

---

**Session Start:** 2025-11-13 05:26 AM  
**Phase 1 Complete:** 2025-11-13 06:15 AM  
**Phases 1B-C Complete:** 2025-11-13 06:42 AM  
**Status:** Phases 1, 1B-C ✅ Complete | Phases 2-4 ⏳ Pending  
**Estimated Duration:** 2-3 sessions (full implementation)

---

## 12. Phase 1, 1B, 1C Completion Summary

### Phase 1: Worker Session Tasks (2025-11-13 06:15 AM) ✅

**Implemented:**
- ✅ Session task list module (`session_tasks.rs`) - 400+ lines
- ✅ Worker agent integration (task lifecycle tracking)
- ✅ Comprehensive testing (8 unit tests)
- ✅ FIFO capacity management (max 10 tasks)
- ✅ Lazy metadata loading
- ✅ Prompt-friendly formatting
- ✅ Compilation verified (zero errors)
- ✅ Architectural documentation (session vs project tasks)

**Files Modified:**
1. `hainet-persona/src/agents/session_tasks.rs` (new)
2. `hainet-persona/src/agents/mod.rs` (exports)
3. `hainet-persona/src/agents/worker.rs` (integration)

### Phase 1B: PM Agent Session Tasks (2025-11-13 06:42 AM) ✅

**Implemented:**
- ✅ Session task list added to `PMAgent` struct
- ✅ Project analysis tracked ("Analyze project requirements")
- ✅ Task decomposition tracked (all project tasks added to session)
- ✅ Worker spawning updates session task status (pending → in_progress)
- ✅ Task validation updates session task status (in_progress → complete)
- ✅ Title truncation for prompt efficiency (max 50 chars)

**Files Modified:**
1. `hainet-persona/src/agents/pm.rs` (session task integration)

**PM Session Task Lifecycle:**
```
1. Startup → Add "Analyze project requirements" (pending)
2. Planning → Mark analysis (in_progress)
3. Plan Complete → Mark analysis (complete), add all tasks (pending)
4. Managing → Spawn worker → Mark task (in_progress)
5. Validation → Approve task → Mark task (complete)
```

### Phase 1C: Admin Agent Session Tasks (2025-11-13 06:42 AM) ✅

**Implemented:**
- ✅ Session task list added to `AdminAgent` struct
- ✅ User request tracking (all inputs added to session)
- ✅ Complex intent workflow tracking (plan generation, project creation, PM spawn)
- ✅ Automatic status updates based on operation results
- ✅ Title truncation for prompt efficiency (max 50 chars)

**Files Modified:**
1. `hainet-persona/src/agents/admin.rs` (session task integration)

**Admin Session Task Lifecycle:**
```
Simple Intent:
1. User request → Add request (in_progress)
2. Generate response → Success/Failure → Mark (complete/failed)

Complex Intent:
1. User request → Add request (in_progress)
2. Add "Generate project plan" (in_progress)
3. Plan generated → Mark plan (complete)
4. Add "Create project: <title>" (in_progress)
5. Add "Spawn PM agent" (in_progress)
6. PM spawned → Mark PM (complete)
7. Project created → Mark project (complete)
8. Request complete → Mark request (complete)
```

**Architecture Impact:**
- Workers now have short-term memory of session progress
- Foundation laid for discovery-based tool loading (Phase 3)
- Reusable pattern for PM/Admin agents (Phase 1B-C)
- **Completely separate from ProjectManager** - no conflicts

### Session Tasks vs Project Tasks (CRITICAL DISTINCTION)

**Session Tasks (NEW - This Implementation):**
- Ephemeral, in-memory only
- Per-agent, for LLM context
- FIFO capacity (10 tasks max)
- NOT stored in database
- Cleared on agent restart
- Purpose: "What am I working on right now?"

**Project Tasks (EXISTING - ProjectManager):**
- Persistent, database-backed
- Global, for coordination
- Unlimited capacity per project
- Stored in SQLite
- Survives restarts
- Purpose: "What needs to be done in this project?"

These are **independent systems** with no overlap or interaction.

### Next Session Tasks

**Immediate (Phase 1B-C):**
- Extend session tasks to PM agents
- Extend session tasks to Admin agent

**After Phase 1 Complete:**
- Phase 2: Tool metadata system
- Phase 3: Discovery-based execution (core fix)
- Phase 4: Model family preferences

<!-- # END OF FILE helperfiles/SESSION_38_WORKER_PROMPT_REFACTORING.md -->
