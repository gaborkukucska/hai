import React from 'react';

export default function ChatView() {
  return (
    <div className="flex h-full text-theme-text-primary bg-theme-bg-primary">
      {/* Left Sidebar: Threads */}
      <div className="w-80 border-r border-theme-border bg-theme-bg-secondary flex flex-col">
        <div className="p-4 border-b border-theme-border">
          <h2 className="text-lg font-bold text-theme-text-primary">Chat & Comms</h2>
          <div className="mt-2 flex gap-2">
             <button className="text-xs bg-theme-bg-tertiary px-2 py-1 rounded text-theme-text-secondary hover:text-theme-text-primary">All</button>
             <button className="text-xs bg-theme-bg-tertiary px-2 py-1 rounded text-theme-text-secondary hover:text-theme-text-primary">Agents</button>
             <button className="text-xs bg-theme-bg-tertiary px-2 py-1 rounded text-theme-text-secondary hover:text-theme-text-primary">Peers</button>
          </div>
        </div>
        <div className="flex-1 overflow-y-auto p-2 space-y-1">
          <div className="p-3 rounded-md bg-theme-bg-tertiary/50 cursor-pointer border-l-2 border-theme-accent-primary">
            <h3 className="text-sm font-semibold">Admin AI</h3>
            <p className="text-xs text-theme-text-muted truncate mt-1">Ready for the next task.</p>
          </div>
        </div>
      </div>

      {/* Main Chat Area */}
      <div className="flex-1 flex flex-col">
        <div className="p-4 border-b border-theme-border flex items-center justify-between bg-theme-bg-secondary">
          <h2 className="text-lg font-semibold">Admin AI</h2>
          <span className="text-xs px-2 py-1 bg-theme-accent-success/20 text-theme-accent-success rounded-full border border-theme-accent-success/30">Online</span>
        </div>
        
        <div className="flex-1 overflow-y-auto p-4 space-y-4">
          <div className="flex items-start gap-3">
             <div className="w-8 h-8 rounded-full bg-theme-accent-primary shrink-0"></div>
             <div className="bg-theme-bg-secondary border border-theme-border p-3 rounded-2xl rounded-tl-none max-w-[80%]">
                <p className="text-sm">Hello! I am your HAI-Net Admin AI. How can I help you today?</p>
             </div>
          </div>
        </div>
        
        <div className="p-4 border-t border-theme-border bg-theme-bg-secondary">
          <div className="flex gap-2">
            <button className="p-2 text-theme-text-muted hover:text-theme-text-primary bg-theme-bg-tertiary rounded-md">📎</button>
            <input 
              type="text" 
              placeholder="Message Admin AI..." 
              className="flex-1 bg-theme-bg-tertiary border border-theme-border rounded-md px-4 py-2 focus:outline-none focus:border-theme-accent-primary"
            />
            <button className="px-4 py-2 bg-theme-accent-primary text-theme-bg-primary font-bold rounded-md hover:bg-theme-accent-secondary transition-colors">Send</button>
          </div>
        </div>
      </div>
    </div>
  );
}
