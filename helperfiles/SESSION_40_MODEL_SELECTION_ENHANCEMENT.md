# Session 40: Model Selection Enhancement & User Preferences

**Date**: 2025-11-14
**Status**: In Progress
**Phases**: 3 phases (Phase 2, Phase 3, Phase 1)

## Overview

This session enhances the HAI-Net model selection system with:
1. **Specialized model filters** - Filter models by task type (math, coder, vision)
2. **User-configurable preferences** - UI for preferred model families per agent type
3. **Project stalling fix verification** - Test Session 39 MCP client sharing fix

## Problem Statement

### Current Limitations

1. **Limited Model Filtering**: Only vision models are filtered. No detection for:
   - Math-specialized models (e.g., "math" in name)
   - Code-specialized models (e.g., "coder", "code" in name)
   - Task-specific model selection

2. **No User Control**: Users cannot specify preferred model families per agent type
   - Admin might prefer Llama 3 for complex reasoning
   - Workers might work better with Gemma 3 for speed
   - No UI to configure preferences

3. **Project Stalling** (Session 39): Workers created with empty MCP clients
   - Fix implemented but not yet tested
   - Need verification before moving forward

---

## Implementation Plan

### **Phase 2: Specialized Model Filters** (Priority 1)

**Goal**: Filter models based on task-specific qualifiers in model names

#### Changes

**File 1**: `hainet-persona/src/ai_providers/catalog.rs`
- Add new `ModelCapability` variants:
  - `MathematicalReasoning`
  - `CodeGeneration`
  - `CodeAnalysis`
- Update `infer_capabilities()` to detect from model names:
  - `"math"` → `MathematicalReasoning`
  - `"coder"` or `"code"` → `CodeGeneration` + `CodeAnalysis`

**LOC**: ~50

---

**File 2**: `hainet-persona/src/ai_providers/selection.rs`

**Change 2.1**: Add detection methods
```rust
impl ModelSelector {
    fn is_math_model(model_id: &str) -> bool {
        model_id.to_lowercase().contains("math")
    }
    
    fn is_coder_model(model_id: &str) -> bool {
        let lower = model_id.to_lowercase();
        lower.contains("coder") || lower.contains("code")
    }
}
```

**Change 2.2**: Extend `SelectionContext`
```rust
pub struct SelectionContext {
    // ... existing fields ...
    pub requires_math: bool,
    pub requires_coding: bool,
    pub task_type: Option<TaskType>,
}

pub enum TaskType {
    General,
    FileOperation,
    CodeGeneration,
    CodeAnalysis,
    MathematicalComputation,
    DataAnalysis,
}
```

**Change 2.3**: Add factory methods
```rust
impl SelectionContext {
    pub fn for_worker_coding() -> Self { /* ... */ }
    pub fn for_worker_math() -> Self { /* ... */ }
}
```

**Change 2.4**: Update `select_best()` filtering logic
```rust
// Prefer math models for math tasks
if context.requires_math && !Self::is_math_model(&score.model_id) {
    debug!("Preferring math models for math task, skipping {}", score.model_id);
    continue;
}

// Prefer coder models for coding tasks
if context.requires_coding && !Self::is_coder_model(&score.model_id) {
    debug!("Preferring coder models for coding task, skipping {}", score.model_id);
    continue;
}
```

**LOC**: ~150

---

#### Phase 2 Summary
- **Total LOC**: ~200
- **Files Modified**: 2
- **Time**: 2-3 hours
- **Testing**: Unit tests + integration test with coding task

---

### **Phase 3: User Model Preference System** (Priority 2)

**Goal**: Allow users to configure preferred model families per agent type with UI

#### Architecture

**Settings Structure**:
```toml
[model_preferences]
admin_family = "llama3"
pm_family = "gemma3"
worker_family = "llama3"

[model_preferences.fallback]
enabled = true
```

**UI**: Settings page with dropdowns per agent type

---

#### Changes

**File 1**: `hainet-portal/src-tauri/src/settings_storage.rs`

**Change 3.1**: Add database table
```sql
CREATE TABLE IF NOT EXISTS model_preferences (
    agent_type TEXT PRIMARY KEY,
    preferred_family TEXT,
    allow_fallback INTEGER DEFAULT 1,
    updated_at INTEGER
)
```

**Change 3.2**: Add methods
```rust
impl SettingsStorage {
    pub async fn save_model_preference(
        &self,
        agent_type: &str,
        family: &str,
        allow_fallback: bool,
    ) -> Result<()> { /* ... */ }
    
    pub async fn get_model_preference(
        &self,
        agent_type: &str,
    ) -> Result<Option<ModelPreference>> { /* ... */ }
    
    pub async fn get_all_model_preferences(&self) -> Result<Vec<ModelPreference>> { /* ... */ }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPreference {
    pub agent_type: String,
    pub preferred_family: String,
    pub allow_fallback: bool,
}
```

**LOC**: ~150

---

**File 2**: `hainet-portal/src-tauri/src/settings_handler.rs`

**Change 3.3**: Add Tauri commands
```rust
#[tauri::command]
pub async fn get_model_preferences(
    settings: State<'_, SettingsStorageState>,
) -> Result<Vec<ModelPreference>, String> { /* ... */ }

#[tauri::command]
pub async fn save_model_preference(
    agent_type: String,
    family: String,
    allow_fallback: bool,
    settings: State<'_, SettingsStorageState>,
) -> Result<(), String> { /* ... */ }
```

**Change 3.4**: Register in `lib.rs`
```rust
.invoke_handler(tauri::generate_handler![
    // ... existing ...
    get_model_preferences,
    save_model_preference,
])
```

**LOC**: ~50

---

**File 3**: `hainet-portal/src/types.ts`

**Change 3.5**: Add TypeScript interfaces
```typescript
export interface ModelPreference {
  agent_type: 'Admin' | 'PM' | 'Worker';
  preferred_family: string;
  allow_fallback: boolean;
}

export interface ModelFamily {
  id: string;
  name: string;
  description: string;
}

export const MODEL_FAMILIES: ModelFamily[] = [
  { id: 'auto', name: 'Auto (Best Available)', description: 'Automatically select best model' },
  { id: 'llama3', name: 'Llama 3', description: 'Meta\'s Llama 3 family' },
  { id: 'gemma3', name: 'Gemma 3', description: 'Google\'s Gemma 3 family' },
  { id: 'qwen', name: 'Qwen', description: 'Alibaba\'s Qwen family' },
  { id: 'deepseek', name: 'DeepSeek', description: 'DeepSeek family' },
  { id: 'phi', name: 'Phi', description: 'Microsoft\'s Phi family' },
];
```

**LOC**: ~30

---

**File 4**: `hainet-portal/src/pages/Settings.tsx`

**Change 3.6**: Add Model Preferences UI section
```tsx
<section className="model-preferences">
  <h2>Model Preferences</h2>
  <p>Configure preferred AI model families for each agent type</p>
  
  {['Admin', 'PM', 'Worker'].map(agentType => (
    <div key={agentType} className="preference-row">
      <label>{agentType} Agent:</label>
      
      <select
        value={pref?.preferred_family || 'auto'}
        onChange={(e) => updatePreference(agentType, e.target.value, pref?.allow_fallback ?? true)}
      >
        {MODEL_FAMILIES.map(family => (
          <option key={family.id} value={family.id}>{family.name}</option>
        ))}
      </select>
      
      <label className="checkbox-label">
        <input
          type="checkbox"
          checked={pref?.allow_fallback ?? true}
          onChange={(e) => updatePreference(agentType, pref?.preferred_family || 'auto', e.target.checked)}
        />
        Allow fallback to other families
      </label>
    </div>
  ))}
</section>
```

**LOC**: ~120

---

**File 5**: `hainet-persona/src/ai_providers/mod.rs`

**Change 3.7**: Add preference loading to AIProviderManager
```rust
pub struct AIProviderManager {
    // ... existing fields ...
    model_preferences: Arc<RwLock<HashMap<AgentType, ModelFamilyPreference>>>,
}

#[derive(Debug, Clone)]
pub struct ModelFamilyPreference {
    pub family_name: String,
    pub allow_fallback: bool,
}

impl AIProviderManager {
    pub async fn load_preferences(&self, db_path: &str) -> Result<()> { /* ... */ }
    
    pub async fn get_preferred_family(&self, agent_type: AgentType) -> Option<String> { /* ... */ }
}
```

**LOC**: ~80

---

**File 6**: `hainet-persona/src/ai_providers/selection.rs`

**Change 3.8**: Integrate preferences into selection
```rust
pub struct SelectionContext {
    // ... existing fields ...
    pub preferred_family: Option<String>,
    pub allow_fallback: bool,
}

pub async fn select_best(
    &self,
    ranked_models: &[ModelScore],
    context: &SelectionContext,
) -> Result<SelectedModel> {
    let preferred_family = context.preferred_family.as_ref();
    let allow_fallback = context.allow_fallback;
    
    // First pass: Try preferred family only
    if let Some(family) = preferred_family {
        if family != "auto" {
            for (index, score) in ranked_models.iter().enumerate() {
                if !score.model_id.to_lowercase().contains(&family.to_lowercase()) {
                    continue;
                }
                // ... validation checks ...
                if let Some(model) = self.check_model_suitability(score, context).await? {
                    return Ok(model);
                }
            }
            
            if !allow_fallback {
                return Err(anyhow!("No suitable {} models found", family));
            }
        }
    }
    
    // Second pass: All models (fallback or auto mode)
    // ... existing selection logic ...
}
```

**LOC**: ~100

---

#### Phase 3 Summary
- **Total LOC**: ~530
- **Files Modified**: 6
- **Time**: 5-7 hours
- **Testing**: UI testing + integration tests with preference variations

---

### **Phase 1: Test & Fix Project Stalling** (Priority 3)

**Goal**: Verify Session 39 MCP client sharing fix resolves stalling

#### Testing Steps

**Step 1**: Manual Test
1. Start Portal: `cd hainet-portal && npm run tauri dev`
2. Watch for MCP initialization logs:
   ```
   [INFO] Initializing MCP servers from config
   [INFO] MCP server 'hainet-files' started successfully
   [INFO] Available MCP servers: [hainet-files, hainet-system]
   ```
3. Create test project: "Create a hello_world.txt file with 'Hello, HAI-Net!' inside"
4. Monitor worker creation logs:
   ```
   [INFO] Spawning FileWorker for task: Create hello_world.txt
   [INFO] Worker created with 2 MCP servers: [hainet-files, hainet-system]
   ```
5. Verify task progression: Planning → Working → Reporting → UnderReview → Complete

**Success Criteria**: Project completes without stalling

---

**Step 2**: If Test Fails - Apply Fixes

**Fix 1.1**: Add MCP Operation Timeouts
**File**: `hainet-persona/src/agents/worker.rs`
```rust
use tokio::time::timeout;

async fn discover_tools(&self) -> Result<Vec<(String, String)>> {
    match timeout(
        Duration::from_secs(10),
        self.mcp_client.read().await.list_all_tool_summaries()
    ).await {
        Ok(Ok(tools)) => Ok(tools),
        Ok(Err(e)) => Err(anyhow!("MCP tool discovery failed: {}", e)),
        Err(_) => Err(anyhow!("MCP tool discovery timed out after 10s")),
    }
}
```
**LOC**: ~30

---

**Fix 1.2**: Worker Error State Handling
**File**: `hainet-persona/src/agents/worker.rs`
```rust
async fn execute_task_with_discovery(&self, task: Task) -> Result<()> {
    match self.discover_tools().await {
        Ok(tools) if tools.is_empty() => {
            let error_msg = "No MCP tools available";
            error!("{}", error_msg);
            
            self.state.write().await.transition(AgentState::Error, error_msg)?;
            self.project_manager.fail_task(task.id, error_msg).await?;
            
            return Err(anyhow!(error_msg));
        }
        Ok(tools) => {
            info!("Discovered {} tools", tools.len());
        }
        Err(e) => {
            let error_msg = format!("Tool discovery failed: {}", e);
            error!("{}", error_msg);
            
            self.state.write().await.transition(AgentState::Error, &error_msg)?;
            self.project_manager.fail_task(task.id, &error_msg).await?;
            
            return Err(anyhow!(error_msg));
        }
    }
    // ... rest of execution ...
}
```
**LOC**: ~60

---

#### Phase 1 Summary
- **Testing Time**: 30-60 minutes
- **Fix Code** (if needed): ~90 LOC
- **Files Modified** (if needed): 1

---

## Implementation Summary

### Execution Order
1. ✅ Document plan (this file)
2. ✅ **Phase 2**: Specialized model filters (COMPLETED)
3. ⏳ **Phase 3**: User model preferences (Backend COMPLETED, Frontend UI pending)
4. ⏳ **Phase 1**: Test stalling fix + additional fixes if needed (0.5-1.5 hours, ~90 LOC)

### Total Estimates
- **Total LOC**: ~820 LOC
- **Total Time**: 8-12 hours
- **Files Modified**: 8 files
- **Testing**: Unit tests, integration tests, UI testing, end-to-end project test

### Files to be Modified

**Phase 2**:
1. `hainet-persona/src/ai_providers/catalog.rs`
2. `hainet-persona/src/ai_providers/selection.rs`

**Phase 3**:
3. `hainet-portal/src-tauri/src/settings_storage.rs`
4. `hainet-portal/src-tauri/src/settings_handler.rs`
5. `hainet-portal/src-tauri/src/lib.rs`
6. `hainet-portal/src/types.ts`
7. `hainet-portal/src/pages/Settings.tsx`
8. `hainet-persona/src/ai_providers/mod.rs`

**Phase 1** (if needed):
9. `hainet-persona/src/agents/worker.rs`

---

## Progress Update (2025-11-14 12:29 PM)

### ✅ Phase 2 Completed
**Files Modified**:
- `hainet-persona/src/ai_providers/catalog.rs` - Added `CodeAnalysis` capability
- `hainet-persona/src/ai_providers/selection.rs` - Added detection methods and filtering logic

**Changes**:
1. Added `CodeAnalysis` to `ModelCapability` enum
2. Updated `infer_capabilities()` to detect "math", "code", "coder" in model names
3. Added `is_math_model()` and `is_coder_model()` detection methods
4. Added `TaskType` enum (General, FileOperation, CodeGeneration, CodeAnalysis, MathematicalComputation, DataAnalysis)
5. Extended `SelectionContext` with `requires_math`, `requires_coding`, `task_type` fields
6. Added factory methods: `for_worker_coding()`, `for_worker_math()`
7. Updated `select_best()` to filter models based on task requirements
8. **Compilation**: ✅ Successful (only pre-existing warnings)

### ✅ Phase 3 Backend Completed
**Files Modified**:
- `hainet-portal/src-tauri/src/settings_storage.rs` - Database schema and methods
- `hainet-portal/src-tauri/src/settings_handler.rs` - Tauri commands
- `hainet-portal/src-tauri/src/lib.rs` - Command registration
- `hainet-portal/src/types.ts` - TypeScript interfaces

**Changes**:
1. Added `model_preferences` table to SQLite schema
2. Added `ModelPreference` struct (agent_type, preferred_family, allow_fallback)
3. Implemented methods: `save_model_preference()`, `get_model_preference()`, `get_all_model_preferences()`
4. Added Tauri commands: `get_model_preferences`, `save_model_preference`, `get_model_preference`
5. Registered all commands in `lib.rs`
6. Added TypeScript types: `ModelPreference`, `ModelFamily`, `MODEL_FAMILIES` constant
7. **Compilation**: ✅ Successful (only pre-existing warnings)

### ⏳ Phase 3 Frontend Pending
**Remaining Work**:
- Add Model Preferences section to Settings.tsx (~120 LOC)
- Integrate user preferences into AIProviderManager model selection
- Test UI functionality

### ⏳ Phase 1 Not Started
Testing of Session 39 MCP client sharing fix deferred until after Phases 2 & 3 complete.

---

## Testing Checklist

### Phase 2
- [x] Code implemented for specialized model filters
- [x] Backend compiles without errors
- [ ] Unit test: `is_math_model()` detects "math" in model names
- [ ] Unit test: `is_coder_model()` detects "coder"/"code" in model names
- [ ] Integration test: Worker with coding task selects coder model
- [ ] Integration test: Worker with math task selects math model

### Phase 3
- [ ] UI test: Settings page shows Model Preferences section
- [ ] UI test: Dropdowns populated with model families
- [ ] UI test: Saving preference updates database
- [ ] Backend test: Preference filtering works in `select_best()`
- [ ] Integration test: Admin uses preferred Llama 3 when available
- [ ] Integration test: Fallback works when preferred unavailable

### Phase 1
- [ ] Manual test: Portal starts without errors
- [ ] Manual test: MCP servers initialize at startup
- [ ] Manual test: Workers receive MCP client with servers
- [ ] Manual test: Simple project (hello_world.txt) completes
- [ ] Manual test: Complex project (snake game) completes
- [ ] Error handling: Worker fails gracefully on MCP timeout
- [ ] Error handling: Task status updates to Failed on worker error

---

## Notes

- Session builds on Session 39 MCP client sharing fix
- Maintains backward compatibility
- No breaking changes to agent interfaces
- Incremental approach allows testing at each phase
- User preferences provide future extensibility for model selection
- All changes follow HAI-Net constitutional principles (user control, transparency)
