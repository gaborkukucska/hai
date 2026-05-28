import React, { useState, useEffect, useRef, useCallback } from 'react';
import { invoke } from '../lib/tauri';

export const LogOverlay: React.FC = () => {
  const [isOpen, setIsOpen] = useState(false);
  const [logs, setLogs] = useState<string>('');
  const [filter, setFilter] = useState<string>('ALL');
  const [loading, setLoading] = useState(false);
  const [copied, setCopied] = useState(false);
  const [logSource, setLogSource] = useState<string>('');
  const logsEndRef = useRef<HTMLDivElement>(null);

  // Drag state
  const [position, setPosition] = useState({ x: 0, y: 0 });
  const [isDragging, setIsDragging] = useState(false);
  const dragOffset = useRef({ x: 0, y: 0 });
  const overlayRef = useRef<HTMLDivElement>(null);

  // Initialize position to bottom-right on first open
  const [initialized, setInitialized] = useState(false);
  useEffect(() => {
    if (isOpen && !initialized) {
      setPosition({
        x: window.innerWidth - 820,
        y: window.innerHeight - 520,
      });
      setInitialized(true);
    }
  }, [isOpen, initialized]);

  const fetchLogs = async () => {
    setLoading(true);
    try {
      const response = await invoke<{ logs: string; source?: string }>('get_system_logs', { lines: 1000 });
      if (response && response.logs) {
        setLogs(response.logs);
        if (response.source) {
          setLogSource(response.source);
        }
      }
    } catch (e) {
      console.error("Failed to fetch logs:", e);
      setLogs("Failed to fetch logs. Check backend connectivity.");
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (isOpen) {
      fetchLogs();
      const interval = setInterval(fetchLogs, 5000);
      return () => clearInterval(interval);
    }
  }, [isOpen]);

  useEffect(() => {
    if (isOpen && logsEndRef.current) {
      logsEndRef.current.scrollIntoView({ behavior: 'smooth' });
    }
  }, [logs, isOpen, filter]);

  const handleCopy = () => {
    navigator.clipboard.writeText(filteredLogs.join('\n'));
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  // --- Drag handlers ---
  const handleMouseDown = useCallback((e: React.MouseEvent<HTMLDivElement>) => {
    if (overlayRef.current) {
      dragOffset.current = {
        x: e.clientX - position.x,
        y: e.clientY - position.y,
      };
      setIsDragging(true);
    }
  }, [position]);

  useEffect(() => {
    if (!isDragging) return;

    const handleMouseMove = (e: MouseEvent) => {
      setPosition({
        x: e.clientX - dragOffset.current.x,
        y: e.clientY - dragOffset.current.y,
      });
    };

    const handleMouseUp = () => {
      setIsDragging(false);
    };

    window.addEventListener('mousemove', handleMouseMove);
    window.addEventListener('mouseup', handleMouseUp);
    return () => {
      window.removeEventListener('mousemove', handleMouseMove);
      window.removeEventListener('mouseup', handleMouseUp);
    };
  }, [isDragging]);

  const filteredLogs = logs.split('\n').filter(line => {
    if (filter === 'ALL') return true;
    return line.includes(filter);
  });

  if (!isOpen) {
    return (
      <button 
        onClick={() => setIsOpen(true)}
        className="fixed bottom-4 right-4 bg-theme-bg-tertiary text-theme-text-primary px-4 py-2 rounded-lg shadow-lg border border-theme-border flex items-center gap-2 hover:bg-theme-bg-hover transition-colors z-50 opacity-60 hover:opacity-100"
        title="Open System Logs"
      >
        <span style={{ fontSize: '16px' }}>⌨</span>
        <span>Terminal Logs</span>
      </button>
    );
  }

  return (
    <div
      ref={overlayRef}
      className="fixed w-[800px] h-[500px] bg-[#1a1a2e] border border-[#3a3a5c] shadow-2xl rounded-lg flex flex-col z-50 overflow-hidden font-mono text-sm"
      style={{
        left: `${position.x}px`,
        top: `${position.y}px`,
        userSelect: isDragging ? 'none' : 'auto',
      }}
    >
      {/* Header — Draggable */}
      <div
        onMouseDown={handleMouseDown}
        className="flex items-center justify-between bg-[#16213e] px-3 py-2 border-b border-[#3a3a5c] select-none"
        style={{ cursor: isDragging ? 'grabbing' : 'grab' }}
      >
        <div className="flex items-center gap-3">
          {/* Traffic light dots */}
          <div className="flex gap-1.5">
            <div className="w-3 h-3 rounded-full bg-red-500 hover:bg-red-400 cursor-pointer" onClick={() => setIsOpen(false)} title="Close" />
            <div className="w-3 h-3 rounded-full bg-yellow-500 hover:bg-yellow-400" title="Minimize" />
            <div className="w-3 h-3 rounded-full bg-green-500 hover:bg-green-400" title="Maximize" />
          </div>
          <span className="text-gray-300 font-bold text-xs tracking-wider uppercase">System Logs</span>
          {logSource && <span className="text-gray-500 text-xs truncate max-w-[200px]" title={logSource}>({logSource.split('/').pop()})</span>}
        </div>
        <div className="flex items-center gap-2">
          <select 
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            onMouseDown={(e) => e.stopPropagation()}
            className="bg-[#1a1a2e] text-gray-300 border border-[#3a3a5c] rounded px-2 py-0.5 text-xs outline-none hover:border-blue-500 transition-colors"
          >
            <option value="ALL">All</option>
            <option value="INFO">INFO</option>
            <option value="WARN">WARN</option>
            <option value="ERROR">ERROR</option>
            <option value="DEBUG">DEBUG</option>
          </select>
          <button 
            onClick={fetchLogs}
            onMouseDown={(e) => e.stopPropagation()}
            className="text-gray-400 hover:text-blue-400 px-1.5 py-0.5 rounded hover:bg-[#1a1a2e] transition-colors text-xs"
            title="Refresh"
          >
            ↻
          </button>
          <button 
            onClick={handleCopy}
            onMouseDown={(e) => e.stopPropagation()}
            className={`px-1.5 py-0.5 rounded transition-colors text-xs ${copied ? 'text-green-400' : 'text-gray-400 hover:text-blue-400 hover:bg-[#1a1a2e]'}`}
            title="Copy Logs"
          >
            {copied ? '✓ Copied' : '📋 Copy'}
          </button>
          <button 
            onClick={() => setIsOpen(false)}
            onMouseDown={(e) => e.stopPropagation()}
            className="text-gray-400 hover:text-red-400 px-1.5 py-0.5 rounded hover:bg-[#1a1a2e] transition-colors text-xs"
            title="Minimize"
          >
            —
          </button>
        </div>
      </div>

      {/* Status bar */}
      <div className="bg-[#0f3460] px-3 py-1 text-xs text-gray-400 flex items-center justify-between border-b border-[#3a3a5c]">
        <span>{filteredLogs.length} lines {filter !== 'ALL' ? `(filtered: ${filter})` : ''}</span>
        <span>{loading ? '● Refreshing...' : '○ Auto-refresh: 5s'}</span>
      </div>

      {/* Logs Area */}
      <div className="flex-1 overflow-y-auto px-4 py-2 bg-[#1a1a2e] text-gray-300 leading-relaxed">
        {filteredLogs.length === 0 ? (
          <div className="text-gray-500 text-center mt-8">No log entries match the current filter.</div>
        ) : (
          filteredLogs.map((line, i) => {
            let colorClass = 'text-gray-400';
            if (line.includes('ERROR')) colorClass = 'text-red-400 font-semibold';
            else if (line.includes('WARN')) colorClass = 'text-yellow-300';
            else if (line.includes('DEBUG')) colorClass = 'text-gray-600';
            else if (line.includes('INFO')) colorClass = 'text-blue-300';

            return (
              <div key={i} className={`whitespace-pre-wrap break-words mb-0.5 ${colorClass}`}>
                {line}
              </div>
            );
          })
        )}
        <div ref={logsEndRef} />
      </div>
    </div>
  );
};
