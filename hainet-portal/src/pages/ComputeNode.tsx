// <!-- # START OF FILE hainet-portal/src/pages/ComputeNode.tsx -->
// Compute Router page — wired to hainet-collab hardware profiler via invoke().
// Displays real hardware stats (CPU, RAM, GPU, capability score) from the local node.

import React, { useState, useEffect } from 'react';
import { invoke } from '../lib/tauri';

/** Hardware profile returned by the backend */
interface HardwareProfile {
  cpu_cores: number;
  cpu_model: string;
  ram_total_gb: number;
  ram_available_gb: number;
  gpu: {
    name: string;
    vram_mb: number;
    cuda_version?: string;
    driver_version?: string;
    temperature_c?: number;
    utilization_pct?: number;
  } | null;
  disk_total_gb: number;
  disk_available_gb: number;
  os: string;
  arch: string;
  capability_score: number;
}

export default function ComputeNode() {
  const [isContributing, setIsContributing] = useState(true);
  const [hardware, setHardware] = useState<HardwareProfile | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Fetch the real hardware profile from hainet-collab on mount
  useEffect(() => {
    const fetchProfile = async () => {
      try {
        const profile = await invoke<HardwareProfile>('get_hardware_profile');
        setHardware(profile);
        setError(null);
        console.debug('[ComputeNode] Hardware profile loaded:', profile.cpu_model, profile.cpu_cores, 'cores');
      } catch (e: any) {
        console.error('[ComputeNode] Failed to load hardware profile:', e);
        setError('Could not detect hardware. Is the daemon running?');
      } finally {
        setIsLoading(false);
      }
    };
    fetchProfile();
  }, []);

  /** Refresh the hardware profile (re-detect) */
  const handleRefresh = async () => {
    setIsLoading(true);
    try {
      await invoke('refresh_hardware_profile');
      const profile = await invoke<HardwareProfile>('get_hardware_profile');
      setHardware(profile);
      console.debug('[ComputeNode] Hardware profile refreshed');
    } catch (e: any) {
      console.error('[ComputeNode] Refresh failed:', e);
    } finally {
      setIsLoading(false);
    }
  };

  /** Format GB values to 1 decimal place */
  const fmtGb = (gb: number) => gb.toFixed(1);

  return (
    <div className="flex-1 h-full overflow-y-auto bg-theme-bg-primary text-theme-text-primary p-6">
      <div className="max-w-5xl mx-auto space-y-6">

        <div className="flex justify-between items-center mb-8">
          <div>
            <h1 className="text-2xl font-bold">Compute Router</h1>
            <p className="text-theme-text-muted text-sm mt-1">Resource sharing and hardware profiling (Powered by pplpwr)</p>
          </div>
          <div className="flex items-center gap-3">
            <button
              onClick={handleRefresh}
              disabled={isLoading}
              className="text-xs px-3 py-1.5 bg-theme-bg-tertiary rounded hover:bg-theme-border transition-colors disabled:opacity-50"
            >
              {isLoading ? 'Scanning...' : '🔄 Refresh'}
            </button>
            <span className="text-sm text-theme-text-muted">Contribution:</span>
            <button
              id="contribution-toggle"
              onClick={() => setIsContributing(!isContributing)}
              className={`w-12 h-6 rounded-full relative cursor-pointer transition-colors ${isContributing ? 'bg-theme-accent-success' : 'bg-theme-bg-tertiary'}`}>
               <div className={`w-4 h-4 rounded-full bg-white absolute top-1 transition-all ${isContributing ? 'right-1' : 'left-1'}`}></div>
            </button>
          </div>
        </div>

        {/* Error state */}
        {error && (
          <div className="bg-theme-accent-danger/10 border border-theme-accent-danger/30 text-theme-accent-danger px-4 py-3 rounded-lg text-sm">
            {error}
          </div>
        )}

        {/* Hardware Profile — wired to real data from hainet-collab */}
        <div className="bg-theme-bg-secondary border border-theme-border rounded-xl p-5 mb-6">
          <div className="flex justify-between items-center mb-4">
            <h2 className="text-lg font-semibold">Local Hardware Profile</h2>
            {hardware && (
              <span className="text-xs px-2 py-1 bg-theme-accent-primary/20 text-theme-accent-primary rounded-full">
                Score: {hardware.capability_score.toFixed(1)}
              </span>
            )}
          </div>

          {isLoading && !hardware ? (
            <p className="text-sm text-theme-text-muted animate-pulse">Detecting hardware...</p>
          ) : hardware ? (
            <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
               {/* CPU */}
               <div className="bg-theme-bg-tertiary rounded-lg p-4">
                  <p className="text-xs text-theme-text-muted uppercase tracking-wider">CPU</p>
                  <p className="text-lg font-medium mt-1">{hardware.cpu_cores} Cores</p>
                  <p className="text-xs text-theme-text-secondary mt-1 truncate" title={hardware.cpu_model}>
                    {hardware.cpu_model}
                  </p>
               </div>

               {/* GPU */}
               <div className={`bg-theme-bg-tertiary rounded-lg p-4 ${hardware.gpu ? 'border border-theme-accent-primary/50' : ''}`}>
                  <p className="text-xs text-theme-text-muted uppercase tracking-wider">
                    GPU {hardware.gpu ? '(Primary Compute)' : ''}
                  </p>
                  {hardware.gpu ? (
                    <>
                      <p className="text-lg font-medium mt-1">{hardware.gpu.name}</p>
                      <p className="text-xs mt-1">
                        VRAM: {(hardware.gpu.vram_mb / 1024).toFixed(0)} GB
                        {hardware.gpu.utilization_pct !== undefined && hardware.gpu.utilization_pct !== null && (
                          <span className="ml-2 text-theme-accent-success">
                            ({hardware.gpu.utilization_pct.toFixed(0)}% Usage)
                          </span>
                        )}
                      </p>
                      {hardware.gpu.temperature_c !== undefined && hardware.gpu.temperature_c !== null && (
                        <p className="text-xs text-theme-text-muted mt-1">
                          🌡️ {hardware.gpu.temperature_c.toFixed(0)}°C
                        </p>
                      )}
                    </>
                  ) : (
                    <>
                      <p className="text-lg font-medium mt-1">None Detected</p>
                      <p className="text-xs text-theme-text-muted mt-1">CPU-only compute</p>
                    </>
                  )}
               </div>

               {/* RAM */}
               <div className="bg-theme-bg-tertiary rounded-lg p-4">
                  <p className="text-xs text-theme-text-muted uppercase tracking-wider">RAM</p>
                  <p className="text-lg font-medium mt-1">{fmtGb(hardware.ram_total_gb)} GB Total</p>
                  <p className="text-xs text-theme-text-secondary mt-1">
                    {fmtGb(hardware.ram_available_gb)} GB Available
                  </p>
               </div>
            </div>
          ) : null}

          {/* Disk & OS info */}
          {hardware && (
            <div className="grid grid-cols-2 gap-4 mt-4">
              <div className="bg-theme-bg-tertiary rounded-lg p-3">
                <p className="text-xs text-theme-text-muted uppercase tracking-wider">Disk</p>
                <p className="text-sm font-medium mt-1">
                  {fmtGb(hardware.disk_available_gb)} GB free / {fmtGb(hardware.disk_total_gb)} GB total
                </p>
              </div>
              <div className="bg-theme-bg-tertiary rounded-lg p-3">
                <p className="text-xs text-theme-text-muted uppercase tracking-wider">OS / Arch</p>
                <p className="text-sm font-medium mt-1">{hardware.os} ({hardware.arch})</p>
              </div>
            </div>
          )}
        </div>

        {/* Compute Networks */}
        <h2 className="text-lg font-semibold mb-4 mt-8">Active Mesh Networks</h2>
        <div className="space-y-3">
           <div className="bg-theme-bg-secondary border border-theme-border rounded-lg p-4 flex items-center justify-between">
              <div>
                 <h3 className="font-medium text-theme-text-primary">HAI-Net Mesh</h3>
                 <p className="text-xs text-theme-text-muted">Local compute sharing with mesh peers</p>
              </div>
              <div className="text-right">
                 <p className="text-sm font-medium">Score: {hardware?.capability_score.toFixed(1) || '—'}</p>
                 <p className={`text-xs ${isContributing ? 'text-theme-accent-success' : 'text-theme-text-muted'}`}>
                   {isContributing ? 'Contributing' : 'Paused'}
                 </p>
              </div>
           </div>
        </div>

      </div>
    </div>
  );
}
