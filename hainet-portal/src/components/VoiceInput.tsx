// # START OF FILE hainet-portal/src/components/VoiceInput.tsx
// Voice input component with VAD (Voice Activity Detection)
// Captures audio, detects speech, sends to backend for transcription

import React, { useState, useRef, useEffect } from 'react';
import { invoke } from '../lib/tauri';

interface AudioData {
  data: string; // Base64-encoded audio
  sample_rate: number;
  channels: number;
  format: string;
}

interface TranscriptionResult {
  text: string;
  confidence: number;
  language: string;
  processing_time_ms: number;
}

interface VoiceInputProps {
  onTranscription: (text: string) => void;
  onError?: (error: string) => void;
}

export const VoiceInput: React.FC<VoiceInputProps> = ({ onTranscription, onError }) => {
  const [isRecording, setIsRecording] = useState(false);
  const [audioLevel, setAudioLevel] = useState(0);
  const [status, setStatus] = useState<string>('Ready');
  const [vadEnabled, setVadEnabled] = useState(true);
  const [vadThreshold, setVadThreshold] = useState(0.5);

  const mediaRecorderRef = useRef<MediaRecorder | null>(null);
  const audioContextRef = useRef<AudioContext | null>(null);
  const analyserRef = useRef<AnalyserNode | null>(null);
  const audioChunksRef = useRef<Blob[]>([]);
  const animationFrameRef = useRef<number | null>(null);

  // Initialize audio context and analyser for VAD
  useEffect(() => {
    return () => {
      // Cleanup on unmount
      if (animationFrameRef.current) {
        cancelAnimationFrame(animationFrameRef.current);
      }
      if (audioContextRef.current) {
        audioContextRef.current.close();
      }
    };
  }, []);

  // Calculate audio level for VAD
  const calculateAudioLevel = (dataArray: Uint8Array): number => {
    const sum = dataArray.reduce((acc, val) => acc + val, 0);
    const avg = sum / dataArray.length;
    return avg / 255; // Normalize to 0.0-1.0
  };

  // Monitor audio level in real-time
  const monitorAudioLevel = () => {
    if (!analyserRef.current) return;

    const dataArray = new Uint8Array(analyserRef.current.frequencyBinCount);
    analyserRef.current.getByteFrequencyData(dataArray);
    
    const level = calculateAudioLevel(dataArray);
    setAudioLevel(level);

    // Check VAD threshold
    if (vadEnabled && level > vadThreshold) {
      setStatus('Speech detected');
    } else if (vadEnabled) {
      setStatus('Listening...');
    }

    animationFrameRef.current = requestAnimationFrame(monitorAudioLevel);
  };

  // Start recording
  const startRecording = async () => {
    try {
      setStatus('Requesting microphone access...');
      
      const stream = await navigator.mediaDevices.getUserMedia({
        audio: {
          echoCancellation: true,
          noiseSuppression: true,
          autoGainControl: true,
          sampleRate: 16000, // Optimal for Whisper
        }
      });

      // Create audio context for VAD
      audioContextRef.current = new AudioContext({ sampleRate: 16000 });
      const source = audioContextRef.current.createMediaStreamSource(stream);
      
      analyserRef.current = audioContextRef.current.createAnalyser();
      analyserRef.current.fftSize = 2048;
      source.connect(analyserRef.current);

      // Start monitoring audio level
      monitorAudioLevel();

      // Create media recorder
      const options = { mimeType: 'audio/webm;codecs=opus' };
      mediaRecorderRef.current = new MediaRecorder(stream, options);
      
      audioChunksRef.current = [];
      
      mediaRecorderRef.current.ondataavailable = (event) => {
        if (event.data.size > 0) {
          audioChunksRef.current.push(event.data);
        }
      };

      mediaRecorderRef.current.onstop = async () => {
        const audioBlob = new Blob(audioChunksRef.current, { type: 'audio/webm' });
        await transcribeAudio(audioBlob);
        
        // Stop all tracks
        stream.getTracks().forEach(track => track.stop());
      };

      mediaRecorderRef.current.start();
      setIsRecording(true);
      setStatus('Recording...');
      
    } catch (error) {
      const errorMsg = `Failed to start recording: ${error}`;
      setStatus(errorMsg);
      if (onError) onError(errorMsg);
    }
  };

  // Stop recording
  const stopRecording = () => {
    if (mediaRecorderRef.current && isRecording) {
      mediaRecorderRef.current.stop();
      setIsRecording(false);
      setStatus('Processing...');
      
      if (animationFrameRef.current) {
        cancelAnimationFrame(animationFrameRef.current);
        animationFrameRef.current = null;
      }
    }
  };

  // Transcribe audio via backend
  const transcribeAudio = async (audioBlob: Blob) => {
    try {
      setStatus('Transcribing...');
      
      // Convert blob to base64
      const arrayBuffer = await audioBlob.arrayBuffer();
      const uint8Array = new Uint8Array(arrayBuffer);
      const base64 = btoa(String.fromCharCode(...uint8Array));

      const audioData: AudioData = {
        data: base64,
        sample_rate: 16000,
        channels: 1,
        format: 'webm',
      };

      const result = await invoke<TranscriptionResult>('transcribe_audio', { audio: audioData });
      
      setStatus(`Transcribed (${result.processing_time_ms}ms, ${(result.confidence * 100).toFixed(0)}%)`);
      onTranscription(result.text);
      
    } catch (error) {
      const errorMsg = `Transcription failed: ${error}`;
      setStatus(errorMsg);
      if (onError) onError(errorMsg);
    }
  };

  return (
    <div className="voice-input-container p-4 border border-gray-300 rounded-lg">
      <div className="flex flex-col space-y-4">
        {/* Recording button */}
        <button
          onClick={isRecording ? stopRecording : startRecording}
          className={`px-6 py-3 rounded-lg font-semibold transition-colors ${
            isRecording
              ? 'bg-red-500 hover:bg-red-600 text-white'
              : 'bg-blue-500 hover:bg-blue-600 text-white'
          }`}
        >
          {isRecording ? '🔴 Stop Recording' : '🎤 Start Recording'}
        </button>

        {/* Status */}
        <div className="text-sm text-gray-600">
          Status: <span className="font-medium">{status}</span>
        </div>

        {/* Audio level visualization */}
        {isRecording && (
          <div className="flex flex-col space-y-2">
            <div className="text-sm text-gray-600">Audio Level:</div>
            <div className="w-full h-4 bg-gray-200 rounded-full overflow-hidden">
              <div
                className={`h-full transition-all ${
                  audioLevel > vadThreshold ? 'bg-green-500' : 'bg-blue-400'
                }`}
                style={{ width: `${audioLevel * 100}%` }}
              />
            </div>
          </div>
        )}

        {/* VAD settings */}
        <div className="flex flex-col space-y-2 pt-2 border-t border-gray-200">
          <label className="flex items-center space-x-2">
            <input
              type="checkbox"
              checked={vadEnabled}
              onChange={(e) => setVadEnabled(e.target.checked)}
              className="form-checkbox"
            />
            <span className="text-sm">Enable Voice Activity Detection</span>
          </label>
          
          {vadEnabled && (
            <div className="flex flex-col space-y-1">
              <label className="text-sm text-gray-600">
                VAD Threshold: {vadThreshold.toFixed(2)}
              </label>
              <input
                type="range"
                min="0.1"
                max="0.9"
                step="0.05"
                value={vadThreshold}
                onChange={(e) => setVadThreshold(parseFloat(e.target.value))}
                className="w-full"
              />
            </div>
          )}
        </div>

        {/* Info text */}
        <div className="text-xs text-gray-500 pt-2">
          <p>📝 Click the microphone button to start recording.</p>
          <p>🟢 Green bar = Speech detected | 🔵 Blue bar = Below threshold</p>
          <p>🔴 Click Stop when finished speaking.</p>
        </div>
      </div>
    </div>
  );
};
