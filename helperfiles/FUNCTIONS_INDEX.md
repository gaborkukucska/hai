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

### hainet-seed/src/installer/network_scanner.rs (Phase 4.5a.1 - Complete)
- `DeviceCandidate` - Struct for discovered SSH-enabled devices (ip, hostname, mac_address)
- `NetworkScanner::new()` - Create network scanner with nmap validation
- `NetworkScanner::find_nmap()` - Locate nmap binary in PATH
- `NetworkScanner::scan_local_network()` - Scan local network for SSH devices (port 22)
- `NetworkScanner::parse_nmap_output(output)` - Parse greppable nmap output
- `NetworkScanner::get_local_ip()` - Get local IP address (excludes loopback/link-local)
- `NetworkScanner::derive_subnet(ip)` - Derive /24 subnet from IP (e.g., 192.168.1.0/24)

### hainet-seed/src/installer/nmap_installer.rs (Phase 4.5a.1 - Complete)
- `ensure_nmap_installed(platform)` - Auto-install nmap if not present
- `is_nmap_installed()` - Check if nmap is available

### hainet-seed/src/installer/ssh_client.rs (Phase 7.1 - Complete)
- `SSHClientTrait` - Trait for SSH client operations, to allow for mocking in tests.
- `DeviceCapabilities` - Device assessment result (IP, hostname, CPU, RAM, GPU, disk, OS, arch, score)
- `DeviceCapabilities::calculate_score()` - Calculate capability score for master election (RAM 40%, GPU 30%, CPU 20%, Disk 10%)
- `SSHCredentials` - SSH authentication credentials (username, password)
- `SSHClient::new(ip, credentials)` - Create SSH client for remote device
- `SSHClientTrait::connect()` - Establish SSH connection with TCP handshake (5s timeout)
- `SSHClientTrait::authenticate_password()` - Authenticate with password
- `SSHClientTrait::authenticate_pubkey(private_key_path, passphrase)` - Authenticate with SSH key
- `SSHClientTrait::disconnect()` - Disconnect SSH session cleanly
- `SSHClientTrait::is_connected()` - Check if client is connected and authenticated
- `SSHClient::test_connection()` - Test TCP connection to SSH port (22) with 5s timeout (legacy)
- `SSHClientTrait::execute_command(command)` - Execute remote command via SSH channel
- `SSHClient::execute_command_with_timeout(command, timeout)` - Execute command with timeout
- `SSHClientTrait::assess_capabilities()` - **Real SSH-based device assessment** (CPU, RAM, GPU, disk, OS, arch)
- `SSHClientTrait::upload_file(local_path, remote_path)` - Upload file via SFTP to a temporary location and then move it to the final destination using sudo.
- `SSHClient::download_file(remote_path, local_path)` - Download file via SFTP
- `SSHClientTrait::create_remote_directory(path)` - Create directory on remote device using sudo mkdir -p.
- `SSHClientTrait::set_permissions(path, mode)` - Set file permissions via sudo chmod.
- `SSHClient::remote_file_exists(path)` - Check if file exists on remote device
- `MockSSHClient::new(ip)` - Create a new mock SSH client for testing
- `MockSSHClient::expect_command(command, output)` - Set an expected command and its output
- `MockSSHClient::set_capabilities(caps)` - Set the mock device capabilities to be returned by `assess_capabilities`

### hainet-seed/src/installer/mod.rs (Updated Phase 4.5a.2)
- `Installer::prompt_mesh_setup()` - Prompt user for multi-device mesh setup
- `Installer::discover_mesh_devices()` - Discover SSH-enabled devices on LAN
- `Installer::prompt_assess_devices()` - Prompt user to assess device capabilities
- `Installer::assess_device_capabilities(devices)` - Connect via SSH and assess each device
- `Installer::display_capabilities(capabilities)` - Display assessment results and recommend master node

### hainet-seed/src/installer/deployment.rs (Phase 7.1 - Complete)
- `DeviceRole` - Enum for device roles (Master, Slave, Standalone, UIOnly)
- `DeviceAssignment` - Device with assigned role and capabilities
- `DeploymentOrchestrator::new()` - Create deployment orchestrator
- `DeploymentOrchestrator::assign_roles(capabilities)` - Assign roles based on capability scores
- `DeploymentOrchestrator::deploy_all(username, client_factory)` - Deploy HAI-Net to all assigned devices
- `DeploymentOrchestrator::deploy_to_device(assignment, username, client_factory)` - Deploy to single device
- `DeploymentOrchestrator::build_binaries(arch)` - Cross-compile binaries for target architecture
- `DeploymentOrchestrator::transfer_binaries(client, role)` - Transfer role-specific binaries via SFTP
- `DeploymentOrchestrator::configure_device(client, assignment)` - Create hainet.toml configuration
- `DeploymentOrchestrator::setup_services(client, role)` - Create and enable systemd services
- `DeploymentOrchestrator::initialize_mesh(master, username, client_factory)` - **Start services and verify mesh health**
- `DeploymentOrchestrator::start_services_on_device(ip, username, role, client_factory)` - **Start systemd services remotely**
- `DeploymentOrchestrator::verify_mesh_health(master, username, client_factory)` - **Check master node service status**
- `get_target_triple(arch)` - Maps architecture name to Rust target triple
- `find_workspace_root()` - Finds the workspace root from the current directory

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
- `AdminAgent::process_user_input(user_input)` - Processes user input, detects complex intents, creates project plans, and spawns PM agents.
- `AdminAgent::spawn_pm_agent(project)` - Spawns a new PMAgent to manage a project.
- `Agent::id()` - Get agent identifier
- `Agent::process_message(message)` - Process incoming message
- `Agent::start()` - Start agent main loop
- `Agent::stop()` - Stop agent gracefully

### hainet-persona/src/agents/pm.rs
- `PMAgent::new(id, context)` - Create a new PM agent

### hainet-persona/src/agents/worker.rs
- `WorkerAgent::new(id, context)` - Create a new Worker agent

### hainet-persona/src/agents/templates.rs
- `WorkerTemplate::file_worker()` - Create a FileWorker template
- `WorkerTemplate::code_worker()` - Create a CodeWorker template
- `WorkerTemplate::network_worker()` - Create a NetworkWorker template
- `WorkerTemplate::research_worker()` - Create a ResearchWorker template
- `WorkerTemplate::all_templates()` - Get all available worker templates
- `WorkerTemplate::select_for_task(task_description)` - Select the most appropriate worker template for a task description

### hainet-persona/src/agents/worker_intelligence.rs (Phase 8B Session 3 - Complete)
- `ErrorCategory::classify(error_msg)` - Classify error from patterns (Transient, Permanent, Unknown)
- `TaskOutcome` - Record of task execution (task_type, tool_used, success, duration, retry_count, error_category, timestamp)
- `SuccessMetrics::success_rate()` - Calculate success rate (0.0 to 1.0)
- `SuccessMetrics::is_reliable()` - Check if metrics indicate reliable tool (>= 3 attempts, >= 0.8 success rate)
- `WorkerLearner::new()` - Create learner with default capacity (100 outcomes)
- `WorkerLearner::with_capacity(capacity)` - Create learner with custom capacity
- `WorkerLearner::record_outcome(outcome)` - Record task outcome with FIFO capacity management
- `WorkerLearner::outcome_count()` - Get number of recorded outcomes
- `WorkerLearner::get_tool_metrics(tool)` - Calculate success metrics for specific tool
- `WorkerLearner::get_task_type_metrics(task_type)` - Calculate success metrics for task type
- `WorkerLearner::recommend_tool(task_type, available_tools)` - Recommend best tool based on history
- `ExecutionStrategy::default()` - Create default strategy (5s timeout, 3 retries, 1.5x backoff)
- `ExecutionStrategy::adjust_for_task(task_type, learner)` - Adjust timeouts/retries based on history
- `ExecutionStrategy::retry_delay_ms(attempt)` - Calculate exponential backoff delay for attempt
- `ToolSelector::new(fallback_order)` - Create tool selector with fallback order
- `ToolSelector::select_best_tool(task_type, available_tools)` - Select optimal tool based on learning
- `ToolSelector::record_outcome(outcome)` - Record outcome for learning
- `ToolSelector::learner()` - Get reference to learner for direct access

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

### hainet-portal/src/components/ChatInterface.tsx
- `ChatInterface()` - Renders the chat UI, handles user input, and displays message history. Imports Tauri functions from the `tauri.ts` wrapper.

### hainet-portal/src/lib/tauri.ts
- **Tauri API Wrapper:** This module re-exports functions from the `@tauri-apps/api` package to provide a single point of import for the rest of the application. This helps to work around potential module resolution issues with Vite.

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

### hainet-portal/src/components/SystemStatus.tsx
- `SystemStatus()` - Component to display system status.

---

## Metrics System (Phase 6B Session 1 - Complete)

### hainet-portal/src-tauri/src/metrics_handler.rs
- `get_agent_metrics()` - Get metrics for all agent types (Admin, PM, Worker)
- `get_agent_metrics_by_type(agent_type)` - Get metrics filtered by agent type
- `get_metrics_summary()` - Get high-level metrics summary with totals and averages
- `export_metrics_json()` - Export full metrics report as JSON string
- `get_historical_metrics(time_range)` - Get historical metrics with time filtering
- `get_metrics_trend(interval)` - Get aggregated trend data (Hourly, Daily, Weekly)
- `export_metrics_csv(time_range)` - Export metrics as CSV with time filtering

### hainet-portal/src/hooks/useMetrics.ts
- `useMetrics()` - React hook for fetching and managing real-time and historical metrics. Imports Tauri functions from the `tauri.ts` wrapper.
- `useMetrics()::refetch` - Manually refetch real-time metrics
- `useMetrics()::getTrendData(interval)` - Fetch historical trend data
- `useMetrics()::exportMetrics(format, time_range)` - Export metrics as CSV or JSON

### hainet-portal/src/components/MetricsDashboard.tsx
- `MetricsDashboard()` - Main component for displaying the metrics dashboard, including real-time data, trend charts, and the metrics toolbar. Imports Tauri functions from the `tauri.ts` wrapper.

### hainet-portal/src/components/MetricsToolbar.tsx
- `MetricsToolbar(onExport, onIntervalChange, selectedInterval, isExporting)` - Toolbar for metrics export and interval selection

### hainet-portal/src/types.ts
- `AgentMetrics` - TypeScript interface for individual agent metrics
- `MetricsSummary` - TypeScript interface for aggregated metrics summary
- `TrendDataPoint` - TypeScript interface for a single historical trend data point
- `TrendInterval` - TypeScript type for trend interval ('Hourly', 'Daily', 'Weekly')
- `TimeRange` - TypeScript interface for time range filtering

---

## Local Hub Networking (Phase 4 - In Progress)

### hainet-core/src/networking/coordinator.rs
- `Coordinator::new()` - Creates a new `Coordinator`.
- `Coordinator::run()` - Runs the coordinator event loop.

### hainet-core/src/networking/discovery.rs
- `DiscoveryBehaviour` - The `NetworkBehaviour` for mDNS discovery.

### hainet-core/src/networking/peer.rs
- `PeerBehaviour` - The `NetworkBehaviour` for Ping.
- `PeerEvent` - Events emitted by the `PeerBehaviour`.

---

## Storage System (Phase 4.4 - Complete)

**Status**: ✅ Distributed Storage with CRDT and Merkle Sync

### hainet-core/src/storage/cas.rs
- `ContentHash::from_bytes(data)` - Create content hash from bytes using BLAKE3
- `ContentHash::as_bytes()` - Get hash bytes
- `ContentAddressedStore::new(base_path)` - Create new CAS with BLAKE3 hashing
- `ContentAddressedStore::put(content, mime_type)` - Store content and return hash
- `ContentAddressedStore::get(hash)` - Retrieve content by hash
- `ContentAddressedStore::has(hash)` - Check if content exists
- `ContentAddressedStore::delete(hash)` - Delete content by hash
- `ContentAddressedStore::list_all()` - List all stored content hashes
- `ContentAddressedStore::metadata(hash)` - Get content metadata

### hainet-core/src/storage/crdt.rs
- `Timestamp::now(logical)` - Create hybrid logical timestamp
- `Timestamp::next()` - Increment logical clock
- `Timestamp::merge(other)` - Merge timestamps (max + 1)
- `VectorClock::new()` - Create empty vector clock
- `VectorClock::increment(node)` - Increment clock for node
- `VectorClock::merge(other)` - Merge vector clocks (take max)
- `VectorClock::happens_before(other)` - Check causality (a < b)
- `VectorClock::is_concurrent(other)` - Check if concurrent
- `LWWRegister::new(value, node_id)` - Create Last-Writer-Wins register
- `LWWRegister::set(value)` - Update value (increment timestamp)
- `LWWRegister::merge(other)` - Merge registers (take higher timestamp)
- `GSet::new()` - Create Grow-only set
- `GSet::insert(element)` - Add element to set
- `GSet::merge(other)` - Union merge
- `TwoPhaseSet::new()` - Create add-remove set
- `TwoPhaseSet::insert(element)` - Add element (fails if previously removed)
- `TwoPhaseSet::remove(element)` - Remove element (permanent)
- `TwoPhaseSet::merge(other)` - Merge add and remove sets
- `LWWElementSet::new()` - Create LWW element set
- `LWWElementSet::insert(element, timestamp)` - Add element with timestamp
- `LWWElementSet::remove(element, timestamp)` - Remove element with timestamp
- `LWWElementSet::contains(element)` - Check if element active (add_ts > remove_ts)
- `LWWElementSet::merge(other)` - Merge with max timestamps

### hainet-core/src/storage/distributed.rs
- `DistributedStorage::new(local_node, store, config)` - Create distributed storage manager
- `DistributedStorage::register_node(capacity)` - Register node capacity
- `DistributedStorage::mark_node_offline(node_id)` - Mark node as offline
- `DistributedStorage::online_nodes()` - Get all online nodes
- `DistributedStorage::select_storage_nodes(size, count)` - Select nodes for placement
- `DistributedStorage::store(content, replica_count)` - Store content with replication
- `DistributedStorage::record_replica(hash, node_id)` - Record replica creation
- `DistributedStorage::locate_content(hash)` - Get content locations
- `DistributedStorage::check_replication_health()` - Check health for all content
- `DistributedStorage::under_replicated_content()` - Get under-replicated content
- `DistributedStorage::delete(hash)` - Delete content and update metadata
- `DistributedStorage::garbage_collect()` - Remove orphaned content
- `DistributedStorage::stats()` - Get storage statistics
- `NodeCapacity::usage()` - Calculate usage percentage
- `NodeCapacity::can_store(size)` - Check if node has capacity
- `ReplicationMetadata::new(hash, size, desired_replicas)` - Create replication metadata
- `ReplicationMetadata::add_replica(node_id, timestamp)` - Add replica location
- `ReplicationMetadata::is_sufficiently_replicated()` - Check replication goal
- `ReplicationMetadata::health()` - Get replication health (0.0-1.0)

### hainet-core/src/storage/sync_protocol.rs
- `MerkleTree::build(content, branching_factor)` - Build Merkle tree from content
- `MerkleTree::diff(other)` - Find differences between trees
- `MerkleTree::stats()` - Get tree statistics
- `MerkleNode::leaf(content)` - Create leaf node
- `MerkleNode::internal(children)` - Create internal node
- `SyncProtocol::new(local_node, store)` - Create sync protocol manager
- `SyncProtocol::build_merkle_tree(branching_factor)` - Build and cache Merkle tree
- `SyncProtocol::invalidate_cache()` - Invalidate tree cache
- `SyncProtocol::create_sync_request(remote_node, branching_factor)` - Create sync request
- `SyncProtocol::handle_sync_request(request, branching_factor)` - Handle incoming request
- `SyncProtocol::start_session(remote_node)` - Start sync session
- `SyncProtocol::end_session(session_id)` - End sync session
- `SyncProtocol::sync_with_peer(remote_node, branching_factor)` - Perform full sync
- `SyncProtocol::get_vector_clock()` - Get current vector clock
- `SyncProtocol::update_vector_clock(other)` - Update with remote clock

### hainet-core/src/storage/coordinator.rs
- `StorageCoordinator::new(local_node, store, storage_config, coordinator_config)` - Create coordinator
- `StorageCoordinator::role()` - Get current node role
- `StorageCoordinator::set_role(role)` - Set node role (Master/Slave/Standalone)
- `StorageCoordinator::start()` - Start coordinator background tasks
- `StorageCoordinator::stop()` - Stop coordinator
- `StorageCoordinator::elect_master(candidates)` - Elect master from candidates
- `StorageCoordinator::join_mesh(master_node)` - Join storage mesh as slave
- `StorageCoordinator::promote_to_master()` - Promote to master role
- `StorageCoordinator::get_stats()` - Get storage statistics
- `StorageCoordinator::get_health_status()` - Get health check results
- `StorageCoordinator::trigger_rebalancing()` - Manually trigger rebalancing

### hainet-core/src/storage/sync.rs
- `P2PFileSync::new(store)` - Create P2P sync manager
- `P2PFileSync::register_peer(peer_id, available_hashes)` - Register peer
- `P2PFileSync::request_file(hash, peer_id)` - Request file from peer
- `P2PFileSync::handle_request(request)` - Handle sync request
- `P2PFileSync::find_peers_with_content(hash)` - Find peers with content
- `P2PFileSync::sync_from_peers(hash)` - Sync from any available peer

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

---

## MCP Servers (Phase 5 - In Progress)

### mcp-servers/hainet-system/src/main.rs (Phase 5.2 - Complete)
- `SystemServer::new()` - Create system management MCP server
- `SystemServer::handle_system_status()` - Get CPU, RAM, disk, uptime, OS info
- `SystemServer::handle_list_services()` - List running HAI-Net services (core, chain, bridge, portal, ollama)
- `SystemServer::handle_restart_service(service_name)` - Restart HAI-Net service via systemctl (whitelist-protected)
- `SystemServer::handle_check_health()` - Run comprehensive health checks (CPU, RAM, disk, services)
- **MCP Tools:** `system_status`, `list_services`, `restart_service`, `check_health`
- **Target User:** Admin AI agent for system monitoring and management
- **Status:** ✅ Complete - 450 LOC, 4 tools, compiles cleanly

### mcp-servers/hainet-dev/src/main.rs (Phase 5.3 - Complete)
- `DevServer::new()` - Create development tools MCP server
- `DevServer::handle_git_status(repo_path)` - Get git repository status (modified/untracked files, branch)
- `DevServer::handle_git_diff(repo_path, file_path)` - View git diff for file or entire repository
- `DevServer::handle_git_commit(repo_path, message)` - Stage and commit all changes with message
- `DevServer::handle_cargo_build(package, release)` - Build Rust packages with cargo (optional release mode)
- `DevServer::handle_cargo_test(package, filter)` - Run cargo tests with package and name filters
- `DevServer::handle_code_search(pattern, search_path)` - Search codebase using ripgrep/grep with line numbers
- `DevServer::handle_read_file_lines(file_path, start_line, end_line)` - Read specific line ranges from files (1-based indexing)
- **MCP Tools:** `git_status`, `git_diff`, `git_commit`, `cargo_build`, `cargo_test`, `code_search`, `read_file_lines`
- **Target User:** Worker AI agents for development task execution
- **Status:** ✅ Complete - 480 LOC, 7 tools, compiles cleanly in 1.39s

---

## Test Infrastructure (Phase 6A Session 1 - Complete)

### hainet-persona/tests/helpers/mod.rs
- `TestRetryConfig::default()` - Create default retry config (3 attempts, 100ms delay)
- `TestRetryConfig::with_attempts(attempts)` - Create config with custom attempts
- `TestRetryConfig::no_validation()` - Create config without format validation
- `retry_with_validation(config, test_fn)` - Execute test with retry logic
- `FailureCategory::from_error(error)` - Categorize error (Infrastructure, LlmVariability, CodeBug, Environment, Unknown)
- `TestResultAnalyzer::new()` - Create result analyzer
- `TestResultAnalyzer::add_result(result)` - Add test result
- `TestResultAnalyzer::pass_rate()` - Calculate overall pass rate
- `TestResultAnalyzer::failure_breakdown()` - Get failure breakdown by category
- `TestResultAnalyzer::average_duration()` - Get average test duration
- `TestResultAnalyzer::average_retries()` - Get average retry count
- `TestResultAnalyzer::print_report()` - Print detailed analysis report
- `execute_test_with_analysis(test_name, config, analyzer, test_fn)` - Execute test with full tracking

### hainet-persona/tests/helpers/json_validator.rs
- `ProjectPlanSchema::default()` - Create default project plan schema
- `ProjectPlanSchema::validate(value)` - Validate JSON against project plan schema
- `TaskDecompositionSchema::default()` - Create default task decomposition schema
- `TaskDecompositionSchema::validate(value)` - Validate JSON against task schema
- `JSONValidator::parse_with_fallbacks(text)` - Parse JSON with 4 fallback strategies
- `JSONValidator::extract_from_markdown(text)` - Extract JSON from markdown code blocks
- `JSONValidator::repair_and_parse(text)` - Repair common JSON issues and parse
- `JSONValidator::regex_extract(text)` - Extract JSON using regex patterns
- `JSONValidator::validate_structure(text)` - Validate JSON structure before parsing
- `JSONValidator::parse_and_validate(text, schema)` - Parse and validate against schema
- `ParsingStrategy` enum - Tracks which parsing strategy succeeded (DirectParse, MarkdownExtraction, JsonRepair, RegexExtraction, Failed)
- `ParseResult` struct - Contains parsed value, strategy used, error details

---

<!-- # END OF FILE helperfiles/FUNCTIONS_INDEX.md -->
