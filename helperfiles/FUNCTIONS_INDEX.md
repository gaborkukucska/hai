<!-- # START OF FILE helperfiles/FUNCTIONS_INDEX.md -->
# Functions Index (v0.02)

This file tracks the core functions/methods defined within the framework, categorized by component. It helps in understanding the codebase and navigating between different parts.

*   **Format:** `[File Path]::[Class Name]::[Method Name](parameters) - Description` or `[File Path]::[Function Name](parameters) - Description`

---

## Prompt Management System (Cycle 0.2 - Complete)

### hainet-persona/src/prompts/types.rs
- `AgentId::new(agent_type, name)` - Create new agent identifier
- `PromptContext::default()` - Create default prompt context with empty values
- `PromptTemplate::validate()` - Validate template structure and content

### hainet-persona/src/prompts/loader.rs
- `PromptLoader::new(base_path)` - Create new prompt loader
- `PromptLoader::load_template(agent_type, state)` - Load template with three-tier resolution
- `PromptLoader::reload_all()` - Hot-reload all templates from disk
- `PromptLoader::validate_all()` - Run validation on all templates

### hainet-persona/src/prompts/renderer.rs
- `PromptRenderer::new()` - Create new Handlebars-based renderer
- `PromptRenderer::render(template, context)` - Render prompt with context injection
- `PromptRenderer::validate_constitutional_compliance(text)` - Check for constitutional keywords

### hainet-persona/src/prompts/cache.rs
- `PromptCache::new(max_entries, ttl)` - Create LRU+TTL cache
- `PromptCache::get(key)` - Retrieve cached prompt if valid
- `PromptCache::put(key, value)` - Store prompt with TTL
- `PromptCache::invalidate_by_agent(agent_id)` - Clear cache for specific agent
- `PromptCache::clear()` - Remove all cached entries

### hainet-persona/src/prompts/mod.rs
- `PromptManager::new(base_path)` - Create unified prompt manager
- `PromptManager::get_prompt(agent_id, state, context)` - Get/render prompt with auto-caching
- `PromptManager::reload_all()` - Reload all templates
- `PromptManager::validate_all()` - Validate all templates

---

## Messaging & Communication System (Cycle 0.3 - Complete)

### hainet-persona/src/messaging/types.rs
- `Message::new(from, to, content, metadata)` - Create new message
- `MessageMetadata::with_priority(priority)` - Create metadata with priority level
- `AgentId::new(agent_type, name)` - Create agent identifier

### hainet-persona/src/messaging/channels.rs
- `MessageBus::new(buffer_size)` - Create message bus with bounded channels
- `MessageBus::register_agent(agent_id)` - Register agent and create channel
- `MessageBus::send_message(message)` - Send message with hierarchy validation
- `MessageBus::receive_message(agent_id)` - Receive message for agent
- `MessageBus::validate_route(from, to)` - Check if route is allowed by hierarchy

### hainet-persona/src/messaging/priority.rs
- `PriorityRouter::new()` - Create 5-level priority queue system
- `PriorityRouter::enqueue(message)` - Add message to priority queue
- `PriorityRouter::dequeue_batch(count)` - Get messages with fair scheduling
- `PriorityRouter::get_stats()` - Get queue statistics

### hainet-persona/src/messaging/guardian.rs
- `GuardianInterceptor::new()` - Create constitutional monitoring interceptor
- `GuardianInterceptor::intercept(message)` - Check message for violations
- `GuardianInterceptor::update_config(config)` - Update detection thresholds

### hainet-persona/src/messaging/audit.rs
- `AuditLogger::new(db_path)` - Create audit trail logger
- `AuditLogger::log_message(message, scores)` - Log message with compliance scores
- `AuditLogger::verify_chain_integrity()` - Validate SHA256 hash chain
- `AuditLogger::query_by_agent(agent_id)` - Query audit entries for agent

### hainet-persona/src/messaging/deadlock.rs
- `DeadlockDetector::new()` - Create deadlock prevention system
- `DeadlockDetector::register_request(request_id, from, to)` - Track request
- `DeadlockDetector::detect_cycles()` - Check for circular dependencies
- `DeadlockDetector::cleanup_stale_requests()` - Remove expired requests (30s timeout)

---

## AI Provider Discovery & Selection (Cycle 0.4 - Complete)

### hainet-persona/src/ai_providers/discovery.rs
- `ProviderDiscovery::new()` - Create network scanner for AI providers
- `ProviderDiscovery::scan_all()` - Scan localhost and LAN for providers
- `ProviderDiscovery::fetch_models(provider)` - Enumerate models from provider
- `ProviderDiscovery::probe_ollama(endpoint)` - Check Ollama availability
- `ProviderDiscovery::probe_vllm(endpoint)` - Check vLLM availability

### hainet-persona/src/ai_providers/catalog.rs
- `ModelCatalog::new()` - Create model database
- `ModelCatalog::add_model(model)` - Add discovered model with capabilities
- `ModelCatalog::infer_capabilities(name)` - Detect model capabilities from name
- `ModelCatalog::models_for_agent(agent_type)` - Filter models by agent requirements
- `ModelCatalog::get_stats()` - Get catalog statistics

### hainet-persona/src/ai_providers/ranking.rs
- `ModelRanker::new()` - Create capability-based ranking system
- `ModelRanker::rank_models(catalog, context)` - Score models for selection
- `ModelRanker::calculate_score(model, criteria)` - Multi-criteria scoring (5 factors)
- `RankingCriteria::for_constitutional_compliance()` - Preset for Guardian agent
- `RankingCriteria::for_high_throughput()` - Preset for performance

### hainet-persona/src/ai_providers/selection.rs
- `ModelSelector::new(catalog)` - Create model selection system
- `ModelSelector::select_best(ranked, context)` - Choose optimal model
- `SelectionContext::for_guardian()` - Context for Guardian agent selection
- `SelectionContext::for_admin()` - Context for Admin agent selection
- `SelectedModel::inference_url()` - Get provider-specific inference endpoint

### hainet-persona/src/ai_providers/providers/ollama.rs
- `OllamaClient::new(endpoint)` - Create Ollama API client
- `OllamaClient::generate(model, prompt, options)` - Run inference
- `OllamaClient::list_models()` - Enumerate available models
- `OllamaClient::health_check()` - Check server availability

### hainet-persona/src/ai_providers/mod.rs
- `AIProviderManager::new()` - Create central provider manager
- `AIProviderManager::discover_providers()` - Scan and catalog all providers
- `AIProviderManager::select_model_for_agent(context)` - Get optimal model
- `AIProviderManager::refresh_catalog()` - Re-scan providers

---

## Constitutional Guardian System (Cycle 0.4 - Complete)

### hainet-persona/src/guardian/pii_detector.rs
- `PIIDetector::new(llm_client)` - Create PII detection system
- `PIIDetector::detect(text)` - Detect personally identifiable information
- `PIIDetector::validate_credit_card(number)` - Luhn algorithm validation
- `PIIDetector::classify_risk(patterns)` - Calculate risk level

### hainet-persona/src/guardian/bias_detector.rs
- `BiasDetector::new(llm_client)` - Create bias detection system
- `BiasDetector::detect(text)` - Detect stereotypes and bias
- `BiasDetector::calculate_fairness_metrics(text)` - Compute bias scores
- `BiasDetector::classify_severity(count)` - Determine severity level

### hainet-persona/src/guardian/harm_analyzer.rs
- `HarmAnalyzer::new(llm_client)` - Create harm analysis system
- `HarmAnalyzer::analyze(text, context)` - Detect harmful content
- `HarmAnalyzer::rule_based_detection(text)` - Keyword-based harm detection
- `HarmAnalyzer::classify_intent(toxicity, types)` - Intent classification
- `HarmAnalyzer::classify_risk(toxicity, types)` - Risk level assessment

### hainet-persona/src/guardian/decision_engine.rs
- `DecisionEngine::new()` - Create Block/Pause/Allow decision engine
- `DecisionEngine::make_decision(pii, bias, harm)` - Calculate guardian action
- `DecisionEngine::calculate_pii_score(report)` - Score PII compliance (0.0-1.0)
- `DecisionEngine::calculate_bias_score(report)` - Score bias compliance (0.0-1.0)
- `DecisionEngine::calculate_harm_score(report)` - Score harm compliance (0.0-1.0)
- `DecisionEngine::collect_violations(reports)` - Aggregate all violations

### hainet-persona/src/guardian/ollama_client.rs
- `GuardianOllamaClient::new(endpoint, model)` - Create Guardian-specific client
- `GuardianOllamaClient::analyze_pii(text)` - ML-based PII detection
- `GuardianOllamaClient::analyze_bias(text)` - ML-based bias detection
- `GuardianOllamaClient::analyze_harm(text)` - ML-based harm detection
- `GuardianOllamaClient::parse_json_response(response)` - Extract JSON from markdown

---

## HAI-Net Seed Installer (Cycle 0.5 Phase B - Complete)

### hainet-seed/src/installer/platform.rs
- `Platform::detect()` - Detect current operating system and architecture
- `Platform::is_termux()` - Check if running in Termux environment
- `Platform::is_supported()` - Verify platform is supported
- `Platform::ollama_install_script()` - Get platform-specific Ollama install URL
- `Architecture::detect()` - Detect CPU architecture (x86_64, aarch64)
- `SystemTier::detect()` - Classify system tier based on RAM (Tier 1-4)
- `SystemTier::get_total_ram_gb()` - Get total system RAM in GB
- `SystemTier::recommended_model()` - Get recommended model for tier

### hainet-seed/src/installer/ollama.rs
- `OllamaInstaller::new(platform)` - Create Ollama installer for platform
- `OllamaInstaller::is_installed()` - Check if Ollama binary exists
- `OllamaInstaller::is_running()` - Health check via API endpoint
- `OllamaInstaller::install()` - Platform-specific Ollama installation
- `OllamaInstaller::install_linux()` - Install Ollama on Linux via script
- `OllamaInstaller::install_macos()` - Install Ollama on macOS via Homebrew
- `OllamaInstaller::install_termux()` - Termux installation (manual required)
- `OllamaInstaller::start_service()` - Start Ollama service in background
- `OllamaInstaller::has_model(name)` - Check if model is available
- `OllamaInstaller::pull_model(name)` - Download model with progress bar
- `OllamaInstaller::list_models()` - Enumerate available models
- `OllamaInstaller::version()` - Get Ollama version

### hainet-seed/src/installer/dependencies.rs
- `DependencyChecker::new(platform)` - Create dependency checker
- `DependencyChecker::check_all()` - Scan for required dependencies
- `DependencyChecker::has_command(cmd)` - Check if command exists
- `DependencyChecker::install_missing(deps)` - Install missing dependencies
- `DependencyChecker::install_linux_deps(deps)` - Install via apt/dnf/pacman
- `DependencyChecker::install_macos_deps(deps)` - Install via Homebrew
- `DependencyChecker::install_termux_deps(deps)` - Install via pkg

### hainet-seed/src/installer/mod.rs
- `Installer::new()` - Create installer with platform detection
- `Installer::install()` - Run complete installation workflow
- `Installer::install_ollama()` - Check/install/start Ollama
- `Installer::download_default_model()` - Download tier-appropriate model
- `Installer::platform()` - Get platform information
- `Installer::tier()` - Get system tier

### hainet-seed/src/lib.rs
- `init()` - Initialize HAI-Net Seed system
- `SeedService::new()` - Create seed service with installer
- `SeedService::install()` - Run installation workflow
- `SeedService::check_requirements()` - Display system information

---

---

## MCP (Model Context Protocol) System (Cycle 0.6 - Migration In Progress)

**Status:** 🚧 90% Complete - Migrating to official `rmcp` SDK (v0.8.2)

### hainet-persona/src/tools/mcp/types.rs (Stub)
- `MCPRequest::new(id, method, params)` - Create JSON-RPC 2.0 request (stub)
- `MCPError::new(code, message)` - Create MCP error (stub)
- `MCPError::with_data(code, message, data)` - Create error with additional data (stub)

### hainet-persona/src/tools/mcp/client.rs (Stub)
- `MCPClient::new()` - Create MCP client manager (stub implementation)
- `MCPClient::start_server(name, path)` - Spawn MCP server process via stdio (stub)
- `MCPClient::discover_tools(server)` - Tool discovery via initialize method (stub)
- `MCPClient::call_tool(server, tool, params)` - Execute tool with retries (stub)
- `MCPClient::list_tools(server)` - Get available tools from server (stub)
- `MCPClient::shutdown()` - Clean shutdown of all servers (stub)

### mcp-servers/hainet-files/src/main.rs (90% Complete - Has Compilation Errors)

**Using Official rmcp SDK:**
- Based on `rmcp::handler::server::ServerHandler` trait
- Uses `rmcp::model::*` for MCP protocol types
- Integrated with hainet-core CAS storage (BLAKE3)

**Implementation Status:**
- ✅ `FilesServer::new(storage_path)` - Create files MCP server with CAS storage
- ✅ `FilesServer::handle_file_read(path)` - Read file with CAS integration
- ✅ `FilesServer::handle_file_write(path, content)` - Write file with CAS storage
- ✅ `FilesServer::handle_file_list(path)` - List directory contents
- ✅ `FilesServer::handle_file_metadata(path)` - Get file metadata
- ⚠️ Trait implementation (ServerHandler) - Has type mismatches
- ⚠️ Error handling (RmcpError) - API differs from assumptions
- ⚠️ Server initialization (serve_stdio) - Method not directly available

**MCP Tools (Defined, Not Yet Functional):**
- `hainet_file_read` - Read file from local filesystem
- `hainet_file_write` - Write content to file
- `hainet_file_list` - List files in directory
- `hainet_file_metadata` - Get file metadata (size, type, permissions)

**Next Steps:**
1. Study rmcp SDK examples for correct API patterns
2. Fix RmcpError construction (no helper methods in v0.8.2)
3. Fix serve_stdio initialization pattern
4. Resolve type mismatches (Arc<Map> vs Map, lifetimes)
5. Complete MCP client implementation

**Resources:**
- Migration Plan: `MCP_ANALYSIS_AND_MIGRATION_PLAN.md`
- Official SDK: https://github.com/modelcontextprotocol/rust-sdk
- rmcp Docs: https://docs.rs/rmcp/latest/rmcp/

---

## Agent System (Phase 1 Cycle 1.1 - Foundation Complete)

### hainet-persona/src/agents/intent.rs
- `IntentParser::new()` - Create intent parser with default threshold (0.6)
- `IntentParser::with_threshold(threshold)` - Create with custom confidence threshold
- `IntentParser::parse(user_input)` - Parse user input to extract intent
- `IntentParser::normalize_text(text)` - Normalize text (lowercase, trim)
- `IntentParser::classify_intent(text)` - Classify intent type (Question, Task, Command, etc.)
- `IntentParser::extract_entities(text, intent_type)` - Extract entities (emails, dates, paths)
- `IntentParser::extract_email(text)` - Extract email addresses from text
- `IntentParser::extract_file_path(text)` - Extract file paths from text
- `IntentParser::suggest_domain_and_action(text, intent_type)` - Suggest PM domain and action
- `IntentParser::calculate_confidence(text, intent_type)` - Calculate confidence score

### hainet-persona/src/agents/planner.rs
- `TaskPlanner::new()` - Create new task planner
- `TaskPlanner::create_plan(intent)` - Create task plan from user intent
- `TaskPlanner::decompose_intent(intent)` - Decompose intent into executable steps
- `TaskPlanner::generate_id()` - Generate unique step ID
- `TaskPlanner::complete_step(plan, step_id, result)` - Mark step as complete
- `TaskPlanner::get_next_step(plan)` - Get next step to execute
- `TaskPlanner::dependencies_met(plan, step)` - Check if dependencies are met

### hainet-persona/src/agents/state.rs
- `AgentStateMachine::new()` - Create state machine in Startup state
- `AgentStateMachine::with_max_duration(duration)` - Create with custom timeout
- `AgentStateMachine::current_state()` - Get current state
- `AgentStateMachine::transition(new_state, reason)` - Transition to new state
- `AgentStateMachine::is_valid_transition(new_state)` - Check if transition is allowed
- `AgentStateMachine::is_stuck()` - Check if stuck in state too long
- `AgentStateMachine::force_error(reason)` - Force emergency error transition
- `AgentStateMachine::time_in_state()` - Get time spent in current state
- `AgentStateMachine::get_history()` - Get recent transition history
- `AgentStateMachine::is_ready()` - Check if agent ready (in Idle state)
- `AgentStateMachine::is_working()` - Check if agent working (Planning/Working state)
- `AgentStateMachine::is_error()` - Check if agent in error state

### hainet-persona/src/agents/admin.rs
- `AdminAgent::new(context)` - Create new Admin AI agent
- `AdminAgent::process_user_input(user_input)` - Process user input (stub)
- `Agent::id()` - Get agent identifier
- `Agent::process_message(message)` - Process incoming message
- `Agent::start()` - Start agent main loop
- `Agent::stop()` - Stop agent gracefully

### hainet-persona/src/agents/mod.rs
- `AgentContext::new(message_bus, prompt_manager, mcp_client, guardian)` - Create shared context

---

## Project Management System (Phase 1 Cycle 1.2 - Complete)

### hainet-persona/src/projects/project.rs
- `ProjectId::new()` - Create new random project ID
- `ProjectId::from_uuid(uuid)` - Create from existing UUID
- `ProjectId::from_string(s)` - Parse from string
- `Project::new(title, overview)` - Create new project
- `Project::assign_pm(pm_id)` - Assign PM agent to project
- `Project::add_worker(worker_id)` - Add worker agent to project
- `Project::remove_worker(worker_id)` - Remove worker agent
- `Project::add_milestone(milestone_id)` - Add milestone to project
- `Project::add_task(task_id)` - Add task to project
- `Project::pause()` - Pause active project
- `Project::resume()` - Resume paused project
- `Project::complete()` - Mark project as completed
- `Project::fail(reason)` - Mark project as failed
- `Project::cancel()` - Cancel project
- `Project::soft_delete()` - Soft delete project
- `Project::is_deleted()` - Check if project deleted
- `Project::progress(completed_tasks)` - Calculate progress percentage
- `ProjectStatus::is_terminal()` - Check if in terminal state
- `ProjectStatus::is_active()` - Check if project active

### hainet-persona/src/projects/task.rs
- `TaskId::new()` - Create new random task ID
- `TaskId::from_uuid(uuid)` - Create from existing UUID
- `TaskId::from_string(s)` - Parse from string
- `Task::new(project_id, title, description)` - Create new task
- `Task::add_dependency(task_id)` - Add dependency to task
- `Task::dependencies_met(completed_task_ids)` - Check if dependencies met
- `Task::assign_to(worker_id)` - Assign task to worker agent
- `Task::start()` - Worker starts working on task
- `Task::block(reason)` - Block task with reason
- `Task::unblock()` - Unblock blocked task
- `Task::submit_for_review(deliverables)` - Submit for PM review
- `Task::approve(notes)` - PM approves task
- `Task::reject(reason)` - PM rejects task
- `Task::fail(reason)` - Mark task as failed
- `Task::duration()` - Calculate task duration
- `TaskStatus::is_terminal()` - Check if in terminal state
- `TaskStatus::is_active()` - Check if task active

### hainet-persona/src/projects/milestone.rs
- `MilestoneId::new()` - Create new random milestone ID
- `MilestoneId::from_uuid(uuid)` - Create from existing UUID
- `MilestoneId::from_string(s)` - Parse from string
- `Milestone::new(project_id, title, description, deadline)` - Create new milestone
- `Milestone::add_task(task_id)` - Add task to milestone
- `Milestone::remove_task(task_id)` - Remove task from milestone
- `Milestone::progress(tasks)` - Calculate progress percentage
- `Milestone::is_complete(tasks)` - Check if all tasks complete
- `Milestone::has_active_tasks(tasks)` - Check if any tasks active
- `Milestone::update_status(tasks)` - Update status based on tasks
- `Milestone::is_delayed()` - Check if past deadline
- `Milestone::time_until_deadline()` - Get time remaining until deadline
- `Milestone::task_stats(tasks)` - Get task count statistics

### hainet-persona/src/projects/storage.rs
- `ProjectStorage::new(db_path)` - Create SQLite storage backend
- `ProjectStorage::create_tables()` - Create database schema
- `ProjectStorage::create_project(project)` - Create project in database
- `ProjectStorage::get_project(id)` - Get project by ID
- `ProjectStorage::update_project(project)` - Update existing project
- `ProjectStorage::delete_project(id)` - Soft delete project
- `ProjectStorage::list_active_projects()` - List all active projects
- `ProjectStorage::create_task(task)` - Create task in database
- `ProjectStorage::get_task(id)` - Get task by ID
- `ProjectStorage::update_task(task)` - Update existing task
- `ProjectStorage::list_project_tasks(project_id)` - List tasks for project
- `ProjectStorage::create_milestone(milestone)` - Create milestone in database
- `ProjectStorage::get_milestone(id)` - Get milestone by ID
- `ProjectStorage::update_milestone(milestone)` - Update existing milestone
- `ProjectStorage::list_project_milestones(project_id)` - List milestones for project

### hainet-persona/src/projects/manager.rs
- `ProjectManager::new(db_path)` - Create project manager with SQLite backend
- `ProjectManager::create_project(title, overview, initial_tasks)` - Create new project
- `ProjectManager::assign_pm(project_id, pm_id)` - Assign PM to project
- `ProjectManager::complete_project(project_id)` - Complete project (trigger hibernation)
- `ProjectManager::delete_project(project_id)` - Delete project (soft delete)
- `ProjectManager::hibernate_agent(agent_id, project_id, agent_type, system_prompt)` - Hibernate agent
- `ProjectManager::wake_agent(agent_id)` - Wake hibernated agent
- `ProjectManager::cleanup_hibernated_agents(project_id)` - Cleanup agents for project
- `ProjectManager::get_project_hibernated_agents(project_id)` - Get hibernated agents
- `ProjectManager::create_task(project_id, title, description)` - Create task
- `ProjectManager::assign_task(task_id, worker_id)` - Assign task to worker
- `ProjectManager::complete_task(task_id, deliverables)` - Complete task
- `ProjectManager::approve_task(task_id, notes)` - PM approves task
- `ProjectManager::reject_task(task_id, reason)` - PM rejects task
- `ProjectManager::create_milestone(project_id, title, description, deadline)` - Create milestone
- `ProjectManager::update_milestone_status(milestone_id)` - Update milestone status
- `ProjectManager::get_project(id)` - Get project by ID
- `ProjectManager::list_active_projects()` - List all active projects
- `ProjectManager::get_project_tasks(project_id)` - Get tasks for project
- `ProjectManager::get_project_milestones(project_id)` - Get milestones for project
- `ProjectManager::get_project_progress(project_id)` - Get project progress summary

---

## Multimodal System (Phase 2 Cycle 2.2 Phase A - Complete)

### hainet-core/src/multimodal/mod.rs
- `MultimodalConfig::default()` - Create default multimodal configuration
- `DeviceRole` enum variants - Master, Slave, Standalone deployment modes

### hainet-core/src/multimodal/audio.rs
- `AudioFormat::detect(data)` - Auto-detect audio format from magic numbers (WebM/Opus, WAV, MP3)
- `AudioFormat::name()` - Get human-readable format name
- `AudioProcessor::new()` - Create audio processor with default settings (16kHz mono)
- `AudioProcessor::with_settings(sample_rate, channels)` - Create with custom settings
- `AudioProcessor::decode_base64(base64_data)` - Decode Base64 audio data for IPC
- `AudioProcessor::process(audio_data)` - Process audio: detect format, convert, resample
- `AudioProcessor::process_wav(wav_data)` - Process WAV audio (verify format, resample if needed)
- `AudioProcessor::decode_with_symphonia(audio_data, format)` - Decode WebM/Opus or MP3 files to WAV
- `AudioProcessor::resample_and_convert_channels(samples, source_rate, source_channels)` - Convert to mono and resample
- `AudioProcessor::resample_linear(samples, source_rate)` - Linear interpolation resampling

### hainet-core/src/multimodal/stt.rs
- `WhisperConfig::default()` - Create default Whisper configuration
- `SpeechToText::new()` - Create STT engine with default configuration
- `SpeechToText::with_config(config)` - Create with custom configuration
- `SpeechToText::transcribe(audio_wav)` - Transcribe audio data (placeholder, ready for Whisper)
- `SpeechToText::transcribe_auto_detect(audio_wav)` - Transcribe with language detection
- `SpeechToText::config()` - Get current configuration
- `SpeechToText::set_config(config)` - Update configuration
- `TranscriptionResult` - Structured transcription output (text, confidence, language, timing)
- `TranscriptionSegment` - Timestamped transcription segment

### hainet-core/src/multimodal/vision.rs
- `VisionSystem::new(config)` - Create new vision system
- `VisionSystem::list_devices()` - List available webcam devices
- `VisionSystem::start_capture()` - Start webcam capture session
- `VisionSystem::stop_capture()` - Stop webcam capture session
- `VisionSystem::capture_frame()` - Capture a single frame
- `VisionSystem::analyze_frame_mock()` - Analyze frame with mock model

### hainet-portal/src-tauri/src/vision_handler.rs
- `list_webcam_devices()` - Tauri command to list webcam devices
- `start_webcam(config)` - Tauri command to start webcam
- `stop_webcam()` - Tauri command to stop webcam
- `capture_frame()` - Tauri command to capture a frame
- `set_privacy_mode(mode)` - Tauri command to set privacy mode

---

## Dynamic UI System (Cycle 2.5 - Complete)

### hainet-portal/src/components/DynamicUIRenderer.tsx
- `DynamicUIRenderer(schema, onAction)` - Renders UI from a JSON schema and handles user actions.

### hainet-portal/src/components/componentLibrary.ts
- `componentLibrary` - A mapping of component names to React components (e.g., `Stack`, `Text`, `Button`).

### hainet-portal/src/types.ts
- `DynamicUIComponent` - Interface for the UI component schema.
- `DynamicUIAction` - Interface for actions triggered by the UI.

---

## Video Streaming (Cycle 2.6 - Complete)

### hainet-portal/src-tauri/src/video_handler.rs
- `stream_video(path)` - Starts a local HTTP server to stream video content and returns the URL.

### hainet-portal/src/components/VideoPlayer.tsx
- `VideoPlayer(src, isVisible, onClose)` - A fullscreen video player overlay with controls.

---

## Settings System (Cycle 2.7 - Complete)

### hainet-portal/src-tauri/src/settings_handler.rs
- `get_settings()` - Tauri command to get the current settings.
- `update_settings(settings)` - Tauri command to update the settings.
- `get_system_status()` - Tauri command to get the system status.

### hainet-portal/src/components/Settings.tsx
- `Settings()` - Main component for the settings panel.

### hainet-portal/src/components/SystemStatus.tsx
- `SystemStatus()` - Component to display system status.

---

## Blockchain & Governance System (Phase 3 - In Progress)

### hainet-chain/src/consensus/mod.rs
- `rpc_client` - Module for the Tendermint RPC client.
- `validator` - Module for block and transaction validation.

### hainet-chain/src/consensus/rpc_client.rs
- `RpcClient::new(rpc_url)` - Creates a new `RpcClient` connected to the specified `rpc_url`.
- `RpcClient::broadcast_tx(tx)` - Broadcasts a transaction to the Tendermint network.
- `RpcClient::status()` - Checks the status of the connected Tendermint node.

### hainet-chain/src/consensus/validator.rs
- `BlockValidator::new(db)` - Create a new BlockValidator
- `BlockValidator::validate_block(block)` - Validate a block
- `BlockValidator::validate_transaction(transaction)` - Validate a single transaction

### hainet-chain/src/state/mod.rs
- `StateMachine::new(db_path)` - Create a new StateMachine
- `StateMachine::apply_block(transactions)` - Apply a block of transactions to the state.
- `StateMachine::get(key)` - Get a value from the state
- `StateMachine::set(key, value)` - Set a value in the state
- `StateMachine::tally_votes(proposal_id)` - Tally votes for a proposal

### hainet-chain/src/transactions/mod.rs
- `Transaction::new(payload, keypair)` - Create and sign a new transaction
- `Transaction::verify()` - Verify the transaction's signature and integrity

### hainet-chain/src/governance/mod.rs
- `Governance::new(rpc_client)` - Create a new Governance service.
- `Governance::submit_proposal(transaction)` - Submits a new proposal by broadcasting a transaction.
- `Governance::cast_vote(transaction)` - Casts a vote by broadcasting a transaction.
- `create_proposal(keypair, title, description, proposal_type, voting_duration_secs, payload)` - Create and sign a new proposal
- `create_vote(keypair, proposal_id, decision)` - Create and sign a new vote
- `tally_votes(db, proposal_id)` - Tally the votes for a given proposal

---

## Statistics

**Total Modules:** 56
**Total Functions:** 174+ (including multimodal system)  
**Lines of Code:** ~17,780 (Phase 0: ~10,570, Phase 1: ~3,241, Phase 2: ~3,969)  
**Test Coverage:** 167 tests (154 previous, 13 new multimodal tests)  
**Constitutional Compliance:** Fully integrated (Articles I, II, III, V, VII)

**Phase 0 Status:** ✅ COMPLETE (Cycles 0.1-0.6)
**Phase 1 Status:** 🚧 Foundation + Project System Complete (Cycles 1.1-1.2)

**Phase 1 Cycle 1.1 Achievements:**
- ✅ Intent parsing system (rule-based, ready for LLM)
- ✅ Task planning with dependency tracking
- ✅ Agent state machine with validation
- ✅ Admin AI stub with core structure
- ✅ Agent trait and shared context

**Phase 1 Cycle 1.2 Achievements:**
- ✅ Complete project lifecycle management (Created → Active → Completed/Failed/Cancelled)
- ✅ Task management with worker assignment and PM validation workflow
- ✅ Milestone tracking with progress monitoring and deadline tracking
- ✅ Agent hibernation system (PM and Worker agents)
- ✅ SQLite persistence layer with full CRUD operations
- ✅ Type-safe IDs for all entities (ProjectId, TaskId, MilestoneId)
- ✅ 6 new modules with 40+ functions
- ✅ Compilation successful (1 harmless warning)

**Phase 2 Cycle 2.2 Phase A Achievements:**
- ✅ Core STT infrastructure in hainet-core/src/multimodal/
- ✅ Audio format detection (WebM/Opus, WAV, MP3 via magic numbers)
- ✅ Audio preprocessing (Base64 decode, resample to 16kHz mono, channel conversion)
- ✅ STT placeholder with Whisper-ready architecture
- ✅ Multi-device deployment support (Master/Slave/Standalone)
- ✅ Offline-first design (models in ~/.hainet/models/)
- ✅ 3 new modules with 9+ functions
- ✅ 13 unit tests (all passing)
- ✅ Clean compilation (0.32s, 1 harmless warning)

**Phase 2 Cycle 2.2 Phase B Achievements:**
- ✅ Comprehensive STT integration tests in hainet-portal/src-tauri/tests/
- ✅ Test coverage: Audio processing (8 tests), STT engine (4 tests), Integration (7 tests), Performance (2 tests)
- ✅ Portal STT handler tests (config, VAD, data serialization)
- ✅ Full pipeline validation (Base64 → AudioProcessor → STT → Transcription)
- ✅ Performance benchmarks (audio processing <10ms, base64 <0.5ms per iteration)
- ✅ Test suite: 19 passed, 0 failed, 1 ignored (Whisper-dependent test)
- ✅ Made stt_handler module public for testing
- ✅ Fixed base64 deprecation warnings (migrated to Engine API)
- ✅ All tests pass cleanly without warnings

---

<!-- # END OF FILE helperfiles/FUNCTIONS_INDEX.md -->
