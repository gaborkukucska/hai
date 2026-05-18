import React, { useState, useEffect } from 'react';

interface NodeStatus {
  status: string;
  service: string;
  role: string;
  version: string;
}

export default function NetworkSettings() {
  const [nodeStatus, setNodeStatus] = useState<NodeStatus | null>(null);
  const [isChecking, setIsChecking] = useState(true);

  useEffect(() => {
    const checkStatus = async () => {
      setIsChecking(true);
      const host = window.location.hostname || '127.0.0.1';
      const portsToTry = [8080, 8081, 8082, 8083];
      
      let found = false;
      for (const port of portsToTry) {
        try {
          // Use AbortController for faster timeout on dead ports
          const controller = new AbortController();
          const timeoutId = setTimeout(() => controller.abort(), 1500);
          
          const res = await fetch(`http://${host}:${port}/health`, { signal: controller.signal });
          clearTimeout(timeoutId);
          
          if (res.ok) {
            const data = await res.json();
            setNodeStatus(data);
            found = true;
            break;
          }
        } catch (e) {
          // Ignore and try next port
        }
      }
      
      if (!found) {
        setNodeStatus(null);
      }
      setIsChecking(false);
    };

    checkStatus();
    const interval = setInterval(checkStatus, 5000);
    return () => clearInterval(interval);
  }, []);

  return (
    <div className="flex-1 h-full overflow-y-auto bg-theme-bg-primary text-theme-text-primary p-6">
      <div className="max-w-4xl mx-auto space-y-8">
        
        <div>
          <h1 className="text-2xl font-bold">Mesh Network & Settings</h1>
          <p className="text-theme-text-muted text-sm mt-1">Configure your HAI-Net node, UI theme, and mesh peers.</p>
        </div>

        {/* Local Node Status */}
        <section className="bg-theme-bg-secondary border border-theme-border rounded-xl p-5">
          <div className="flex justify-between items-center mb-4 border-b border-theme-border pb-2">
            <h2 className="text-lg font-semibold">Local Node Status (hainet-core)</h2>
            <div className="flex items-center gap-2">
              {isChecking ? (
                <span className="text-xs text-theme-text-muted">Checking...</span>
              ) : nodeStatus ? (
                <span className="flex items-center gap-1 text-xs px-2 py-1 bg-theme-accent-success/20 text-theme-accent-success rounded-full border border-theme-accent-success/30">
                  <span className="w-1.5 h-1.5 rounded-full bg-theme-accent-success animate-pulse"></span>
                  Online
                </span>
              ) : (
                <span className="flex items-center gap-1 text-xs px-2 py-1 bg-theme-accent-danger/20 text-theme-accent-danger rounded-full border border-theme-accent-danger/30">
                  Offline
                </span>
              )}
            </div>
          </div>
          {nodeStatus ? (
            <div className="grid grid-cols-3 gap-4">
               <div className="p-3 bg-theme-bg-tertiary rounded-md border border-theme-border/50">
                 <p className="text-xs text-theme-text-muted uppercase tracking-wider">Service</p>
                 <p className="font-medium mt-1">{nodeStatus.service}</p>
               </div>
               <div className="p-3 bg-theme-bg-tertiary rounded-md border border-theme-border/50">
                 <p className="text-xs text-theme-text-muted uppercase tracking-wider">Role</p>
                 <p className="font-medium mt-1 capitalize">{nodeStatus.role}</p>
               </div>
               <div className="p-3 bg-theme-bg-tertiary rounded-md border border-theme-border/50">
                 <p className="text-xs text-theme-text-muted uppercase tracking-wider">Version</p>
                 <p className="font-medium mt-1">{nodeStatus.version}</p>
               </div>
            </div>
          ) : (
            <p className="text-sm text-theme-text-muted">Local core daemon is not reachable on port 8080.</p>
          )}
        </section>

        {/* Theme Settings */}
        <section className="bg-theme-bg-secondary border border-theme-border rounded-xl p-5">
          <h2 className="text-lg font-semibold mb-4 border-b border-theme-border pb-2">Appearance</h2>
          <div className="space-y-4">
             <div>
               <label className="block text-sm font-medium text-theme-text-secondary mb-2">Theme Preset</label>
               <div className="flex gap-3">
                 <button className="px-4 py-2 rounded-md bg-theme-bg-tertiary border-2 border-theme-accent-primary text-sm font-medium">Dark Earthy</button>
                 <button className="px-4 py-2 rounded-md bg-[#0B1221] border-2 border-transparent text-sm font-medium text-blue-400">Cyber Blue</button>
                 <button className="px-4 py-2 rounded-md bg-zinc-950 border-2 border-transparent text-sm font-medium text-zinc-300">Monochrome</button>
               </div>
               <p className="text-xs text-theme-text-muted mt-2">Customizing themes requires editing tailwind CSS variables currently.</p>
             </div>
          </div>
        </section>

        {/* Provider Configuration */}
        <section className="bg-theme-bg-secondary border border-theme-border rounded-xl p-5">
          <h2 className="text-lg font-semibold mb-4 border-b border-theme-border pb-2">AI Providers (TrippleEffect)</h2>
          <div className="space-y-4">
             <div className="grid grid-cols-2 gap-4">
               <div>
                 <label className="block text-sm font-medium text-theme-text-secondary mb-1">Local Ollama URL</label>
                 <input type="text" defaultValue="http://127.0.0.1:11434" className="w-full bg-theme-bg-tertiary border border-theme-border rounded-md px-3 py-2" />
               </div>
               <div>
                 <label className="block text-sm font-medium text-theme-text-secondary mb-1">OpenRouter API Key</label>
                 <input type="password" placeholder="sk-or-v1-..." className="w-full bg-theme-bg-tertiary border border-theme-border rounded-md px-3 py-2" />
               </div>
             </div>
             <button className="px-4 py-2 bg-theme-bg-tertiary text-sm rounded-md hover:bg-theme-border transition-colors">Save Providers</button>
          </div>
        </section>

        {/* Mesh Peers */}
        <section className="bg-theme-bg-secondary border border-theme-border rounded-xl p-5">
          <div className="flex justify-between items-center mb-4 border-b border-theme-border pb-2">
            <h2 className="text-lg font-semibold">Mesh Peers</h2>
            <button className="text-sm text-theme-accent-primary hover:underline">Add Peer</button>
          </div>
          <div className="space-y-2">
             <div className="p-3 bg-theme-bg-tertiary rounded-md flex justify-between items-center">
               <div>
                 <p className="font-medium text-sm">lenovo (10.208.118.21)</p>
                 <p className="text-xs text-theme-accent-success">Connected • Slave Node</p>
               </div>
               <button className="text-xs text-theme-accent-danger hover:underline">Disconnect</button>
             </div>
             <div className="p-3 bg-theme-bg-tertiary rounded-md flex justify-between items-center">
               <div>
                 <p className="font-medium text-sm">Air14 (10.208.118.168)</p>
                 <p className="text-xs text-theme-accent-success">Connected • Slave Node</p>
               </div>
               <button className="text-xs text-theme-accent-danger hover:underline">Disconnect</button>
             </div>
          </div>
        </section>

      </div>
    </div>
  );
}
