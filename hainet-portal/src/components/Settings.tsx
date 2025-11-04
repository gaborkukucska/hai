// <!-- # START OF FILE hainet-portal/src/components/Settings.tsx -->
import React, { useState, useEffect } from 'react';
import { invoke } from '../lib/tauri';
import SystemStatus from './SystemStatus';

interface SettingsData {
  theme: string;
  audio_input_device: string | null;
  video_input_device: string | null;
  stt_model: string | null;
  tts_model: string | null;
  vision_model: string | null;
}

const Settings: React.FC = () => {
  const [settings, setSettings] = useState<SettingsData | null>(null);

  useEffect(() => {
    invoke<SettingsData>('get_settings').then(setSettings);
  }, []);

  const handleSettingChange = (key: keyof SettingsData, value: string) => {
    if (settings) {
      const newSettings = { ...settings, [key]: value };
      setSettings(newSettings);
      invoke('update_settings', { settings: newSettings });
    }
  };

  if (!settings) {
    return <div>Loading settings...</div>;
  }

  return (
    <div className="p-6 bg-gray-900 text-white h-full overflow-y-auto">
      <h1 className="text-2xl font-bold mb-6">Settings</h1>

      <div className="space-y-8">
        {/* Audio/Video Settings */}
        <div className="p-4 border border-gray-700 rounded-lg">
          <h2 className="text-lg font-semibold mb-3">Audio & Video</h2>
          <div className="space-y-4">
            <div>
              <label htmlFor="mic-select" className="block mb-2">Microphone</label>
              <select id="mic-select" className="w-full p-2 bg-gray-800 border border-gray-600 rounded" value={settings.audio_input_device || ''} onChange={(e) => handleSettingChange('audio_input_device', e.target.value)}>
                <option value="">Default Microphone</option>
              </select>
            </div>
            <div>
              <label htmlFor="cam-select" className="block mb-2">Camera</label>
              <select id="cam-select" className="w-full p-2 bg-gray-800 border border-gray-600 rounded" value={settings.video_input_device || ''} onChange={(e) => handleSettingChange('video_input_device', e.target.value)}>
                <option value="">Default Camera</option>
              </select>
            </div>
          </div>
        </div>

        {/* AI Model Settings */}
        <div className="p-4 border border-gray-700 rounded-lg">
          <h2 className="text-lg font-semibold mb-3">AI Models</h2>
          <div className="space-y-4">
            <div>
              <label htmlFor="stt-model-select" className="block mb-2">Speech-to-Text Model</label>
              <select id="stt-model-select" className="w-full p-2 bg-gray-800 border border-gray-600 rounded" value={settings.stt_model || ''} onChange={(e) => handleSettingChange('stt_model', e.target.value)}>
                <option value="">Default STT Model</option>
              </select>
            </div>
            <div>
              <label htmlFor="tts-model-select" className="block mb-2">Text-to-Speech Model</label>
              <select id="tts-model-select" className="w-full p-2 bg-gray-800 border border-gray-600 rounded" value={settings.tts_model || ''} onChange={(e) => handleSettingChange('tts_model', e.target.value)}>
                <option value="">Default TTS Model</option>
              </select>
            </div>
            <div>
              <label htmlFor="vision-model-select" className="block mb-2">Vision Model</label>
              <select id="vision-model-select" className="w-full p-2 bg-gray-800 border border-gray-600 rounded" value={settings.vision_model || ''} onChange={(e) => handleSettingChange('vision_model', e.target.value)}>
                <option value="">Default Vision Model</option>
              </select>
            </div>
          </div>
        </div>

        {/* Theme Settings */}
        <div className="p-4 border border-gray-700 rounded-lg">
          <h2 className="text-lg font-semibold mb-3">Appearance</h2>
          <div>
            <label htmlFor="theme-select" className="block mb-2">Theme</label>
            <select id="theme-select" className="w-full p-2 bg-gray-800 border border-gray-600 rounded" value={settings.theme} onChange={(e) => handleSettingChange('theme', e.target.value)}>
              <option value="dark">Dark</option>
              <option value="light">Light</option>
            </select>
          </div>
        </div>

        {/* System Status */}
        <SystemStatus />
      </div>
    </div>
  );
};

export default Settings;
// <!-- # END OF FILE hainet-portal/src/components/Settings.tsx -->
