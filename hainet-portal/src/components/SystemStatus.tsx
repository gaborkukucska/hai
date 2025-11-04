// <!-- # START OF FILE hainet-portal/src/components/SystemStatus.tsx -->
import React, { useState, useEffect } from 'react';
import { invoke } from '../lib/tauri';

interface SystemStatusData {
  cpu_usage: number;
  memory_usage: number;
  total_memory: number;
  disk_usage: number;
  total_disk: number;
}

const formatBytes = (bytes: number) => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
}

const SystemStatus: React.FC = () => {
  const [status, setStatus] = useState<SystemStatusData | null>(null);

  useEffect(() => {
    const interval = setInterval(() => {
      invoke<SystemStatusData>('get_system_status').then(setStatus);
    }, 2000); // Update every 2 seconds

    return () => clearInterval(interval);
  }, []);

  return (
    <div className="p-4 bg-gray-800 text-white rounded-lg">
      <h2 className="text-xl font-bold mb-4">System Status</h2>
      {status ? (
        <div className="grid grid-cols-2 gap-4">
          <div>
            <p className="font-semibold">CPU Usage</p>
            <p>{status.cpu_usage.toFixed(2)}%</p>
          </div>
          <div>
            <p className="font-semibold">Memory Usage</p>
            <p>{formatBytes(status.memory_usage)} / {formatBytes(status.total_memory)}</p>
          </div>
          <div>
            <p className="font-semibold">Disk Usage</p>
            <p>{formatBytes(status.disk_usage)} / {formatBytes(status.total_disk)}</p>
          </div>
        </div>
      ) : (
        <p>Loading system status...</p>
      )}
    </div>
  );
};

export default SystemStatus;
// <!-- # END OF FILE hainet-portal/src/components/SystemStatus.tsx -->
