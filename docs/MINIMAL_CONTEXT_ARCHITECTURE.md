# Minimal Context Architecture for 4B Models

**Date**: 2025-11-26  
**Version**: 0.54  
**Sessions**: 57-58

## Overview

This document describes the architectural shift to minimal, just-in-time context delivery for LLM prompts, specifically optimized for 4B parameter models. This approach dramatically reduces token usage while improving LLM compliance and output quality.

## Problem Statement

### Original Issue
- **Verbose prompts**: Worker and Admin agents used ~60-line prompts with full tool metadata dumps
- **Token waste**: ~1500+ tokens per planning call spent on unused tool information
- **LLM confusion**: 4B models overwhelmed by verbose context, leading to:
  - Explanatory text before JSON output
  - Malformed JSON with missing required fields
  - Project stalls due to parsing failures

### Root Cause
Prompts included complete metadata for ALL available tools upfront, regardless of whether they would be used:
```
TOOLS:
hainet-files::file_search:
  <full parameter schema, description, examples>
hainet-files::file_write:
  <full parameter schema, description, examples>
... (10+ tools with full metadata)
```

## Solution: Just-In-Time Tool Discovery

### Core Principle
**Provide minimal context upfront, deliver detailed information only when needed.**

### Implementation Pattern

#### 1. Minimal Tool List
Instead of full metadata, agents receive only tool names:
```
AVAILABLE TOOLS:
- hainet-files::file_search
- hainet-files::file_write
- hainet-files::directory_create
- worker::get_tool_info  (or admin::get_tool_info)
```

#### 2. Get Tool Info Handler
Special handler that returns tool metadata on-demand:
```rust
if step.tool == "worker::get_tool_info" {
    let tool_name = step.params.get("tool_name")?;
    if let Some(metadata) = tool_metadata.get(tool_name) {
        return Ok(metadata.clone());
    }
}
```

#### 3. Minimal Prompts
Simplified from ~60 lines to ~25 lines:
```
TASK: {description}

AVAILABLE TOOLS:
{minimal_tool_list}
- worker::get_tool_info

RULES:
1. Call worker::get_tool_info before using a tool
2. Use exact tool names
3. Create directories before files

OUTPUT JSON with 5 required fields per step:
{...}
```

## Implementation Details

### Worker Agent Changes (Session 57)

**Files Modified**: 2
- `hainet-persona/src/agents/worker.rs`
- `hainet-persona/src/agents/worker_discovery.rs`

**Functions Updated**: 5
1. **`generate_execution_plan_discovery`** (line 2554)
   - Before: ~60 lines with full metadata
   - After: ~25 lines with minimal list
   - Savings: ~1000 tokens

2. **`generate_replanning_discovery`** (line 2665)
   - Before: ~65 lines with full metadata
   - After: ~30 lines with minimal list
   - Savings: ~1000 tokens

3. **`generate_plan_for_subtask`** (line 2243)
   - Before: ~45 lines with full metadata
   - After: ~35 lines with minimal list
   - Savings: ~500 tokens

4. **`identify_needed_tools_discovery`** (line 2433)
   - Already minimal (tool selection phase)
   - Uses `format_tool_list`

5. **`identify_needed_tools_for_subtask`** (line 2178)
   - Already minimal (tool selection phase)
   - Uses `format_tool_list`

**New Handler**: `worker::get_tool_info` (line 2879)
- Worker-level implementation (no MCP roundtrip)
- Returns tool metadata from internal `tool_metadata` HashMap
- Enables just-in-time discovery

**Total Impact**:
- ~2500 tokens saved per task execution
- 0 remaining uses of verbose `format_tool_metadata`

### Admin Agent Changes (Session 58)

**Files Modified**: 1
- `hainet-persona/src/agents/admin.rs`

**New Functions**: 6

1. **`discover_available_tools()`** (line 1305)
   - Lists all tools from all MCP servers
   - Returns `Vec<String>` of tool identifiers

2. **`load_tool_metadata()`** (line 1323)
   - Loads metadata for selected tools only
   - Returns `HashMap<String, String>`

3. **`execute_tool_step()`** (line 1347)
   - Executes single tool step
   - **Handles `admin::get_tool_info`** for just-in-time discovery
   - Calls MCP tools via `mcp_client`

4. **`generate_tool_execution_plan()`** (line 1383)
   - Generates minimal execution plan (~25 lines)
   - Reuses Worker's `format_tool_list` and `parse_execution_plan`

5. **`is_tool_execution_request()`** (line 1446)
   - Heuristic detection of direct tool requests
   - Keywords + length + not project creation

6. **`handle_tool_execution_request()`** (line 1465)
   - Orchestrates: Discover → Plan → Load → Execute → Format

**Request Routing** (updated `process_user_input`):
```
1. Project management commands? → handle_project_management_command()
2. Tool execution request? → handle_tool_execution_request() [NEW]
3. Complex intent? → handle_complex_intent() (create project)
4. Simple intent? → handle_simple_intent() (conversational)
```

**Total Impact**:
- ~1000 tokens saved per simple tool request
- No project overhead for direct tool execution
- Foundation for future tool management

## Architecture Benefits

### Token Efficiency
- **Worker**: ~2500 tokens saved per task
- **Admin**: ~1000 tokens saved per simple request
- **Cumulative**: Significant reduction in inference costs for 4B models

### LLM Compliance
- **Clearer focus**: Only see what's immediately needed
- **Reduced cognitive load**: Minimal context = better adherence to instructions
- **Faster inference**: Smaller prompts = faster generation

### Scalability
- **Works with any number of tools**: Minimal list scales linearly, not exponentially
- **Consistent pattern**: Same approach across Worker and Admin agents
- **Code reuse**: Shared infrastructure (`DiscoveryExecutionStep`, parsers)

### Maintainability
- **Single source of truth**: Tool metadata defined once in MCP servers
- **Easy updates**: Change tool metadata without touching agent prompts
- **Clear separation**: Tool discovery vs. tool execution

## Code Reuse

### Shared Infrastructure
- **`DiscoveryExecutionStep`**: Common execution step structure
- **`format_tool_list()`**: Minimal tool name formatting (made public)
- **`parse_execution_plan()`**: JSON plan parsing (made public)

### Pattern Consistency
Both Worker and Admin agents now use:
- Minimal tool lists upfront
- `get_tool_info` handlers for just-in-time discovery
- ~25 line execution planning prompts
- Same 5 required fields per step

## Future Enhancements

### Tool Management (Admin)
1. **Tool creation**: Admin defines new MCP tools dynamically
2. **Tool management**: Admin enables/disables tools for specific agents
3. **Tool composition**: Admin creates composite tools from existing ones
4. **Tool delegation**: Admin assigns specific tools to specific worker types

### Prompt Optimization
1. **Adaptive prompts**: Adjust verbosity based on model size
2. **Context caching**: Cache tool metadata within worker sessions
3. **Progressive disclosure**: Reveal more detail only on errors

### Performance Monitoring
1. **Token usage tracking**: Monitor actual savings in production
2. **Success rate metrics**: Track JSON parsing success rates
3. **LLM compliance**: Measure adherence to output format

## Verification Status

### Build Status
✅ **Compiled successfully** with no errors

### Testing Required
Runtime testing needed to verify:
1. LLM outputs pure JSON (no explanatory text prefix)
2. All 5 required fields present in every step
3. `get_tool_info` calls work correctly
4. Tasks complete without parsing errors
5. Admin tool execution works for simple requests

## References

- **Implementation Plan**: `/home/tom/.gemini/antigravity/brain/.../implementation_plan.md`
- **Walkthrough**: `/home/tom/.gemini/antigravity/brain/.../walkthrough.md`
- **Agent Prompt Analysis**: `/home/tom/.gemini/antigravity/brain/.../agent_prompt_analysis.md`
- **Project Status**: `helperfiles/3_PROJECT_STATUS.toml` (Sessions 57-58)

## Summary

The minimal context architecture represents a fundamental shift in how HAI-Net agents interact with LLMs:

**Before**: Dump all information upfront, hope LLM uses it correctly  
**After**: Provide minimal context, let LLM request details as needed

This approach is specifically optimized for 4B parameter models, which are highly capable but require "to-the-point, concise, in-the-moment instructions" to perform optimally. The result is more efficient, more reliable, and more maintainable agent behavior.
