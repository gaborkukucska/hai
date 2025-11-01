//! # START OF FILE hainet-portal/src/hooks/useMetrics.ts
import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { MetricsSummary } from '../types';

interface UseMetricsResult {
  metrics: MetricsSummary | null;
  loading: boolean;
  error: string | null;
  refetch: () => Promise<void>;
}

export function useMetrics(): UseMetricsResult {
  const [metrics, setMetrics] = useState<MetricsSummary | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchMetrics = useCallback(async () => {
    try {
      setError(null);
      const summary = await invoke<MetricsSummary>('get_metrics_summary');
      setMetrics(summary);
      setLoading(false);
    } catch (err) {
      console.error('Failed to fetch metrics:', err);
      setError(err instanceof Error ? err.message : 'Failed to fetch metrics');
      setLoading(false);
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
  };
}
