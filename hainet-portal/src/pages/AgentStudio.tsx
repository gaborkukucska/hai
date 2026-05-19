// <!-- # START OF FILE hainet-portal/src/pages/AgentStudio.tsx -->
// Agent Studio page — wired to hainet-core for real agent & project data.
// Polls active agents and projects from the TrippleEffect bridge every 5 seconds.

import React, { useState, useEffect } from 'react';
import { invoke } from '../lib/tauri';

/** Active agent info from the backend */
interface AgentInfo {
  agent_id: string;
  agent_type: string;
  status: string;
  model?: string;
  state?: string;
}

/** Active project info from the backend */
interface ProjectInfo {
  project_id: string;
  title: string;
  status: string;
  tasks?: any[];
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
                   <div key={agent.agent_id} className="flex items-center justify-between p-2 rounded-md hover:bg-theme-bg-tertiary transition-colors">
                     <div>
                       <p className="font-medium text-sm">{agent.agent_type}</p>
                       <p className="text-xs text-theme-text-muted flex items-center gap-1">
                         <span className={`w-1.5 h-1.5 rounded-full ${getStatusColor(agent.status)}`}></span>
                         {agent.state || agent.status || 'Unknown'}
                       </p>
                     </div>
                     <span className="text-xs px-2 py-1 bg-theme-bg-tertiary rounded text-theme-text-secondary">
                       {agent.model || 'default'}
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
             <div className="flex-1 bg-theme-bg-primary rounded-md border border-theme-border p-4 font-mono text-xs text-theme-text-muted space-y-2 overflow-y-auto min-h-[200px]">
               {!selectedProject ? (
                 <p className="text-theme-text-muted">Create a new project to see live task output here.</p>
               ) : (
                 <>
                   <p><span className="text-theme-accent-primary">[System]</span> Project "{selectedProject.title}" is {selectedProject.status}.</p>
                   {agents.map(agent => (
                     <p key={agent.agent_id}>
                       <span className="text-theme-accent-success">[{agent.agent_type}]</span>{' '}
                       State: {agent.state || 'initializing'} — Status: {agent.status || 'idle'}
                     </p>
                   ))}
                 </>
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
                        onClick={() => handleResume(selectedProject.project_id)}
                        className="flex-1 px-4 py-2 bg-theme-accent-success/20 border border-theme-accent-success text-theme-text-primary rounded-md transition-colors"
                      >
                        Resume Project
                      </button>
                    ) : (
                      <button
                        onClick={() => handlePause(selectedProject.project_id)}
                        className="flex-1 px-4 py-2 bg-theme-bg-tertiary hover:bg-theme-border text-theme-text-primary rounded-md transition-colors"
                      >
                        Pause Project
                      </button>
                    )}
                    <button
                      onClick={() => handleStop(selectedProject.project_id)}
                      className="px-4 py-2 bg-theme-accent-danger/20 border border-theme-accent-danger/30 text-theme-accent-danger rounded-md hover:bg-theme-accent-danger/30 transition-colors"
                    >
                      Stop
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
                  key={project.project_id}
                  onClick={() => setSelectedProject(project)}
                  className={`p-3 rounded-md cursor-pointer flex justify-between items-center transition-colors ${
                    selectedProject?.project_id === project.project_id
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
