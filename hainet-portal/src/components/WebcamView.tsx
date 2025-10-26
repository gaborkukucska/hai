// hainet-portal/src/components/WebcamView.tsx
import React, { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import { PrivacyMode, FrameAnalysisResult } from '../types'; // Assuming types are defined

interface WebcamViewProps {
  onFrameAnalysis: (analysis: FrameAnalysisResult) => void;
}

const WebcamView: React.FC<WebcamViewProps> = ({ onFrameAnalysis }) => {
  const [devices, setDevices] = useState<string[]>([]);
  const [selectedDevice, setSelectedDevice] = useState<number>(0);
  const [isCapturing, setIsCapturing] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);
  const videoRef = useRef<HTMLVideoElement>(null);

  useEffect(() => {
    const fetchDevices = async () => {
      try {
        const deviceList = await invoke<string[]>('list_webcam_devices');
        setDevices(deviceList);
      } catch (err) {
        setError(err as string);
      }
    };
    fetchDevices();
  }, []);

  const handleStartCapture = async () => {
    try {
      await invoke('start_webcam', {
        config: {
          device_index: selectedDevice,
          resolution_width: 1280,
          resolution_height: 720,
          frame_rate: 30,
          privacy_mode: 'Off',
        },
      });
      setIsCapturing(true);
      setError(null);
      // Logic to stream video to the video element would go here
      // This is a simplification; real implementation would use WebRTC
    } catch (err) {
      setError(err as string);
    }
  };

  const handleStopCapture = async () => {
    try {
      await invoke('stop_webcam');
      setIsCapturing(false);
    } catch (err) {
      setError(err as string);
    }
  };

  const handleCaptureFrame = async () => {
    try {
      const result = await invoke<{ image_base64: string; analysis: FrameAnalysisResult }>('capture_frame');
      onFrameAnalysis(result.analysis);
      // You could display the captured frame in an img element if desired
    } catch (err) {
      setError(err as string);
    }
  };

  return (
    <div>
      <h3>Webcam</h3>
      {error && <p style={{ color: 'red' }}>{error}</p>}
      <div>
        <select onChange={(e) => setSelectedDevice(parseInt(e.target.value, 10))} value={selectedDevice}>
          {devices.map((device, index) => (
            <option key={index} value={index}>
              {device}
            </option>
          ))}
        </select>
        {!isCapturing ? (
          <button onClick={handleStartCapture}>Start Camera</button>
        ) : (
          <button onClick={handleStopCapture}>Stop Camera</button>
        )}
        {isCapturing && <button onClick={handleCaptureFrame}>Capture Frame</button>}
      </div>
      <video ref={videoRef} style={{ width: '100%', maxWidth: '500px', border: '1px solid black' }} />
    </div>
  );
};

export default WebcamView;
