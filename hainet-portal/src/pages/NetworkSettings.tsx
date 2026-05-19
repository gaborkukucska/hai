// <!-- # START OF FILE hainet-portal/src/pages/NetworkSettings.tsx -->
// Mesh Network & Settings page — wired to hainet-core for real node status,
// provider config persistence (settings.db), and mesh peer discovery.

import React, { useState, useEffect } from 'react';
import { invoke } from '../lib/tauri';

/** Node health check response */
interface NodeStatus {
  status: string;
  service: string;
  role: string;
  version: string;
}

/** Provider configuration loaded from settings.db */
interface ProviderConfig {
  ollama_url: string;
  openrouter_key: string;
}

export default function NetworkSettings() {
  // Node status (health check)
  const [nodeStatus, setNodeStatus] = useState<NodeStatus | null>(null);
  const [isChecking, setIsChecking] = useState(true);

  // Provider configuration — persisted to settings.db
  const [ollamaUrl, setOllamaUrl] = useState('http://127.0.0.1:11434');
  const [openrouterKey, setOpenrouterKey] = useState('');
  const [isSaving, setIsSaving] = useState(false);
  const [saveStatus, setSaveStatus] = useState<'idle' | 'saved' | 'error'>('idle');

  // Mesh peers from the gossip engine
  const [peerCount, setPeerCount] = useState(0);

  // --- Node health check (direct HTTP, not through invoke) ---
  useEffect(() => {
    const checkStatus = async () => {
      setIsChecking(true);
      try {
        const controller = new AbortController();
        const timeoutId = setTimeout(() => controller.abort(), 1500);

        const res = await fetch(`/health`, { signal: controller.signal });
        clearTimeout(timeoutId);

        if (res.ok) {
          const data = await res.json();
          setNodeStatus(data);
          console.debug('[NetworkSettings] Node health check passed');
        } else {
          setNodeStatus(null);
          console.debug('[NetworkSettings] No healthy node found');
        }
      } catch (e) {
        setNodeStatus(null);
        console.debug('[NetworkSettings] No healthy node found');
      }
      setIsChecking(false);
    };

    checkStatus();
    const interval = setInterval(checkStatus, 5000);
    return () => clearInterval(interval);
  }, []);

  // --- Load saved provider config from settings.db on mount ---
  useEffect(() => {
    const loadConfig = async () => {
      try {
        const config = await invoke<ProviderConfig>('get_provider_config');
        if (config) {
          setOllamaUrl(config.ollama_url || 'http://127.0.0.1:11434');
          setOpenrouterKey(config.openrouter_key || '');
          console.debug('[NetworkSettings] Provider config loaded from settings.db');
        }
      } catch (e: any) {
        console.debug('[NetworkSettings] Could not load provider config:', e.message);
      }
    };
    loadConfig();
  }, []);

  // --- Load peer count from gossip engine ---
  useEffect(() => {
    const loadPeers = async () => {
      try {
        const result = await invoke<{ peer_count: number }>('get_peer_count');
        setPeerCount(result?.peer_count || 0);
        console.debug('[NetworkSettings] Peer count:', result?.peer_count);
      } catch (e) {
        console.debug('[NetworkSettings] Could not fetch peer count');
      }
    };
    loadPeers();
    const interval = setInterval(loadPeers, 10000);
    return () => clearInterval(interval);
  }, []);

  /** Save provider config to settings.db via the backend */
  const handleSaveProviders = async () => {
    setIsSaving(true);
    setSaveStatus('idle');
    try {
      await invoke('save_provider_config', {
        ollama_url: ollamaUrl,
        openrouter_key: openrouterKey,
      });
      setSaveStatus('saved');
      console.debug('[NetworkSettings] Provider config saved to settings.db');
      // Reset the success indicator after 3 seconds
      setTimeout(() => setSaveStatus('idle'), 3000);
    } catch (e: any) {
      console.error('[NetworkSettings] Failed to save provider config:', e);
      setSaveStatus('error');
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <div className="flex-1 h-full overflow-y-auto bg-theme-bg-primary text-theme-text-primary p-6">
      <div className="max-w-4xl mx-auto space-y-8">

        <div>
          <h1 className="text-2xl font-bold">Mesh Network & Settings</h1>
          <p className="text-theme-text-muted text-sm mt-1">Configure your HAI-Net node, AI providers, and mesh peers.</p>
        </div>

        {/* === Local Node Status (health check) === */}
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
            <p className="text-sm text-theme-text-muted">Local core daemon is not reachable. Start it with <code className="bg-theme-bg-tertiary px-1 rounded">cargo run --package hainet-core</code></p>
          )}
        </section>

        {/* === Theme Settings === */}
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

        {/* === AI Provider Configuration (persisted to settings.db) === */}
        <section className="bg-theme-bg-secondary border border-theme-border rounded-xl p-5">
          <h2 className="text-lg font-semibold mb-4 border-b border-theme-border pb-2">AI Providers (TrippleEffect)</h2>
          <div className="space-y-4">
             <div className="grid grid-cols-2 gap-4">
               <div>
                 <label htmlFor="ollama-url-input" className="block text-sm font-medium text-theme-text-secondary mb-1">Local Ollama URL</label>
                 <input
                   type="text"
                   id="ollama-url-input"
                   value={ollamaUrl}
                   onChange={(e) => setOllamaUrl(e.target.value)}
                   className="w-full bg-theme-bg-tertiary border border-theme-border rounded-md px-3 py-2 text-theme-text-primary focus:outline-none focus:border-theme-accent-primary"
                 />
               </div>
               <div>
                 <label htmlFor="openrouter-key-input" className="block text-sm font-medium text-theme-text-secondary mb-1">OpenRouter API Key</label>
                 <input
                   type="password"
                   id="openrouter-key-input"
                   placeholder="sk-or-v1-..."
                   value={openrouterKey}
                   onChange={(e) => setOpenrouterKey(e.target.value)}
                   className="w-full bg-theme-bg-tertiary border border-theme-border rounded-md px-3 py-2 text-theme-text-primary focus:outline-none focus:border-theme-accent-primary"
                 />
               </div>
             </div>
             <button
               id="save-providers-btn"
               onClick={handleSaveProviders}
               disabled={isSaving}
               className={`px-4 py-2 text-sm rounded-md transition-colors ${
                 saveStatus === 'saved'
                   ? 'bg-theme-accent-success/20 text-theme-accent-success border border-theme-accent-success/30'
                   : saveStatus === 'error'
                   ? 'bg-theme-accent-danger/20 text-theme-accent-danger border border-theme-accent-danger/30'
                   : 'bg-theme-bg-tertiary hover:bg-theme-border text-theme-text-primary'
               } disabled:opacity-50`}
             >
               {isSaving ? 'Saving...' : saveStatus === 'saved' ? '✓ Saved!' : saveStatus === 'error' ? '✗ Error' : 'Save Providers'}
             </button>
          </div>
        </section>

        {/* === Mesh Peers (from gossip engine) === */}
        <section className="bg-theme-bg-secondary border border-theme-border rounded-xl p-5">
          <div className="flex justify-between items-center mb-4 border-b border-theme-border pb-2">
            <h2 className="text-lg font-semibold">Mesh Peers</h2>
            <div className="flex items-center gap-2">
              <span className="text-xs text-theme-text-muted">{peerCount} connected</span>
              <button className="text-sm text-theme-accent-primary hover:underline">Add Peer</button>
            </div>
          </div>
          <div className="space-y-2">
            {peerCount === 0 ? (
              <p className="text-sm text-theme-text-muted py-4 text-center">
                No peers connected yet. Deploy the mesh with <code className="bg-theme-bg-tertiary px-1 rounded">hainet-seed</code> to connect nodes.
              </p>
            ) : (
              <p className="text-sm text-theme-text-muted">
                {peerCount} peer(s) connected via gossip protocol. Full peer list coming in Phase 4.
              </p>
            )}
          </div>
        </section>

      </div>
    </div>
  );
}
