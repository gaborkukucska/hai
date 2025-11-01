//! # START OF FILE hainet-portal/src/components/MetricsToolbar.tsx
import React from 'react';
import { Download, Calendar } from 'lucide-react';
import { TrendInterval } from '../types';

interface MetricsToolbarProps {
  onExport: (format: 'csv' | 'json') => void;
  onIntervalChange: (interval: TrendInterval) => void;
  selectedInterval: TrendInterval;
  isExporting: boolean;
}

export default function MetricsToolbar({ onExport, onIntervalChange, selectedInterval, isExporting }: MetricsToolbarProps) {
  const intervals: { label: string; value: TrendInterval }[] = [
    { label: 'Hourly', value: 'Hourly' },
    { label: 'Daily', value: 'Daily' },
    { label: 'Weekly', value: 'Weekly' },
  ];

  return (
    <div className="bg-gray-800 rounded-lg p-4 flex items-center justify-between">
      <div className="flex items-center space-x-4">
        <div className="flex items-center text-gray-400">
          <Calendar className="w-5 h-5 mr-2" />
          <span className="font-medium text-white">Trend Interval:</span>
        </div>
        <div className="flex items-center bg-gray-700 rounded-md">
          {intervals.map(({ label, value }) => (
            <button
              key={value}
              onClick={() => onIntervalChange(value)}
              className={`px-3 py-1.5 text-sm font-medium rounded-md transition ${
                selectedInterval === value
                  ? 'bg-blue-600 text-white'
                  : 'text-gray-300 hover:bg-gray-600'
              }`}
            >
              {label}
            </button>
          ))}
        </div>
      </div>
      <div className="flex items-center space-x-2">
        <button
          onClick={() => onExport('csv')}
          disabled={isExporting}
          className="px-3 py-1.5 bg-green-600 text-white rounded-md hover:bg-green-700 transition flex items-center disabled:opacity-50"
        >
          <Download className="w-4 h-4 mr-2" />
          Export CSV
        </button>
        <button
          onClick={() => onExport('json')}
          disabled={isExporting}
          className="px-3 py-1.5 bg-gray-600 text-white rounded-md hover:bg-gray-500 transition flex items-center disabled:opacity-50"
        >
          <Download className="w-4 h-4 mr-2" />
          Export JSON
        </button>
      </div>
    </div>
  );
}
