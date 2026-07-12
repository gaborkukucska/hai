// <!-- # START OF FILE hainet-portal/src/pages/ChatView.tsx -->
// Chat & Comms page — wired to hainet-core Admin AI bridge via invoke().
// Loads conversation history on mount and sends messages through the real backend.

import React, { useState, useEffect, useRef } from 'react';
import { invoke } from '../lib/tauri';

interface DynamicComponent {
  type: string;
  props?: any;
  children?: any[];
  action?: any;
}

/** Shape of a single chat message */
interface ChatMessage {
  id: number;
  role: 'user' | 'assistant' | 'system';
  content: string;
  dynamicComponent?: DynamicComponent;
}

const DynamicRenderer = ({ comp }: { comp: DynamicComponent | string }) => {
  if (typeof comp === 'string') return <span>{comp}</span>;
  if (!comp || !comp.type) return null;
  
  const children = comp.children?.map((c, i) => (
    <React.Fragment key={i}>
      <DynamicRenderer comp={c} />
    </React.Fragment>
  ));

  const props = comp.props || {};

  if (comp.type === 'Button') {
    return (
      <button {...props} onClick={() => {
        if (comp.action?.type === 'invoke' && comp.action?.payload?.command) {
            invoke(comp.action.payload.command, comp.action.payload.args || {}).then(console.log).catch(console.error);
        }
      }}>
        {children}
      </button>
    );
  }

  if (comp.type === 'Stack') {
    return <div className="flex flex-col gap-2" {...props}>{children}</div>;
  }
  
  if (comp.type === 'Text') {
    return <span {...props}>{children}</span>;
  }
  
  return <div {...props}>{children}</div>;
};

export default function ChatView() {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [peers, setPeers] = useState<any[]>([]);
  const [activeTab, setActiveTab] = useState<'All' | 'Agents' | 'Peers'>('All');
  const messagesEndRef = useRef<HTMLDivElement>(null);

  // Auto-scroll to bottom when messages change
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  useEffect(() => {
    const fetchPeers = async () => {
      try {
        const res = await invoke<{peers: any[]}>('get_mesh_peers');
        if (res?.peers) setPeers(res.peers);
      } catch (e) {}
    };
    fetchPeers();
    const interval = setInterval(fetchPeers, 5000);
    return () => clearInterval(interval);
  }, []);

  // Load conversation history from backend on mount
  useEffect(() => {
    const loadHistory = async () => {
      try {
        const history = await invoke<any>('get_history');
        if (history && Array.isArray(history)) {
          // Map backend history format to our ChatMessage format
          const mapped: ChatMessage[] = history.map((msg: any, idx: number) => ({
            id: idx,
            role: msg.role || 'assistant',
            content: msg.content || '',
          }));
          setMessages(mapped);
          console.debug('[ChatView] Loaded', mapped.length, 'messages from history');
        }
      } catch (e) {
        // No history yet — show welcome message
        console.debug('[ChatView] No history found, showing welcome message');
        setMessages([{
          id: 1,
          role: 'assistant',
          content: 'Hello! I am your HAI-Net Admin AI. How can I help you today?'
        }]);
      }
    };
    loadHistory();
  }, []);

  /** Send a message to the Admin AI via the backend bridge */
  const handleSend = async () => {
    if (!input.trim() || isLoading) return;

    const userMsg: ChatMessage = { id: Date.now(), role: 'user', content: input };
    setMessages(prev => [...prev, userMsg]);
    setInput('');
    setIsLoading(true);
    setError(null);

    try {
      // Call the real send_message endpoint on hainet-core
      const response = await invoke<any>('send_message', {
        content: input,
        attachments: [],
      });

      // Parse the response — the backend returns the assistant's reply
      let assistantContent = "";
      let dynamicComponent = undefined;

      if (typeof response?.message === 'string') {
        assistantContent = response.message;
      } else if (response?.message) {
        assistantContent = response.message.content !== undefined ? response.message.content : "";
        dynamicComponent = response.message.dynamic_component;
      } else if (response?.content) {
        assistantContent = response.content;
      } else if (typeof response === 'string') {
        assistantContent = response;
      } else {
        assistantContent = JSON.stringify(response);
      }

      const assistantMsg: ChatMessage = {
        id: Date.now() + 1,
        role: 'assistant',
        content: assistantContent,
        dynamicComponent,
      };

      setMessages(prev => [...prev, assistantMsg]);
      console.debug('[ChatView] Received response from Admin AI');
    } catch (e: any) {
      console.error('[ChatView] Error sending message:', e);
      setError(e.message || 'Failed to reach Admin AI');

      // Show error as a system message so the user knows what happened
      setMessages(prev => [...prev, {
        id: Date.now() + 1,
        role: 'assistant',
        content: `⚠️ Error: ${e.message || 'Failed to reach the Admin AI backend. Is hainet-persona running?'}`,
      }]);
    } finally {
      setIsLoading(false);
    }
  };

  /** Clear the chat history */
  const handleClear = async () => {
    try {
      await invoke('clear_history');
      setMessages([{
        id: Date.now(),
        role: 'assistant',
        content: 'Chat history cleared. How can I help you?'
      }]);
      console.debug('[ChatView] History cleared');
    } catch (e) {
      console.error('[ChatView] Failed to clear history:', e);
    }
  };

  return (
    <div className="flex h-full text-theme-text-primary bg-theme-bg-primary">
      {/* Left Sidebar: Threads */}
      <div className="w-80 border-r border-theme-border bg-theme-bg-secondary flex flex-col">
        <div className="p-4 border-b border-theme-border">
          <h2 className="text-lg font-bold text-theme-text-primary">Chat & Comms</h2>
          <div className="mt-2 flex gap-2">
             <button onClick={() => setActiveTab('All')} className={`text-xs px-2 py-1 rounded transition-colors ${activeTab === 'All' ? 'bg-theme-accent-primary text-theme-bg-primary' : 'bg-theme-bg-tertiary text-theme-text-secondary hover:text-theme-text-primary'}`}>All</button>
             <button onClick={() => setActiveTab('Agents')} className={`text-xs px-2 py-1 rounded transition-colors ${activeTab === 'Agents' ? 'bg-theme-accent-primary text-theme-bg-primary' : 'bg-theme-bg-tertiary text-theme-text-secondary hover:text-theme-text-primary'}`}>Agents</button>
             <button onClick={() => setActiveTab('Peers')} className={`text-xs px-2 py-1 rounded transition-colors ${activeTab === 'Peers' ? 'bg-theme-accent-primary text-theme-bg-primary' : 'bg-theme-bg-tertiary text-theme-text-secondary hover:text-theme-text-primary'}`}>Peers</button>
          </div>
        </div>
        <div className="flex-1 overflow-y-auto p-2 space-y-1">
          {(activeTab === 'All' || activeTab === 'Agents') && (
            <div className="p-3 rounded-md bg-theme-bg-tertiary/50 cursor-pointer border-l-2 border-theme-accent-primary">
              <h3 className="text-sm font-semibold">Admin AI</h3>
              <p className="text-xs text-theme-text-muted truncate mt-1">
                {messages.length > 0 ? messages[messages.length - 1].content.slice(0, 50) + '...' : 'Ready for the next task.'}
              </p>
            </div>
          )}
          {(activeTab === 'All' || activeTab === 'Peers') && peers.map((p, i) => (
            <div key={i} className="p-3 rounded-md hover:bg-theme-bg-tertiary/30 cursor-pointer border-l-2 border-transparent hover:border-theme-border transition-colors">
              <h3 className="text-sm font-semibold">{p.handle || "Unknown Peer"}</h3>
              <p className="text-xs text-theme-text-muted truncate mt-1">
                {p.public_key ? p.public_key.slice(0, 16) + '...' : ''}
              </p>
            </div>
          ))}
        </div>
      </div>

      {/* Main Chat Area */}
      <div className="flex-1 flex flex-col">
        <div className="p-4 border-b border-theme-border flex items-center justify-between bg-theme-bg-secondary">
          <h2 className="text-lg font-semibold">Admin AI</h2>
          <div className="flex items-center gap-2">
            <button
              onClick={handleClear}
              className="text-xs px-2 py-1 bg-theme-bg-tertiary rounded hover:bg-theme-border transition-colors"
            >
              Clear History
            </button>
            <span className={`text-xs px-2 py-1 rounded-full border ${
              isLoading
                ? 'bg-yellow-500/20 text-yellow-400 border-yellow-500/30'
                : 'bg-theme-accent-success/20 text-theme-accent-success border-theme-accent-success/30'
            }`}>
              {isLoading ? 'Thinking...' : 'Online'}
            </span>
          </div>
        </div>

        {/* Messages area */}
        <div className="flex-1 overflow-y-auto p-4 space-y-4 flex flex-col">
          {messages.map(msg => (
            <div key={msg.id} className={`flex items-start gap-3 ${msg.role === 'user' ? 'flex-row-reverse' : ''}`}>
               <div className={`w-8 h-8 rounded-full shrink-0 flex items-center justify-center text-xs font-bold ${
                 msg.role === 'user'
                   ? 'bg-theme-bg-tertiary text-theme-text-secondary'
                   : 'bg-theme-accent-primary text-theme-bg-primary'
               }`}>
                 {msg.role === 'user' ? 'U' : 'AI'}
               </div>
               <div className={`bg-theme-bg-secondary border border-theme-border p-3 rounded-2xl max-w-[80%] ${
                 msg.role === 'user'
                   ? 'rounded-tr-none bg-theme-bg-tertiary'
                   : 'rounded-tl-none'
               }`}>
                  {msg.content && <p className="text-sm whitespace-pre-wrap">{msg.content}</p>}
                  {msg.dynamicComponent && (
                    <div className="mt-2">
                      <DynamicRenderer comp={msg.dynamicComponent} />
                    </div>
                  )}
               </div>
            </div>
          ))}

          {/* Loading indicator */}
          {isLoading && (
            <div className="flex items-start gap-3">
              <div className="w-8 h-8 rounded-full shrink-0 flex items-center justify-center text-xs font-bold bg-theme-accent-primary text-theme-bg-primary">
                AI
              </div>
              <div className="bg-theme-bg-secondary border border-theme-border p-3 rounded-2xl rounded-tl-none">
                <div className="flex gap-1">
                  <span className="w-2 h-2 bg-theme-text-muted rounded-full animate-bounce" style={{animationDelay: '0ms'}}></span>
                  <span className="w-2 h-2 bg-theme-text-muted rounded-full animate-bounce" style={{animationDelay: '150ms'}}></span>
                  <span className="w-2 h-2 bg-theme-text-muted rounded-full animate-bounce" style={{animationDelay: '300ms'}}></span>
                </div>
              </div>
            </div>
          )}

          <div ref={messagesEndRef} />
        </div>

        {/* Input area */}
        <div className="p-4 border-t border-theme-border bg-theme-bg-secondary">
          {error && (
            <div className="mb-2 text-xs text-theme-accent-danger bg-theme-accent-danger/10 px-3 py-1.5 rounded">
              {error}
            </div>
          )}
          <div className="flex gap-2">
            <button className="p-2 text-theme-text-muted hover:text-theme-text-primary bg-theme-bg-tertiary rounded-md">📎</button>
            <input
              type="text"
              id="chat-message-input"
              placeholder="Message Admin AI..."
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && handleSend()}
              disabled={isLoading}
              className="flex-1 bg-theme-bg-tertiary border border-theme-border rounded-md px-4 py-2 focus:outline-none focus:border-theme-accent-primary disabled:opacity-50"
            />
            <button
              id="chat-send-button"
              onClick={handleSend}
              disabled={!input.trim() || isLoading}
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
