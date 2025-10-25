// START OF FILE hainet-portal/src/components/AudioPlayer.tsx
/**
 * AudioPlayer Component
 * 
 * Plays synthesized speech audio from base64-encoded data
 */

import React, { useEffect, useRef } from 'react';

interface AudioPlayerProps {
  audioBase64: string;
  format: string;
  autoPlay?: boolean;
  onEnded?: () => void;
  onError?: (error: Error) => void;
}

export const AudioPlayer: React.FC<AudioPlayerProps> = ({
  audioBase64,
  format,
  autoPlay = true,
  onEnded,
  onError,
}) => {
  const audioRef = useRef<HTMLAudioElement>(null);

  useEffect(() => {
    if (!audioBase64 || !audioRef.current) return;

    try {
      // Determine MIME type from format
      const mimeType = format.toLowerCase().includes('wav') 
        ? 'audio/wav' 
        : 'audio/mpeg';

      // Create blob from base64
      const binaryString = atob(audioBase64);
      const bytes = new Uint8Array(binaryString.length);
      for (let i = 0; i < binaryString.length; i++) {
        bytes[i] = binaryString.charCodeAt(i);
      }
      const blob = new Blob([bytes], { type: mimeType });

      // Create object URL and set as audio source
      const url = URL.createObjectURL(blob);
      audioRef.current.src = url;

      if (autoPlay) {
        audioRef.current.play().catch((err) => {
          console.error('Audio playback failed:', err);
          onError?.(err);
        });
      }

      // Cleanup URL on unmount
      return () => {
        URL.revokeObjectURL(url);
      };
    } catch (err) {
      console.error('Failed to create audio from base64:', err);
      onError?.(err as Error);
    }
  }, [audioBase64, format, autoPlay, onError]);

  const handleEnded = () => {
    onEnded?.();
  };

  const handleError = (e: React.SyntheticEvent<HTMLAudioElement, Event>) => {
    const error = new Error('Audio playback error');
    console.error('Audio playback error:', e);
    onError?.(error);
  };

  return (
    <audio
      ref={audioRef}
      onEnded={handleEnded}
      onError={handleError}
      style={{ display: 'none' }}
    />
  );
};

export default AudioPlayer;
