// <!-- # START OF FILE hainet-portal/src/components/Sidebar.tsx -->
// Main navigation sidebar — wired to hainet-core for live peer count display.

import React, { useState, useEffect } from 'react';
import { NavLink } from 'react-router-dom';
import { MessageSquare, Rss, Layers, Cpu, Network, Settings, User } from 'lucide-react';
import { invoke } from '../lib/tauri';

export const Sidebar: React.FC = () => {
  // Live peer count from the gossip engine
  const [peerCount, setPeerCount] = useState<number>(0);

  // Poll peer count every 15 seconds for the sidebar status indicator
  useEffect(() => {
    const fetchPeerCount = async () => {
      try {
        const result = await invoke<{ peer_count: number }>('get_peer_count');
        setPeerCount(result?.peer_count || 0);
      } catch (e) {
        // Silently fail — sidebar should never block on backend errors
        console.debug('[Sidebar] Could not fetch peer count');
      }
    };

    fetchPeerCount();
    const interval = setInterval(fetchPeerCount, 15000);
    return () => clearInterval(interval);
  }, []);

  const navItems = [
    { name: 'Chat & Comms', path: '/chat', icon: MessageSquare },
    { name: 'Social Feed', path: '/feed', icon: Rss },
    { name: 'Agent Studio', path: '/studio', icon: Layers },
    { name: 'Compute Node', path: '/compute', icon: Cpu },
    { name: 'Mesh Network', path: '/network', icon: Network },
    { name: 'Settings', path: '/settings', icon: Settings },
  ];

  return (
    <aside className="w-64 h-full bg-theme-bg-secondary border-r border-theme-border flex flex-col transition-colors duration-200">
      <div className="p-4 border-b border-theme-border">
        <h1 className="text-2xl font-bold text-theme-accent-primary tracking-tight">HAI-Net</h1>
        <p className="text-xs text-theme-text-muted mt-1 uppercase tracking-wider font-semibold">Decentralized Mesh</p>
      </div>

      <nav className="flex-1 overflow-y-auto py-4">
        <ul className="space-y-1 px-3">
          {navItems.map((item) => (
            <li key={item.name}>
              <NavLink
                to={item.path}
                className={({ isActive }) =>
                  `flex items-center gap-3 px-3 py-2.5 rounded-md transition-colors duration-150 ${
                    isActive
                      ? 'bg-theme-bg-tertiary text-theme-accent-primary font-medium shadow-sm'
                      : 'text-theme-text-secondary hover:bg-theme-bg-tertiary/50 hover:text-theme-text-primary'
                  }`
                }
              >
                <item.icon size={18} className="shrink-0" />
                <span className="text-sm">{item.name}</span>
              </NavLink>
            </li>
          ))}
        </ul>
      </nav>

      {/* User / node status footer — shows real peer count */}
      <div className="p-4 border-t border-theme-border">
        <div className="flex items-center gap-3 px-2 py-2 rounded-md hover:bg-theme-bg-tertiary transition-colors cursor-pointer">
          <div className="w-8 h-8 rounded-full bg-theme-bg-tertiary flex items-center justify-center shrink-0 border border-theme-border">
            <User size={16} className="text-theme-text-muted" />
          </div>
          <div className="overflow-hidden flex-1">
            <p className="text-sm font-medium text-theme-text-primary truncate">User Node</p>
            <p className="text-xs text-theme-text-muted truncate">
              {peerCount > 0
                ? `Online (${peerCount} peer${peerCount !== 1 ? 's' : ''})`
                : 'Online (no peers)'
              }
            </p>
          </div>
          {/* Green dot for online status */}
          <span className="w-2 h-2 rounded-full bg-theme-accent-success animate-pulse shrink-0"></span>
        </div>
      </div>
    </aside>
  );
};
