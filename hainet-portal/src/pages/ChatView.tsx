import React, { useState } from 'react';

export default function ChatView() {
  const [messages, setMessages] = useState([
    { id: 1, role: 'assistant', content: 'Hello! I am your HAI-Net Admin AI. How can I help you today?' }
  ]);
  const [input, setInput] = useState('');

  const handleSend = () => {
    if (!input.trim()) return;
    const newMsg = { id: Date.now(), role: 'user', content: input };
    setMessages([...messages, newMsg]);
    setInput('');
    
    // Mock response
    setTimeout(() => {
      setMessages(prev => [...prev, { 
        id: Date.now() + 1, 
        role: 'assistant', 
        content: "I'm currently in mock mode. The Rust backend gRPC bridge will be connected soon!" 
      }]);
    }, 1000);
  };

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
        
        <div className="flex-1 overflow-y-auto p-4 space-y-4 flex flex-col">
          {messages.map(msg => (
            <div key={msg.id} className={`flex items-start gap-3 ${msg.role === 'user' ? 'flex-row-reverse' : ''}`}>
               <div className={`w-8 h-8 rounded-full shrink-0 flex items-center justify-center text-xs font-bold ${msg.role === 'user' ? 'bg-theme-bg-tertiary text-theme-text-secondary' : 'bg-theme-accent-primary text-theme-bg-primary'}`}>
                 {msg.role === 'user' ? 'U' : 'AI'}
               </div>
               <div className={`bg-theme-bg-secondary border border-theme-border p-3 rounded-2xl max-w-[80%] ${msg.role === 'user' ? 'rounded-tr-none bg-theme-bg-tertiary' : 'rounded-tl-none'}`}>
                  <p className="text-sm whitespace-pre-wrap">{msg.content}</p>
               </div>
            </div>
          ))}
        </div>
        
        <div className="p-4 border-t border-theme-border bg-theme-bg-secondary">
          <div className="flex gap-2">
            <button className="p-2 text-theme-text-muted hover:text-theme-text-primary bg-theme-bg-tertiary rounded-md">📎</button>
            <input 
              type="text" 
              placeholder="Message Admin AI..." 
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && handleSend()}
              className="flex-1 bg-theme-bg-tertiary border border-theme-border rounded-md px-4 py-2 focus:outline-none focus:border-theme-accent-primary"
            />
            <button 
              onClick={handleSend}
              disabled={!input.trim()}
              className="px-4 py-2 bg-theme-accent-primary text-theme-bg-primary font-bold rounded-md hover:bg-theme-accent-secondary transition-colors disabled:opacity-50"
            >
              Send
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
