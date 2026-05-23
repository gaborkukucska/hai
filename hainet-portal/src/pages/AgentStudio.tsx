// <!-- # START OF FILE hainet-portal/src/pages/AgentStudio.tsx -->
// Agent Studio page — wired to hainet-core for real agent & project data.
// Polls active agents and projects from the TrippleEffect bridge every 5 seconds.

import React, { useState, useEffect } from 'react';
import { invoke } from '../lib/tauri';

/** Active agent info from the backend */
interface AgentId {
  agent_type: string;
  name: string;
}

interface AgentStatus {
  state: string;
  activity: string;
  last_updated: number;
}

interface AgentInfo {
  id: AgentId;
  status: AgentStatus | null;
}

interface TaskInfo {
  id: string;
  title: string;
  description: string;
  status: string;
  dependencies: string[];
  worker_agent_id?: string;
}

/** Active project info from the backend */
interface ProjectInfo {
  id: string;
  title: string;
  status: string;
  tasks?: TaskInfo[];
}

export default function AgentStudio() {
  const [agents, setAgents] = useState<AgentInfo[]>([]);
  const [projects, setProjects] = useState<ProjectInfo[]>([]);
  const [selectedProject, setSelectedProject] = useState<ProjectInfo | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [showOutput, setShowOutput] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Fetch agents and projects from the backend
  const fetchData = async () => {
    try {
      // Fetch active agents from the TrippleEffect bridge
      const agentData = await invoke<any>('get_active_agents');
      if (Array.isArray(agentData)) {
        setAgents(agentData);
        console.debug('[AgentStudio] Loaded', agentData.length, 'active agents');
      } else {
        setAgents([]);
      }

      // Fetch active projects
      const projectData = await invoke<any>('get_active_projects');
      if (Array.isArray(projectData)) {
        setProjects(projectData);
        if (projectData.length > 0 && !selectedProject) {
          setSelectedProject(projectData[0]);
        }
        console.debug('[AgentStudio] Loaded', projectData.length, 'active projects');
      } else {
        setProjects([]);
      }

      setError(null);
    } catch (e: any) {
      console.debug('[AgentStudio] Backend not available:', e.message);
      setError('Agent system not connected. Start hainet-persona to see live agents.');
    } finally {
      setIsLoading(false);
    }
  };

  // Poll every 5 seconds for live updates
  useEffect(() => {
    fetchData();
    const interval = setInterval(fetchData, 5000);
    return () => clearInterval(interval);
  }, []);

  /** Pause a running project */
  const handlePause = async (projectId: string) => {
    try {
      await invoke('pause_project', { project_id: projectId });
      console.debug('[AgentStudio] Paused project:', projectId);
      fetchData(); // Refresh immediately
    } catch (e: any) {
      console.error('[AgentStudio] Failed to pause:', e);
    }
  };

  /** Resume a paused project */
  const handleResume = async (projectId: string) => {
    try {
      await invoke('resume_project', { project_id: projectId });
      console.debug('[AgentStudio] Resumed project:', projectId);
      fetchData();
    } catch (e: any) {
      console.error('[AgentStudio] Failed to resume:', e);
    }
  };

  /** Stop a running project */
  const handleStop = async (projectId: string) => {
    try {
      await invoke('stop_project', { project_id: projectId });
      console.debug('[AgentStudio] Stopped project:', projectId);
      fetchData();
    } catch (e: any) {
      console.error('[AgentStudio] Failed to stop:', e);
    }
  };

  /** Delete a project */
  const handleDelete = async (projectId: string) => {
    if (!window.confirm('Are you sure you want to delete this project?')) return;
    try {
      await invoke('delete_project', { project_id: projectId });
      console.debug('[AgentStudio] Deleted project:', projectId);
      setSelectedProject(null);
      fetchData();
    } catch (e: any) {
      console.error('[AgentStudio] Failed to delete:', e);
    }
  };

  /** Get status color indicator */
  const getStatusColor = (status: string) => {
    switch (status?.toLowerCase()) {
      case 'idle': return 'bg-theme-accent-success';
      case 'processing': return 'bg-yellow-500 animate-pulse';
      case 'error': return 'bg-theme-accent-danger';
      case 'paused': return 'bg-yellow-500';
      default: return 'bg-theme-text-muted';
    }
  };

  /** Get task status badge styling */
  const getTaskBadge = (status: string) => {
    switch (status?.toLowerCase()) {
      case 'unassigned': return 'bg-theme-bg-tertiary text-theme-text-muted border-theme-border';
      case 'inprogress': return 'bg-blue-500/20 text-blue-400 border-blue-500/30 animate-pulse';
      case 'underreview': return 'bg-purple-500/20 text-purple-400 border-purple-500/30';
      case 'needsrevision': return 'bg-yellow-500/20 text-yellow-400 border-yellow-500/30';
      case 'complete': return 'bg-green-500/20 text-green-400 border-green-500/30';
      case 'failed': return 'bg-red-500/20 text-red-400 border-red-500/30';
      case 'stuck': return 'bg-orange-500/20 text-orange-400 border-orange-500/30';
      default: return 'bg-theme-bg-tertiary text-theme-text-muted border-theme-border';
    }
  };

  /** Render a task and its dependents recursively */
  const renderTaskTree = (task: TaskInfo, allTasks: TaskInfo[], depth: number = 0, visited: Set<string> = new Set()) => {
    if (visited.has(task.id)) return null; // Prevent infinite loops in cycles
    visited.add(task.id);

    // Find tasks that depend on THIS task
    const dependents = allTasks.filter(t => t.dependencies.includes(task.id));
    
    // Find assigned worker name if possible
    let workerName = 'Unassigned';
    if (task.worker_agent_id) {
      const worker = agents.find(a => a.id.name === task.worker_agent_id || a.id.agent_type === task.worker_agent_id);
      workerName = worker ? worker.id.name : task.worker_agent_id;
    }

    return (
      <div key={task.id} className="flex flex-col mt-2">
        <div 
          className={`flex items-center gap-3 p-3 rounded-lg border bg-theme-bg-primary transition-all hover:border-theme-accent-primary/50`}
          style={{ marginLeft: `${depth * 1.5}rem` }}
        >
          {/* Status icon / branch line */}
          <div className="flex-shrink-0 relative">
            {depth > 0 && (
              <div className="absolute -left-6 top-1/2 w-4 border-t-2 border-theme-border border-dashed"></div>
            )}
            <div className={`w-3 h-3 rounded-full ${getTaskBadge(task.status).split(' ')[0]}`}></div>
          </div>
          
          <div className="flex-1 min-w-0">
            <div className="flex justify-between items-start mb-1">
              <h4 className="text-sm font-medium text-theme-text-primary truncate" title={task.title}>
                {task.title}
              </h4>
              <span className={`text-[10px] uppercase font-bold px-2 py-0.5 rounded-full border ${getTaskBadge(task.status)}`}>
                {task.status}
              </span>
            </div>
            
            <div className="flex items-center justify-between mt-1.5">
              <span className="text-xs text-theme-text-muted truncate max-w-[70%]" title={task.description}>
                {task.description.length > 60 ? task.description.substring(0, 60) + '...' : task.description}
              </span>
              {task.worker_agent_id && (
                <span className="text-[10px] flex items-center gap-1 text-theme-accent-primary bg-theme-accent-primary/10 px-2 py-0.5 rounded">
                  <svg className="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 13.255A23.931 23.931 0 0112 15c-3.183 0-6.22-.62-9-1.745M16 6V4a2 2 0 00-2-2h-4a2 2 0 00-2 2v2m4 6h.01M5 20h14a2 2 0 002-2V8a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z" />
                  </svg>
                  {workerName}
                </span>
              )}
            </div>
          </div>
        </div>
        
        {/* Render dependents nested underneath */}
        {dependents.length > 0 && (
          <div className="flex flex-col relative">
            <div className="absolute left-[0.3rem] top-0 bottom-4 border-l-2 border-theme-border border-dashed" style={{ marginLeft: `${depth * 1.5}rem` }}></div>
            {dependents.map(dep => renderTaskTree(dep, allTasks, depth + 1, visited))}
          </div>
        )}
      </div>
    );
  };

  return (
    <div className="flex-1 h-full overflow-y-auto bg-theme-bg-primary text-theme-text-primary p-6">
      <div className="max-w-5xl mx-auto space-y-6">

        <div className="flex justify-between items-center mb-8">
          <div>
            <h1 className="text-2xl font-bold">Agent Studio</h1>
            <p className="text-theme-text-muted text-sm mt-1">Orchestrate your local AI workforce (TrippleEffect & NoSlop Engine)</p>
          </div>
          <button
            id="new-project-btn"
            className="px-4 py-2 bg-theme-accent-primary text-theme-bg-primary font-bold rounded-md hover:bg-theme-accent-secondary transition-colors"
          >
            + New Project
          </button>
        </div>

        {/* Error banner */}
        {error && (
          <div className="bg-yellow-500/10 border border-yellow-500/30 text-yellow-400 px-4 py-3 rounded-lg text-sm">
            ℹ️ {error}
          </div>
        )}

        <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
          {/* Active Agents */}
          <div className="col-span-1 bg-theme-bg-secondary border border-theme-border rounded-xl p-5">
             <h2 className="text-lg font-semibold mb-4 flex items-center gap-2">
               <span className={`w-2 h-2 rounded-full ${agents.length > 0 ? 'bg-theme-accent-success animate-pulse' : 'bg-theme-text-muted'}`}></span>
               Active Swarm ({agents.length})
             </h2>
             <div className="space-y-3">
               {isLoading ? (
                 <p className="text-xs text-theme-text-muted">Loading agents...</p>
               ) : agents.length === 0 ? (
                 <p className="text-xs text-theme-text-muted">No agents running. Create a project to start the swarm.</p>
               ) : (
                 agents.map(agent => (
                   <div key={agent.id?.name || String(Math.random())} className="flex items-center justify-between p-2 rounded-md hover:bg-theme-bg-tertiary transition-colors">
                     <div>
                       <p className="font-medium text-sm">{agent.id?.agent_type || 'Agent'}</p>
                       <p className="text-xs text-theme-text-muted flex items-center gap-1">
                         <span className={`w-1.5 h-1.5 rounded-full ${getStatusColor(agent.status?.state || 'idle')}`}></span>
                         {agent.status?.state || agent.status?.activity || 'Unknown'}
                       </p>
                     </div>
                     <span className="text-xs px-2 py-1 bg-theme-bg-tertiary rounded text-theme-text-secondary">
                       {agent.id?.name || 'default'}
                     </span>
                   </div>
                 ))
               )}
             </div>
          </div>

          {/* Active Project / Tasks */}
          <div className="col-span-2 bg-theme-bg-secondary border border-theme-border rounded-xl p-5 flex flex-col">
             <h2 className="text-lg font-semibold mb-4">
               {selectedProject
                 ? `Current Project: "${selectedProject.title}"`
                 : 'No Active Project'
               }
             </h2>

             {/* Project log / task output area */}
             <div className="flex-1 bg-theme-bg-tertiary/30 rounded-md border border-theme-border p-4 overflow-y-auto min-h-[300px]">
               {!selectedProject ? (
                 <div className="h-full flex flex-col items-center justify-center text-theme-text-muted">
                   <svg className="w-12 h-12 mb-3 opacity-20" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                     <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1} d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2" />
                   </svg>
                   <p>Create a new project to see the task delegation tree here.</p>
                 </div>
               ) : (
                 <div className="space-y-1 pb-4">
                   <div className="flex justify-between items-center mb-4 border-b border-theme-border pb-3">
                     <h3 className="font-semibold text-theme-text-primary">Task Dependency Tree</h3>
                     <span className="text-xs bg-theme-bg-tertiary px-2 py-1 rounded text-theme-text-secondary">
                       {selectedProject.tasks?.filter(t => t.status === 'Complete').length || 0} / {selectedProject.tasks?.length || 0} Tasks Done
                     </span>
                   </div>
                   
                   {/* Find root tasks (no dependencies) and render their trees */}
                   {selectedProject.tasks && selectedProject.tasks.length > 0 ? (
                     selectedProject.tasks
                       .filter(t => !t.dependencies || t.dependencies.length === 0)
                       .map(rootTask => renderTaskTree(rootTask, selectedProject.tasks!))
                   ) : (
                     <p className="text-sm text-theme-text-muted italic py-4 text-center">
                       PM Agent is analyzing project and generating tasks...
                     </p>
                   )}
                 </div>
               )}
             </div>

             {showOutput && selectedProject && (
               <div className="mt-4 p-4 border border-theme-accent-primary/30 bg-theme-bg-tertiary rounded-md">
                 <p className="text-sm font-medium mb-2">Generated Output</p>
                 <div className="w-full h-32 bg-black rounded flex items-center justify-center border border-theme-border">
                   <p className="text-theme-text-muted text-xs font-mono">Waiting for output...</p>
                 </div>
               </div>
             )}

             <div className="mt-4 flex gap-2">
                {selectedProject && (
                  <>
                    {selectedProject.status === 'paused' ? (
                      <button
                        onClick={() => handleResume(selectedProject.id)}
                        className="flex-1 px-4 py-2 bg-theme-accent-success/20 border border-theme-accent-success text-theme-text-primary rounded-md transition-colors"
                      >
                        Resume Project
                      </button>
                    ) : (
                      <button
                        onClick={() => handlePause(selectedProject.id)}
                        className="flex-1 px-4 py-2 bg-theme-bg-tertiary hover:bg-theme-border text-theme-text-primary rounded-md transition-colors"
                      >
                        Pause Project
                      </button>
                    )}
                    <button
                      onClick={() => handleStop(selectedProject.id)}
                      className="px-4 py-2 bg-theme-accent-danger/20 border border-theme-accent-danger/30 text-theme-accent-danger rounded-md hover:bg-theme-accent-danger/30 transition-colors"
                    >
                      Stop
                    </button>
                    <button
                      onClick={() => handleDelete(selectedProject.id)}
                      className="px-4 py-2 bg-red-600/20 border border-red-500/30 text-red-500 rounded-md hover:bg-red-600/30 transition-colors ml-auto"
                      title="Delete Project"
                    >
                      Delete
                    </button>
                  </>
                )}
                <button
                  onClick={() => setShowOutput(!showOutput)}
                  className="flex-1 px-4 py-2 bg-theme-bg-tertiary text-theme-text-primary rounded-md hover:bg-theme-border transition-colors"
                >
                  {showOutput ? 'Hide Outputs' : 'View Outputs'}
                </button>
             </div>
          </div>
        </div>

        {/* All Projects List */}
        {projects.length > 1 && (
          <div className="bg-theme-bg-secondary border border-theme-border rounded-xl p-5">
            <h2 className="text-lg font-semibold mb-4">All Projects</h2>
            <div className="space-y-2">
              {projects.map(project => (
                <div
                  key={project.id}
                  onClick={() => setSelectedProject(project)}
                  className={`p-3 rounded-md cursor-pointer flex justify-between items-center transition-colors ${
                    selectedProject?.id === project.id
                      ? 'bg-theme-bg-tertiary border-l-2 border-theme-accent-primary'
                      : 'hover:bg-theme-bg-tertiary/50'
                  }`}
                >
                  <div>
                    <p className="text-sm font-medium">{project.title}</p>
                    <p className="text-xs text-theme-text-muted">{project.status}</p>
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}

      </div>
    </div>
  );
}
