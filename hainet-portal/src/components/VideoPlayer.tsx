//! # START OF FILE hainet-portal/src/components/VideoPlayer.tsx
import React, { useRef, useEffect } from 'react';

interface VideoPlayerProps {
  src: string | null;
  isVisible: boolean;
  onClose: () => void;
}

const VideoPlayer: React.FC<VideoPlayerProps> = ({ src, isVisible, onClose }) => {
  const videoRef = useRef<HTMLVideoElement>(null);

  useEffect(() => {
    if (isVisible && videoRef.current) {
      videoRef.current.play().catch(error => {
        console.error("Video play failed:", error);
      });
    } else if (!isVisible && videoRef.current) {
      videoRef.current.pause();
    }
  }, [isVisible]);

  if (!isVisible || !src) {
    return null;
  }

  return (
    <div className="fixed inset-0 bg-black bg-opacity-80 flex items-center justify-center z-50">
      <div className="relative w-full max-w-4xl">
        <video ref={videoRef} src={src} controls className="w-full h-full" />
        <button
          onClick={onClose}
          className="absolute top-2 right-2 bg-red-600 text-white rounded-full p-2 w-10 h-10 flex items-center justify-center"
        >
          ✕
        </button>
      </div>
    </div>
  );
};

export default VideoPlayer;
