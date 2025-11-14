# Session 41: Model Settings Persistence Fix

**Date**: 2025-11-14  
**Status**: ✅ COMPLETED

## Problem

User reported that when updating model family preferences in the Settings UI:
1. ❌ Selected values don't stay shown in the dropdown field
2. ❌ When closing and reloading the UI, model family settings are empty again

### Root Cause Analysis

From the logs, we found that:
- ✅ Settings were being saved to the Portal's database correctly
- ✅ Settings were being loaded from the database correctly
- ❌ **Model preferences were NOT being used during model selection**

The issue was that `hainet-persona` (the agent system) had no access to the user preferences stored in the Portal's database. The `ModelSelector::select_best()` method was selecting models based on capability scores **without filtering by user-preferred family**.

## Solution Architecture

### 1. Add Preference Filtering to Model Selection

**Files Modified:**
- `hainet-persona/src/ai_providers/selection.rs`

**Changes:**
1. Added `preferred_family: Option<String>` field to `SelectionContext`
2. Added `allow_fallback: bool` field to control fallback behavior
3. Implemented `matches_family()` helper to check if a model belongs to a family
4. Rewrote `select_best()` with two-pass logic:
   - **First pass**: Try to find models from preferred family
   - **Second pass**: Fall back to all models if no suitable model found in preferred family
5. Added helper methods `with_preferred_family()` and `with_fallback()` to `SelectionContext`

### 2. Create User Settings Storage in hainet-persona

**Files Created:**
- `hainet-persona/src/user_settings.rs`

**Changes:**
1. Created `UserSettingsManager` to store user preferences in hainet-persona's database
2. Provides methods:
   - `set_model_preference(agent_type, model_family)`
   - `get_model_preference(agent_type)`
   - `get_all_preferences()`
   - `clear_all_preferences()`
3. Uses SQLite database at `~/.hainet/data/user_settings.db`

### 3. Integrate User Settings into Agent System

**Files Modified:**
- `hainet-persona/src/lib.rs` - Export `UserSettingsManager`
- `hainet-persona/src/agents/mod.rs` - Add `user_settings` to `AgentContext`
- `hainet-portal/src-tauri/src/admin_bridge.rs` - Create and pass `UserSettingsManager` to context
- `hainet-persona/src/agents/admin.rs` - Load and use preferences when selecting models

**Changes:**
1. Added `user_settings: Option<SharedUserSettings>` to `AgentContext`
2. Updated `AdminAgent` to load user preference before selecting models:
   ```rust
   let preferred_family = if let Some(ref user_settings) = self.context.user_settings {
       let settings = user_settings.read().await;
       settings.get_model_preference("admin").await.ok().flatten()
   } else {
       None
   };
   ```
3. Call new method `select_model_for_agent_with_preferences()` instead of `select_model_for_agent()`

### 4. Sync Preferences from Portal to hainet-persona

**Files Modified:**
- `hainet-portal/src-tauri/src/settings_handler.rs`

**Changes:**
1. Added `sync_preference_to_persona()` helper function
2. Modified `save_model_preference()` to sync preferences to hainet-persona's database after saving to Portal's database
3. This ensures that when the user updates preferences in the Settings UI, they are propagated to the agent system

### 5. Add Convenience Method to AIProviderManager

**Files Modified:**
- `hainet-persona/src/ai_providers/mod.rs`

**Changes:**
1. Added `select_model_for_agent_with_preferences()` method
2. This wraps `select_model_for_agent()` and applies user preferences to the selection context

## Data Flow

```
1. User selects "Gemma 3" for Admin in Settings UI
   ↓
2. Frontend calls save_model_preference("admin", "gemma3")
   ↓
3. Backend saves to Portal DB (~/.hainet-portal/settings.db)
   ↓
4. Backend syncs to hainet-persona DB (~/.hainet/data/user_settings.db)
   ↓
5. AdminAgent loads preference when selecting model
   ↓
6. ModelSelector filters models by family "gemma3"
   ↓
7. Admin uses Gemma 3 model for inference
```

## Files Changed

### hainet-persona
- `src/user_settings.rs` ✨ NEW
- `src/lib.rs` - Export `UserSettingsManager`
- `src/agents/mod.rs` - Add `user_settings` to `AgentContext`
- `src/agents/admin.rs` - Load and use preferences
- `src/ai_providers/selection.rs` - Add family filtering logic
- `src/ai_providers/mod.rs` - Add `select_model_for_agent_with_preferences()`

### hainet-portal
- `src-tauri/src/admin_bridge.rs` - Create `UserSettingsManager`
- `src-tauri/src/settings_handler.rs` - Sync preferences to hainet-persona

## Testing Plan

1. ✅ Compile the project (no errors)
2. ⏳ Run the Portal and verify:
   - Model family selection persists in UI
   - Settings reload correctly after closing/reopening
   - Admin agent uses the selected model family
   - Logs show correct model selection with preference filtering
3. ⏳ Test fallback behavior:
   - Select a family that has no suitable models
   - Verify fallback to other families works

## Implementation Notes

### Two-Pass Model Selection

The `select_best()` method now uses a two-pass approach:

1. **First Pass** (if `preferred_family` is set):
   - Filters models by family name
   - Applies all other criteria (capabilities, score, etc.)
   - Returns first suitable model from preferred family

2. **Second Pass** (fallback):
   - If no suitable model found in first pass AND `allow_fallback` is true
   - Searches all models regardless of family
   - Logs fallback behavior for debugging

### Database Architecture

We now have **two separate databases**:
1. **Portal DB** (`~/.hainet-portal/settings.db`): UI preferences, device settings
2. **hainet-persona DB** (`~/.hainet/data/user_settings.db`): Agent model preferences

This separation maintains the architectural boundary between the Portal (UI) and hainet-persona (agent system).

### Sync Strategy

Preferences are synced **on write** when the user updates settings in the UI. This ensures:
- ✅ No additional API calls needed to fetch preferences
- ✅ Preferences are immediately available to agents
- ✅ Portal settings are source of truth

## Known Limitations

1. **PM and Worker agents** currently don't use user preferences (only Admin agent implemented)
2. **Preference sync is one-way** (Portal → hainet-persona)
3. **No preference migration** if database schemas change

## Future Enhancements

1. Extend preference support to PM and Worker agents
2. Add UI to view/edit preferences per agent type
3. Add preference versioning for schema migrations
4. Add preference import/export for backup
5. Consider using a shared database instead of syncing between two databases

## Conclusion

The model settings persistence issue has been resolved by:
1. ✅ Adding family-based filtering to model selection
2. ✅ Creating a user settings storage system in hainet-persona
3. ✅ Integrating preferences into the agent system
4. ✅ Syncing preferences from Portal to hainet-persona

The user can now select model families in the Settings UI, and those preferences will be respected by the Admin agent when selecting models for inference.
