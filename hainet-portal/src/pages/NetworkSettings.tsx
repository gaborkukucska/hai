import React from 'react';

export default function NetworkSettings() {
  return (
    <div className="flex-1 h-full overflow-y-auto bg-theme-bg-primary text-theme-text-primary p-6">
      <div className="max-w-4xl mx-auto space-y-8">
        
        <div>
          <h1 className="text-2xl font-bold">Mesh Network & Settings</h1>
          <p className="text-theme-text-muted text-sm mt-1">Configure your HAI-Net node, UI theme, and mesh peers.</p>
        </div>

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
