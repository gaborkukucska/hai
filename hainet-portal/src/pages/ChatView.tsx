// <!-- # START OF FILE hainet-portal/src/pages/ChatView.tsx -->
// Chat & Comms page — wired to hainet-core Admin AI bridge via invoke().
// Loads conversation history on mount and syncs DMs from mobile.

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
  id: string | number;
  role: 'user' | 'assistant' | 'system';
  content: string;
  dynamicComponent?: DynamicComponent;
}

interface PeerDM {
  id: string;
  peer: string;
  sender: string;
  content: string;
  timestamp: number;
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
  const [adminMessages, setAdminMessages] = useState<ChatMessage[]>([]);
  const [dms, setDms] = useState<PeerDM[]>([]);
  const [input, setInput] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [peers, setPeers] = useState<any[]>([]);
  
  const [activeTab, setActiveTab] = useState<'All' | 'Agents' | 'Peers'>('All');
  const [selectedPeerId, setSelectedPeerId] = useState<string>('AdminAI');
  
  const messagesEndRef = useRef<HTMLDivElement>(null);

  // Auto-scroll to bottom when messages change
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [adminMessages, dms, selectedPeerId]);

  // Polling loop for Peers and DMs
  useEffect(() => {
    const fetchSyncData = async () => {
      try {
        const peerRes = await invoke<{peers: any[]}>('get_mesh_peers');
        if (peerRes?.peers) {
          // Filter out Admin AI from the generic synced peers list to avoid duplicates
          setPeers(peerRes.peers.filter((p: any) => p.handle !== "Admin AI"));
        }
        
        const dmRes = await invoke<{dms: PeerDM[]}>('get_dms');
        if (dmRes?.dms) {
          setDms(dmRes.dms);
        }
      } catch (e) {}
    };
    fetchSyncData();
    const interval = setInterval(fetchSyncData, 5000);
    return () => clearInterval(interval);
  }, []);

  // Load Admin AI conversation history from backend on mount
  useEffect(() => {
    const loadHistory = async () => {
      try {
        const history = await invoke<any>('get_history');
        if (history && Array.isArray(history)) {
          const mapped: ChatMessage[] = history.map((msg: any, idx: number) => ({
            id: idx,
            role: msg.role || 'assistant',
            content: msg.content || '',
          }));
          setAdminMessages(mapped);
        }
      } catch (e) {
        setAdminMessages([{
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
    if (!input.trim() || isLoading || selectedPeerId !== 'AdminAI') return;

    const userMsg: ChatMessage = { id: Date.now(), role: 'user', content: input };
    setAdminMessages(prev => [...prev, userMsg]);
    setInput('');
    setIsLoading(true);
    setError(null);

    try {
      const response = await invoke<any>('send_message', {
        content: input,
        attachments: [],
      });

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

      setAdminMessages(prev => [...prev, assistantMsg]);
    } catch (e: any) {
      console.error('[ChatView] Error sending message:', e);
      setError(e.message || 'Failed to reach Admin AI');
      setAdminMessages(prev => [...prev, {
        id: Date.now() + 1,
        role: 'assistant',
        content: `⚠️ Error: ${e.message || 'Failed to reach the Admin AI backend. Is hainet-persona running?'}`,
      }]);
    } finally {
      setIsLoading(false);
    }
  };

  /** Clear the Admin AI chat history */
  const handleClear = async () => {
    if (selectedPeerId !== 'AdminAI') return;
    try {
      await invoke('clear_history');
      setAdminMessages([{
        id: Date.now(),
        role: 'assistant',
        content: 'Chat history cleared. How can I help you?'
      }]);
    } catch (e) {
      console.error('[ChatView] Failed to clear history:', e);
    }
  };

  // Resolve current view context
  const isPeerChat = selectedPeerId !== 'AdminAI';
  const currentPeerName = isPeerChat 
    ? peers.find(p => p.public_key === selectedPeerId)?.handle || "Unknown Peer"
    : "Admin AI";

  // Compute messages to display based on selected context
  const displayMessages: ChatMessage[] = isPeerChat
    ? dms.filter(dm => dm.peer === selectedPeerId)
         .sort((a, b) => Number(a.timestamp) - Number(b.timestamp))
         .map(dm => ({
           id: dm.id,
           // If the sender is the peer, they are the "assistant" (left side)
           // If the sender is NOT the peer, it must be the local mobile user (right side)
           role: dm.sender === selectedPeerId ? 'assistant' : 'user',
           content: dm.content
         }))
    : adminMessages;

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
          {/* Admin AI Thread */}
          {(activeTab === 'All' || activeTab === 'Agents') && (
            <div 
              onClick={() => setSelectedPeerId('AdminAI')}
              className={`p-3 rounded-md cursor-pointer border-l-2 transition-colors ${selectedPeerId === 'AdminAI' ? 'bg-theme-bg-tertiary/50 border-theme-accent-primary' : 'hover:bg-theme-bg-tertiary/30 border-transparent hover:border-theme-border'}`}
            >
              <h3 className="text-sm font-semibold">Admin AI</h3>
              <p className="text-xs text-theme-text-muted truncate mt-1">
                {adminMessages.length > 0 ? adminMessages[adminMessages.length - 1].content.slice(0, 50) + '...' : 'Ready for the next task.'}
              </p>
            </div>
          )}
          
          {/* Synced Mobile Peers Threads */}
          {(activeTab === 'All' || activeTab === 'Peers') && peers.map((p, i) => {
            const peerDMs = dms.filter(dm => dm.peer === p.public_key);
            // Get last message content or fallback
            const lastDm = peerDMs.length > 0 
                ? peerDMs.reduce((prev, curr) => (Number(curr.timestamp) > Number(prev.timestamp) ? curr : prev)).content 
                : 'No messages yet';
                
            return (
              <div 
                key={i} 
                onClick={() => setSelectedPeerId(p.public_key)}
                className={`p-3 rounded-md cursor-pointer border-l-2 transition-colors ${selectedPeerId === p.public_key ? 'bg-theme-bg-tertiary/50 border-theme-accent-primary' : 'hover:bg-theme-bg-tertiary/30 border-transparent hover:border-theme-border'}`}
              >
                <h3 className="text-sm font-semibold">{p.handle || "Unknown Peer"}</h3>
                <p className="text-xs text-theme-text-muted truncate mt-1">
                  {lastDm.slice(0, 50) + (lastDm.length > 50 ? '...' : '')}
                </p>
              </div>
            );
          })}
        </div>
      </div>

      {/* Main Chat Area */}
      <div className="flex-1 flex flex-col">
        <div className="p-4 border-b border-theme-border flex items-center justify-between bg-theme-bg-secondary">
          <h2 className="text-lg font-semibold">{currentPeerName}</h2>
          <div className="flex items-center gap-2">
            {!isPeerChat && (
              <button
                onClick={handleClear}
                className="text-xs px-2 py-1 bg-theme-bg-tertiary rounded hover:bg-theme-border transition-colors"
              >
                Clear History
              </button>
            )}
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
          {displayMessages.length === 0 && isPeerChat && (
            <div className="text-center py-8 text-theme-text-muted text-sm">
              No synced messages found for this contact.
            </div>
          )}

          {displayMessages.map(msg => (
            <div key={msg.id} className={`flex items-start gap-3 ${msg.role === 'user' ? 'flex-row-reverse' : ''}`}>
               <div className={`w-8 h-8 rounded-full shrink-0 flex items-center justify-center text-xs font-bold ${
                 msg.role === 'user'
                   ? 'bg-theme-bg-tertiary text-theme-text-secondary'
                   : 'bg-theme-accent-primary text-theme-bg-primary'
               }`}>
                 {msg.role === 'user' ? 'U' : currentPeerName.charAt(0).toUpperCase()}
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
              placeholder={isPeerChat ? "Read-only mode (Reply via mobile app)" : "Message Admin AI..."}
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && handleSend()}
              disabled={isLoading || isPeerChat}
              className="flex-1 bg-theme-bg-tertiary border border-theme-border rounded-md px-4 py-2 focus:outline-none focus:border-theme-accent-primary disabled:opacity-50"
            />
            <button
              id="chat-send-button"
              onClick={handleSend}
              disabled={!input.trim() || isLoading || isPeerChat}
              className="px-4 py-2 bg-theme-accent-primary text-theme-bg-primary font-bold rounded-md hover:bg-theme-accent-secondary transition-colors disabled:opacity-50 disabled:bg-theme-bg-tertiary disabled:text-theme-text-muted"
            >
              Send
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
