//! # START OF FILE hainet-portal/src/hooks/useMetrics.ts
import { useState, useEffect, useCallback } from 'react';
import { invoke, listen, UnlistenFn, save, writeTextFile } from '../lib/tauri';
import { MetricsSummary, TrendDataPoint, TrendInterval, TimeRange } from '../types';

interface UseMetricsResult {
  metrics: MetricsSummary | null;
  loading: boolean;
  error: string | null;
  refetch: () => Promise<void>;
  trendData: TrendDataPoint[] | null;
  trendLoading: boolean;
  trendError: string | null;
  getTrendData: (interval: TrendInterval) => Promise<void>;
  exportMetrics: (format: 'csv' | 'json', timeRange?: TimeRange) => Promise<void>;
}

export function useMetrics(): UseMetricsResult {
  const [metrics, setMetrics] = useState<MetricsSummary | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const [trendData, setTrendData] = useState<TrendDataPoint[] | null>(null);
  const [trendLoading, setTrendLoading] = useState(false);
  const [trendError, setTrendError] = useState<string | null>(null);

  const fetchMetrics = useCallback(async () => {
    try {
      setError(null);
      const summary = await invoke<MetricsSummary>('get_metrics_summary');
      setMetrics(summary);
    } catch (err) {
      console.error('Failed to fetch metrics:', err);
      setError(err instanceof Error ? err.message : 'Failed to fetch metrics');
    } finally {
      setLoading(false);
    }
  }, []);

  const getTrendData = useCallback(async (interval: TrendInterval) => {
    setTrendLoading(true);
    setTrendError(null);
    try {
      const data = await invoke<TrendDataPoint[]>('get_metrics_trend', { interval });
      setTrendData(data);
    } catch (err) {
      console.error('Failed to fetch trend data:', err);
      setTrendError(err instanceof Error ? err.message : 'Failed to fetch trend data');
    } finally {
      setTrendLoading(false);
    }
  }, []);

  const exportMetrics = useCallback(async (format: 'csv' | 'json', timeRange?: TimeRange) => {
    try {
      const command = `export_metrics_${format}`;
      const content = await invoke<string>(command, { timeRange });

      const suggestedFilename = `hainet-metrics-${new Date().toISOString().split('T')[0]}.${format}`;
      const filePath = await save({
        title: `Export Metrics as ${format.toUpperCase()}`,
        defaultPath: suggestedFilename,
        filters: [{
          name: format.toUpperCase(),
          extensions: [format],
        }],
      });

      if (filePath) {
        await writeTextFile(filePath, content);
      }
    } catch (err) {
      console.error(`Failed to export metrics as ${format}:`, err);
      // Optionally, set an export-specific error state to show in the UI
    }
  }, []);

  // Listen to Tauri events for real-time metric updates
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;

    const setupListener = async () => {
      try {
        unlisten = await listen<MetricsSummary>('metrics-updated', (event) => {
          console.log('Metrics updated via event:', event.payload);
          setMetrics(event.payload);
          setLoading(false);
        });
      } catch (err) {
        console.warn('Failed to setup event listener, falling back to polling:', err);
      }
    };

    setupListener();

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  // Fallback polling if events are not available (every 5 seconds)
  useEffect(() => {
    // Initial fetch
    fetchMetrics();

    // Setup polling interval
    const interval = setInterval(fetchMetrics, 5000);

    return () => {
      clearInterval(interval);
    };
  }, [fetchMetrics]);

  return {
    metrics,
    loading,
    error,
    refetch: fetchMetrics,
    trendData,
    trendLoading,
    trendError,
    getTrendData,
    exportMetrics,
  };
}
