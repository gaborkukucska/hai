//! # START OF FILE helperfiles/PROJECT_BASED_AGENTIC_SYSTEM.md
# Project-Based Agentic System Architecture

**Version:** 2.0  
**Date:** 2025-10-22  
**Status:** Architecture Planning  

---

## Overview

This document defines the new project-based architecture for HAI-Net's multi-agent system, where the Admin AI creates and manages multiple parallel projects, each with dedicated PM (Project Manager) and Worker agents.

---

## Core Architecture Principles

### 1. **Admin AI as Orchestrator**
- Single Admin AI instance per user
- Manages multiple parallel projects
- Primary interface for all user interactions
- Monitors system health and project status

### 2. **Project as First-Class Entity**
- Each user intent becomes a discrete project
- Projects have dedicated PM and Worker agents
- Projects run in parallel, isolated from each other
- Projects have lifecycle: Created → Active → Completed/Failed

### 3. **Dynamic Agent Creation**
- PM agents created on-demand per project
- Worker agents created by PM based on needs
- Agents terminated when project completes

---

## Agent State Machines

### Admin AI States

```
Startup → Conversation / Planning → Monitoring
   ↓           ↓           ↓            ↓
  User     No clear    Complex       Active
 greeting   intent     intent      projects
```

**State Definitions:**

1. **Startup**
   - Initial state on framework launch
   - Analyze conversation history with user
   - Determine current context and user state
   - Transition to appropriate state

2. **Conversation**
   - Default state for casual interaction
   - No actionable intent detected
   - Respond to questions, provide information
   - Monitor for actionable intents

3. **Planning**
   - Complex or multi-step intent detected
   - Decompose user request into project plan
   - Create: `<plan_title>`, `<plan_overview>`, `<plan_task_list>`
   - Request framework to create project
   - Create PM agent with project-specific system prompt
   - Transition to Monitoring

4. **Monitoring**
   - Active when 1+ projects running
   - Simultaneously available for user conversation
   - Monitor project progress
   - Handle project status updates
   - Intervene if projects need guidance

### PM Agent States

```
Startup → Planning → Manage → Complete/Failed
```

**State Definitions:**

1. **Startup**
   - Receive project context from framework
   - Parse: project name, overview, initial tasks
   - Analyze requirements and constraints
   - Transition to Planning

2. **Planning**
   - Break down initial tasks into detailed subtasks
   - Create milestones with deadlines
   - Design worker agent team (specialized roles)
   - Define success criteria
   - Request framework to create worker agents
   - Transition to Manage

3. **Manage**
   - Assign unassigned tasks to workers
   - Monitor worker progress
   - Validate worker outputs
   - Mark tasks complete after verification
   - Handle blockers and dependencies
   - Report progress to Admin AI
   - Transition to Complete when all tasks done

4. **Complete**
   - Final validation of deliverables
   - Generate completion report
   - Archive project artifacts
   - Notify Admin AI
   - Terminate PM and worker agents

5. **Failed**
   - Error state for unrecoverable failures
   - Generate failure report
   - Notify Admin AI with reasons
   - Terminate agents

### Worker Agent States

```
Idle → Working → Reporting → Idle (loop)
                     ↓
                  Complete
```

**State Definitions:**

1. **Idle**
   - Waiting for task assignment from PM
   - No active work

2. **Working**
   - Execute assigned task
   - Use MCP tools as needed
   - Track progress internally
   - Handle errors and retries

3. **Reporting**
   - Report completed work to PM
   - Provide deliverables and artifacts
   - Wait for PM validation

4. **Complete**
   - All assigned tasks finished
   - Agent terminates

---

## Project Management System

### Project Structure

```rust
pub struct Project {
    pub id: ProjectId,
    pub title: String,
    pub overview: String,
    pub status: ProjectStatus,
    
    // Agents
    pub pm_agent_id: Option<AgentId>,
    pub worker_agent_ids: Vec<AgentId>,
    
    // Tasks
    pub milestones: Vec<Milestone>,
    pub tasks: Vec<Task>,
    
    // Metadata
    pub created_at: SystemTime,
    pub started_at: Option<SystemTime>,
    pub completed_at: Option<SystemTime>,
    
    // Communication
    pub message_history: Vec<Message>,
}

pub enum ProjectStatus {
    Created,      // Project created, PM not yet assigned
    Active,       // PM managing, workers executing
    Paused,       // User paused
    Completed,    // Successfully finished
    Failed,       // Unrecoverable error
    Cancelled,    // User cancelled
}

pub struct Milestone {
    pub id: MilestoneId,
    pub title: String,
    pub description: String,
    pub deadline: Option<SystemTime>,
    pub task_ids: Vec<TaskId>,
    pub status: MilestoneStatus,
}

pub struct Task {
    pub id: TaskId,
    pub title: String,
    pub description: String,
    pub assigned_worker: Option<AgentId>,
    pub dependencies: Vec<TaskId>,
    pub status: TaskStatus,
    pub deliverables: Vec<String>,
    pub created_at: SystemTime,
    pub completed_at: Option<SystemTime>,
}

pub enum TaskStatus {
    Unassigned,
    Assigned,
    InProgress,
    Blocked,
    UnderReview,  // PM validating
    Complete,
    Failed,
}
```

### Project Lifecycle

```
User Intent → Admin AI Planning → Project Created → PM Assigned → 
PM Planning → Workers Created → Task Execution → PM Validation → 
Project Complete → Cleanup
```

**Detailed Flow:**

1. **User expresses intent** (e.g., "Build me a todo app")
2. **Admin AI** detects complex intent → Planning state
3. **Admin AI** creates plan:
   - Title: "Todo App Development"
   - Overview: "Create a web-based todo application with React and local storage"
   - Initial tasks: ["Design UI", "Implement backend", "Test application"]
4. **Framework** creates Project entity
5. **Admin AI** creates PM agent with project-specific prompt:
   ```
   You are a Project Manager for the "Todo App Development" project.
   Your goal: Create a web-based todo application with React and local storage.
   Initial tasks: Design UI, Implement backend, Test application.
   ```
6. **Framework** assigns PM to project → PM Startup state
7. **PM** breaks down tasks → creates workers → Manage state
8. **Workers** execute tasks → report to PM
9. **PM** validates work → marks complete → reports to Admin
10. **Admin** notifies user → project archived

---

## Implementation Plan

### Phase 1: Project Management Infrastructure (~60K tokens)

**Files to Create:**

1. `hainet-persona/src/projects/mod.rs` - Project management module
2. `hainet-persona/src/projects/project.rs` - Project entity and lifecycle
3. `hainet-persona/src/projects/milestone.rs` - Milestone tracking
4. `hainet-persona/src/projects/task.rs` - Task management
5. `hainet-persona/src/projects/manager.rs` - ProjectManager (create, assign, monitor)

**Key Features:**
- Project CRUD operations
- Task assignment and tracking
- Milestone management
- Project lifecycle state machine
- Multi-project parallel execution

### Phase 2: Enhanced Agent State Machines (~40K tokens)

**Files to Modify:**

1. `hainet-persona/src/agents/state.rs` - Add new states (Conversation, Monitoring)
2. `hainet-persona/src/agents/admin.rs` - Implement full Admin AI logic
3. `hainet-persona/src/agents/pm.rs` - Create PM agent (new file)
4. `hainet-persona/src/agents/worker.rs` - Create Worker agent (new file)

**Key Features:**
- Admin AI state machine with Planning and Monitoring
- PM agent startup and planning logic
- Worker agent task execution
- Inter-agent communication via projects

### Phase 3: Admin AI Planning & PM Creation (~50K tokens)

**Functionality:**

1. **Intent Analysis for Planning Trigger**
   - Detect complex/multi-step intents
   - Transition to Planning state
   - Use LLM to decompose intent

2. **Project Plan Generation**
   - Title extraction
   - Overview summarization
   - Initial task list creation
   - Request framework to create project

3. **PM Agent Creation**
   - Generate project-specific system prompt
   - Instantiate PM agent
   - Assign to project
   - Transition PM to Startup state

### Phase 4: PM Agent Planning & Worker Management (~60K tokens)

**Functionality:**

1. **PM Startup Logic**
   - Receive project context
   - Analyze initial tasks
   - Create detailed plan

2. **Detailed Task Breakdown**
   - Break tasks into subtasks
   - Define dependencies
   - Create milestones
   - Estimate timelines

3. **Worker Team Creation**
   - Identify required specializations
   - Create worker agents with specialized prompts
   - Assign initial tasks

4. **Task Assignment & Monitoring**
   - Assign tasks based on dependencies
   - Monitor worker progress
   - Validate deliverables
   - Mark tasks complete

### Phase 5: Worker Agent Execution (~40K tokens)

**Functionality:**

1. **Task Execution Loop**
   - Receive task from PM
   - Use MCP tools to execute
   - Track progress
   - Handle errors

2. **Work Reporting**
   - Report completed work to PM
   - Provide deliverables
   - Wait for validation

3. **Tool Integration**
   - File operations (hainet_file_read, hainet_file_write)
   - Network operations (hainet_http_get)
   - Compute operations (hainet_execute_command)

### Phase 6: Integration & Testing (~50K tokens)

**Tasks:**

1. End-to-end project flow testing
2. Multi-project parallel execution
3. Admin AI monitoring verification
4. Error handling and recovery
5. Performance optimization

---

## Estimated Implementation

**Total Tokens:** ~300K tokens  
**Estimated Time:** 10-12 development sessions  
**Phases:** 6  

**Breakdown:**
- Phase 1: Project infrastructure (60K tokens, 2 sessions)
- Phase 2: Agent state machines (40K tokens, 1 session)
- Phase 3: Admin AI planning (50K tokens, 2 sessions)
- Phase 4: PM agent logic (60K tokens, 2 sessions)
- Phase 5: Worker agents (40K tokens, 1 session)
- Phase 6: Integration (50K tokens, 2 sessions)

---

## Benefits of This Architecture

1. **Scalability:** Multiple parallel projects
2. **Isolation:** Projects don't interfere with each other
3. **Clarity:** Clear hierarchy (Admin → PM → Workers)
4. **Flexibility:** Dynamic agent creation per need
5. **Efficiency:** Agents terminated when not needed
6. **Monitoring:** Admin always available for user interaction
7. **Constitutional Compliance:** Guardian monitors all agents

---

## Next Steps

1. ✅ Create this architecture document
2. Update PROJECT_PLAN.md with new Phase 1 breakdown
3. Implement Phase 1: Project management infrastructure
4. Implement Phase 2: Enhanced state machines
5. Continue with Phases 3-6

---

**Last Updated:** 2025-10-22  
**Author:** HAI-Net Development Team

//! # END OF FILE helperfiles/PROJECT_BASED_AGENTIC_SYSTEM.md
