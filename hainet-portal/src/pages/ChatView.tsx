// <!-- # START OF FILE hainet-portal/src/pages/ChatView.tsx -->
// Chat & Comms page — wired to hainet-core Admin AI bridge via invoke().
// Loads conversation history on mount and sends messages through the real backend.

import React, { useState, useEffect, useRef } from 'react';
import { invoke } from '../lib/tauri';

/** Shape of a single chat message */
interface ChatMessage {
  id: number;
  role: 'user' | 'assistant' | 'system';
  content: string;
}

export default function ChatView() {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  // Auto-scroll to bottom when messages change
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

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
      const assistantContent = typeof response?.message === 'string'
        ? response.message
        : response?.message?.content
        || response?.content
        || (typeof response === 'string' ? response : JSON.stringify(response));

      const assistantMsg: ChatMessage = {
        id: Date.now() + 1,
        role: 'assistant',
        content: assistantContent,
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
             <button className="text-xs bg-theme-bg-tertiary px-2 py-1 rounded text-theme-text-secondary hover:text-theme-text-primary">All</button>
             <button className="text-xs bg-theme-bg-tertiary px-2 py-1 rounded text-theme-text-secondary hover:text-theme-text-primary">Agents</button>
             <button className="text-xs bg-theme-bg-tertiary px-2 py-1 rounded text-theme-text-secondary hover:text-theme-text-primary">Peers</button>
          </div>
        </div>
        <div className="flex-1 overflow-y-auto p-2 space-y-1">
          <div className="p-3 rounded-md bg-theme-bg-tertiary/50 cursor-pointer border-l-2 border-theme-accent-primary">
            <h3 className="text-sm font-semibold">Admin AI</h3>
            <p className="text-xs text-theme-text-muted truncate mt-1">
              {messages.length > 0 ? messages[messages.length - 1].content.slice(0, 50) + '...' : 'Ready for the next task.'}
            </p>
          </div>
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
                  <p className="text-sm whitespace-pre-wrap">{msg.content}</p>
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
