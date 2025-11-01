//! # START OF FILE hainet-portal/src/pages/MetricsDashboard.tsx
import React, { useState, useEffect, useRef } from 'react';
import { formatDistanceToNow } from 'date-fns';
import { RefreshCw, TrendingUp, DollarSign, Cpu, Activity, Download } from 'lucide-react';
import { LineChart, Line, XAxis, YAxis, CartesianGrid, Tooltip, Legend, ResponsiveContainer } from 'recharts';
import { useMetrics } from '../hooks/useMetrics';
import { AgentMetrics } from '../types';
import { invoke } from '@tauri-apps/api/core';
import { save } from '@tauri-apps/api/dialog';
import { writeTextFile } from '@tauri-apps/api/fs';


export default function MetricsDashboard() {
  const { metrics, loading, error, refetch } = useMetrics();
  const scrollableRef = useRef<HTMLDivElement>(null);
  const [isUserScrolling, setIsUserScrolling] = useState(false);
  const scrollTimeoutRef = useRef<NodeJS.Timeout>();
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [isExporting, setIsExporting] = useState(false);
  const [exportError, setExportError] = useState<string | null>(null);

  const handleExport = async (format: 'csv' | 'json') => {
    setIsExporting(true);
    setExportError(null);
    try {
      const command = `export_metrics_${format}`;
      const content = await invoke<string>(command);

      const filePath = await save({
        defaultPath: `hainet-metrics-export-${new Date().toISOString().split('T')[0]}.${format}`,
        filters: [{
          name: format.toUpperCase(),
          extensions: [format],
        }],
      });

      if (filePath) {
        await writeTextFile(filePath, content);
      }
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : `Failed to export as ${format}`;
      console.error(errorMessage, err);
      setExportError(errorMessage);
      // Hide error after 5 seconds
      setTimeout(() => setExportError(null), 5000);
    } finally {
      setIsExporting(false);
    }
  };

  // Auto-scroll to bottom when new data arrives
  useEffect(() => {
    if (!isUserScrolling && scrollableRef.current && metrics) {
      scrollableRef.current.scrollTo({
        top: scrollableRef.current.scrollHeight,
        behavior: 'smooth'
      });
    }
  }, [metrics, isUserScrolling]);

  // Detect manual scroll and pause auto-scroll for 10 seconds
  const handleScroll = () => {
    setIsUserScrolling(true);
    clearTimeout(scrollTimeoutRef.current);
    
    scrollTimeoutRef.current = setTimeout(() => {
      setIsUserScrolling(false);
    }, 10000);
  };

  // Manual refresh
  const handleRefresh = async () => {
    setIsRefreshing(true);
    await refetch();
    setTimeout(() => setIsRefreshing(false), 500);
  };

  // Cleanup timeout on unmount
  useEffect(() => {
    return () => {
      if (scrollTimeoutRef.current) {
        clearTimeout(scrollTimeoutRef.current);
      }
    };
  }, []);

  if (loading && !metrics) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="text-center">
          <Activity className="w-12 h-12 text-blue-500 animate-pulse mx-auto mb-4" />
          <p className="text-gray-400">Loading metrics...</p>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="text-center">
          <p className="text-red-500 mb-4">Error: {error}</p>
          <button
            onClick={handleRefresh}
            className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 transition"
          >
            Retry
          </button>
        </div>
      </div>
    );
  }

  if (!metrics) {
    return (
      <div className="flex items-center justify-center h-full">
        <p className="text-gray-400">No metrics available</p>
      </div>
    );
  }

  const lastUpdated = new Date(metrics.timestamp_unix * 1000);
  const chartData = metrics.agents.map((agent, index) => ({
    name: agent.agent_type,
    success: agent.success_rate * 100,
    operations: agent.total_operations,
  }));

  return (
    <div
      ref={scrollableRef}
      onScroll={handleScroll}
      className="flex-1 overflow-y-auto px-4 py-6 space-y-6"
    >
      {/* Header with refresh */}
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-bold text-white">Agent Metrics</h2>
          <p className="text-sm text-gray-400">
            Updated {formatDistanceToNow(lastUpdated, { addSuffix: true })}
          </p>
        </div>
        <div className="flex items-center gap-2">
           <button
             onClick={() => handleExport('csv')}
             disabled={isExporting}
             className="px-3 py-2 bg-gray-800 hover:bg-gray-700 rounded-lg transition disabled:opacity-50 flex items-center gap-2"
             aria-label="Export as CSV"
           >
             <Download className="w-5 h-5 text-gray-400" />
             <span className="text-sm text-gray-300">CSV</span>
           </button>
           <button
             onClick={() => handleExport('json')}
             disabled={isExporting}
             className="px-3 py-2 bg-gray-800 hover:bg-gray-700 rounded-lg transition disabled:opacity-50 flex items-center gap-2"
             aria-label="Export as JSON"
           >
             <Download className="w-5 h-5 text-gray-400" />
             <span className="text-sm text-gray-300">JSON</span>
           </button>
          <button
            onClick={handleRefresh}
            disabled={isRefreshing || isExporting}
            className="p-2 bg-gray-800 hover:bg-gray-700 rounded-lg transition disabled:opacity-50"
            aria-label="Refresh metrics"
          >
            <RefreshCw className={`w-5 h-5 text-gray-400 ${isRefreshing ? 'animate-spin' : ''}`} />
          </button>
        </div>
      </div>

      {/* Export Error Toast */}
      {exportError && (
        <div className="fixed top-20 right-4 bg-red-600 text-white px-4 py-2 rounded-lg shadow-lg text-sm z-50">
          {exportError}
        </div>
      )}

      {/* Summary Cards */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
        <SummaryCard
          icon={<Cpu className="w-6 h-6" />}
          label="Total Tasks"
          value={metrics.total_tasks.toLocaleString()}
          color="blue"
        />
        <SummaryCard
          icon={<TrendingUp className="w-6 h-6" />}
          label="Success Rate"
          value={`${(metrics.overall_success_rate * 100).toFixed(1)}%`}
          color={metrics.overall_success_rate >= 0.9 ? 'green' : metrics.overall_success_rate >= 0.7 ? 'yellow' : 'red'}
        />
        <SummaryCard
          icon={<Activity className="w-6 h-6" />}
          label="Total Tokens"
          value={metrics.total_tokens.toLocaleString()}
          color="purple"
        />
        <SummaryCard
          icon={<DollarSign className="w-6 h-6" />}
          label="Est. Cost"
          value={`$${metrics.total_cost_usd.toFixed(4)}`}
          color="green"
        />
      </div>

      {/* Historical Trends Section */}
      <HistoricalTrends />

      {/* Agent Performance Cards */}
      <div className="space-y-4">
        <h3 className="text-lg font-semibold text-white">Agent Performance</h3>
        {metrics.agents.map((agent) => (
          <AgentCard key={agent.agent_type} agent={agent} />
        ))}
      </div>

      {/* Success Rate Chart */}
      <div className="bg-gray-800 rounded-lg p-6">
        <h3 className="text-lg font-semibold text-white mb-4">Success Rate by Agent</h3>
        <ResponsiveContainer width="100%" height={300}>
          <LineChart data={chartData}>
            <CartesianGrid strokeDasharray="3 3" stroke="#374151" />
            <XAxis dataKey="name" stroke="#9CA3AF" />
            <YAxis stroke="#9CA3AF" domain={[0, 100]} />
            <Tooltip
              contentStyle={{ backgroundColor: '#1F2937', border: 'none', borderRadius: '0.5rem' }}
              labelStyle={{ color: '#F9FAFB' }}
            />
            <Legend />
            <Line
              type="monotone"
              dataKey="success"
              stroke="#3B82F6"
              strokeWidth={2}
              dot={{ fill: '#3B82F6' }}
              name="Success Rate (%)"
            />
          </LineChart>
        </ResponsiveContainer>
      </div>

      {/* Scroll indicator when user scrolling is paused */}
      {isUserScrolling && (
        <div className="fixed bottom-20 left-1/2 transform -translate-x-1/2 bg-yellow-600 text-white px-4 py-2 rounded-full text-sm shadow-lg">
          Auto-scroll paused for 10s
        </div>
      )}
    </div>
  );
}

// Historical Trends Component
type TrendInterval = 'Hourly' | 'Daily' | 'Weekly';

function HistoricalTrends() {
  const [interval, setInterval] = useState<TrendInterval>('Daily');
  const [trendData, setTrendData] = useState<any[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchTrendData = async (selectedInterval: TrendInterval) => {
    setLoading(true);
    setError(null);
    try {
      const data = await invoke<any[]>('get_metrics_trend', { interval: selectedInterval });
      const formattedData = data.map(d => ({
        ...d,
        timestamp_str: new Date(d.timestamp_unix * 1000).toLocaleString(),
        success_rate_percent: d.success_rate * 100,
      }));
      setTrendData(formattedData);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to fetch trend data');
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchTrendData(interval);
  }, [interval]);

  return (
    <div className="bg-gray-800 rounded-lg p-6">
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-lg font-semibold text-white">Historical Trends</h3>
        <div className="flex items-center gap-2">
          {(['Hourly', 'Daily', 'Weekly'] as TrendInterval[]).map((i) => (
            <button
              key={i}
              onClick={() => setInterval(i)}
              className={`px-3 py-1 text-sm rounded-md transition ${
                interval === i ? 'bg-blue-600 text-white' : 'bg-gray-700 hover:bg-gray-600 text-gray-300'
              }`}
            >
              {i}
            </button>
          ))}
        </div>
      </div>

      {loading && <p className="text-center text-gray-400">Loading trends...</p>}
      {error && <p className="text-center text-red-500">Error: {error}</p>}

      {!loading && !error && trendData.length > 0 && (
        <ResponsiveContainer width="100%" height={300}>
          <LineChart data={trendData}>
            <CartesianGrid strokeDasharray="3 3" stroke="#374151" />
            <XAxis dataKey="timestamp_str" stroke="#9CA3AF" />
            <YAxis yAxisId="left" stroke="#3B82F6" label={{ value: 'Success Rate (%)', angle: -90, position: 'insideLeft', fill: '#9CA3AF' }} />
            <YAxis yAxisId="right" orientation="right" stroke="#8B5CF6" label={{ value: 'Operations', angle: 90, position: 'insideRight', fill: '#9CA3AF' }} />
            <Tooltip
              contentStyle={{ backgroundColor: '#1F2937', border: 'none', borderRadius: '0.5rem' }}
              labelStyle={{ color: '#F9FAFB' }}
            />
            <Legend />
            <Line yAxisId="left" type="monotone" dataKey="success_rate_percent" name="Success Rate" stroke="#3B82F6" strokeWidth={2} dot={false} />
            <Line yAxisId="right" type="monotone" dataKey="total_operations" name="Operations" stroke="#8B5CF6" strokeWidth={2} dot={false} />
          </LineChart>
        </ResponsiveContainer>
      )}

      {!loading && !error && trendData.length === 0 && (
        <p className="text-center text-gray-400 py-10">No historical data available for this period.</p>
      )}
    </div>
  );
}

// Summary Card Component
interface SummaryCardProps {
  icon: React.ReactNode;
  label: string;
  value: string;
  color: 'blue' | 'green' | 'yellow' | 'red' | 'purple';
}

function SummaryCard({ icon, label, value, color }: SummaryCardProps) {
  const colorClasses = {
    blue: 'text-blue-500 bg-blue-500/10',
    green: 'text-green-500 bg-green-500/10',
    yellow: 'text-yellow-500 bg-yellow-500/10',
    red: 'text-red-500 bg-red-500/10',
    purple: 'text-purple-500 bg-purple-500/10',
  };

  return (
    <div className="bg-gray-800 rounded-lg p-4">
      <div className={`w-12 h-12 rounded-lg flex items-center justify-center mb-3 ${colorClasses[color]}`}>
        {icon}
      </div>
      <p className="text-sm text-gray-400 mb-1">{label}</p>
      <p className="text-2xl font-bold text-white">{value}</p>
    </div>
  );
}

// Agent Card Component
interface AgentCardProps {
  agent: AgentMetrics;
}

function AgentCard({ agent }: AgentCardProps) {
  const [expanded, setExpanded] = useState(false);
  
  const successRate = agent.success_rate * 100;
  const successColor = successRate >= 90 ? 'bg-green-500' : successRate >= 70 ? 'bg-yellow-500' : 'bg-red-500';

  return (
    <div className="bg-gray-800 rounded-lg p-4">
      <div className="flex items-center justify-between mb-3">
        <div>
          <h4 className="text-lg font-semibold text-white">{agent.agent_type}</h4>
          <p className="text-sm text-gray-400">{agent.total_operations} operations</p>
        </div>
        <button
          onClick={() => setExpanded(!expanded)}
          className="text-sm text-blue-400 hover:text-blue-300 transition"
        >
          {expanded ? 'Less' : 'More'}
        </button>
      </div>

      {/* Success Rate Progress Bar */}
      <div className="mb-3">
        <div className="flex justify-between text-sm mb-1">
          <span className="text-gray-400">Success Rate</span>
          <span className="text-white font-medium">{successRate.toFixed(1)}%</span>
        </div>
        <div className="w-full bg-gray-700 rounded-full h-2">
          <div
            className={`${successColor} h-2 rounded-full transition-all duration-300`}
            style={{ width: `${successRate}%` }}
          />
        </div>
      </div>

      {/* Key Metrics */}
      <div className="grid grid-cols-2 gap-4 text-sm">
        <div>
          <p className="text-gray-400">Avg Response Time</p>
          <p className="text-white font-medium">{agent.avg_response_time_ms.toFixed(0)}ms</p>
        </div>
        <div>
          <p className="text-gray-400">Avg Tokens</p>
          <p className="text-white font-medium">{agent.avg_tokens_used.toFixed(0)}</p>
        </div>
      </div>

      {/* Expanded Details */}
      {expanded && (
        <div className="mt-4 pt-4 border-t border-gray-700 grid grid-cols-2 gap-4 text-sm">
          <div>
            <p className="text-gray-400">JSON Parse Success</p>
            <p className="text-white font-medium">{(agent.json_parse_success_rate * 100).toFixed(1)}%</p>
          </div>
          <div>
            <p className="text-gray-400">Validation Pass</p>
            <p className="text-white font-medium">{(agent.validation_pass_rate * 100).toFixed(1)}%</p>
          </div>
          <div>
            <p className="text-gray-400">Syntax Errors</p>
            <p className="text-white font-medium">{(agent.syntax_error_rate * 100).toFixed(1)}%</p>
          </div>
          <div>
            <p className="text-gray-400">First Operation</p>
            <p className="text-white font-medium">
              {formatDistanceToNow(new Date(agent.first_operation_unix * 1000), { addSuffix: true })}
            </p>
          </div>
        </div>
      )}
    </div>
  );
}
