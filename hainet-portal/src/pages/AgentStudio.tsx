import React from 'react';

export default function AgentStudio() {
  return (
    <div className="flex-1 h-full overflow-y-auto bg-theme-bg-primary text-theme-text-primary p-6">
      <div className="max-w-5xl mx-auto space-y-6">
        
        <div className="flex justify-between items-center mb-8">
          <div>
            <h1 className="text-2xl font-bold">Agent Studio</h1>
            <p className="text-theme-text-muted text-sm mt-1">Orchestrate your local AI workforce (TrippleEffect & NoSlop Engine)</p>
          </div>
          <button className="px-4 py-2 bg-theme-accent-primary text-theme-bg-primary font-bold rounded-md hover:bg-theme-accent-secondary transition-colors">
            + New Project
          </button>
        </div>

        <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
          {/* Active Agents */}
          <div className="col-span-1 bg-theme-bg-secondary border border-theme-border rounded-xl p-5">
             <h2 className="text-lg font-semibold mb-4 flex items-center gap-2">
               <span className="w-2 h-2 rounded-full bg-theme-accent-success animate-pulse"></span>
               Active Swarm
             </h2>
             <div className="space-y-3">
               <div className="flex items-center justify-between p-2 rounded-md hover:bg-theme-bg-tertiary transition-colors">
                 <div>
                   <p className="font-medium text-sm">Project Manager</p>
                   <p className="text-xs text-theme-text-muted">Status: Planning</p>
                 </div>
                 <span className="text-xs px-2 py-1 bg-theme-bg-tertiary rounded text-theme-text-secondary">qwen3.5:9b</span>
               </div>
               <div className="flex items-center justify-between p-2 rounded-md hover:bg-theme-bg-tertiary transition-colors">
                 <div>
                   <p className="font-medium text-sm">Media Editor</p>
                   <p className="text-xs text-theme-text-muted">Status: Rendering Video</p>
                 </div>
                 <span className="text-xs px-2 py-1 bg-theme-bg-tertiary rounded text-theme-text-secondary">ComfyUI</span>
               </div>
             </div>
          </div>

          {/* Active Project / Tasks */}
          <div className="col-span-2 bg-theme-bg-secondary border border-theme-border rounded-xl p-5 flex flex-col">
             <h2 className="text-lg font-semibold mb-4">Current Project: "Cyberpunk Promo Video"</h2>
             <div className="flex-1 bg-theme-bg-primary rounded-md border border-theme-border p-4 font-mono text-xs text-theme-text-muted space-y-2 overflow-y-auto">
                <p><span className="text-theme-accent-primary">[PM]</span> Task decomposed into 3 subtasks.</p>
                <p><span className="text-theme-accent-success">[Worker: Writer]</span> Script draft completed.</p>
                <p><span className="text-theme-accent-success">[Worker: Editor]</span> Received script, generating prompts for ComfyUI...</p>
                <p><span className="text-theme-text-secondary">[System]</span> Triggering workflow 'vid2vid' via local ComfyUI instance.</p>
             </div>
             <div className="mt-4 flex gap-2">
                <button className="flex-1 px-4 py-2 bg-theme-bg-tertiary text-theme-text-primary rounded-md hover:bg-theme-border transition-colors">Pause Project</button>
                <button className="flex-1 px-4 py-2 bg-theme-bg-tertiary text-theme-text-primary rounded-md hover:bg-theme-border transition-colors">View Outputs</button>
             </div>
          </div>
        </div>

      </div>
    </div>
  );
}
