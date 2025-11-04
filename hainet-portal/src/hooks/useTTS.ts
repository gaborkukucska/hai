// START OF FILE hainet-portal/src/hooks/useTTS.ts
/**
 * useTTS Hook
 * 
 * React hook for Text-to-Speech functionality via Tauri backend
 */

import { useState, useCallback } from 'react';
import { invoke } from '../lib/tauri';

interface SynthesisRequest {
  text: string;
  voice?: string;
  speed?: number;
}

interface SynthesisResponse {
  audio_base64: string;
  format: string;
  sample_rate: number;
  duration_ms: number;
  text: string;
}

interface UseTTSReturn {
  synthesize: (text: string, options?: { voice?: string; speed?: number }) => Promise<SynthesisResponse>;
  isSynthesizing: boolean;
  error: Error | null;
  isReady: boolean;
  checkReady: () => Promise<boolean>;
  listVoices: () => Promise<string[]>;
}

export const useTTS = (): UseTTSReturn => {
  const [isSynthesizing, setIsSynthesizing] = useState(false);
  const [error, setError] = useState<Error | null>(null);
  const [isReady, setIsReady] = useState(false);

  const checkReady = useCallback(async (): Promise<boolean> => {
    try {
      const ready = await invoke<boolean>('tts_is_ready');
      setIsReady(ready);
      return ready;
    } catch (err) {
      console.error('Failed to check TTS readiness:', err);
      setIsReady(false);
      return false;
    }
  }, []);

  const synthesize = useCallback(async (
    text: string,
    options?: { voice?: string; speed?: number }
  ): Promise<SynthesisResponse> => {
    setIsSynthesizing(true);
    setError(null);

    try {
      const request: SynthesisRequest = {
        text,
        voice: options?.voice,
        speed: options?.speed,
      };

      const response = await invoke<SynthesisResponse>('synthesize_speech', { request });
      return response;
    } catch (err) {
      const error = err instanceof Error ? err : new Error(String(err));
      setError(error);
      throw error;
    } finally {
      setIsSynthesizing(false);
    }
  }, []);

  const listVoices = useCallback(async (): Promise<string[]> => {
    try {
      return await invoke<string[]>('list_tts_voices');
    } catch (err) {
      console.error('Failed to list voices:', err);
      return [];
    }
  }, []);

  return {
    synthesize,
    isSynthesizing,
    error,
    isReady,
    checkReady,
    listVoices,
  };
};

export default useTTS;
