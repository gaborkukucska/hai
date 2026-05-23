//! # START OF FILE hainet-portal/src/pages/Settings.tsx
import React, { useEffect, useState } from 'react';
import { Shield, Moon, Database, Bell, Info, Cpu, Mic, Video, MessageSquare } from 'lucide-react';
import { invoke } from '../lib/tauri';
import type { Settings as SettingsType, ModelPreference, ModelFamily } from '../types';
import { MODEL_FAMILIES } from '../types';

export default function Settings() {
  const [settings, setSettings] = useState<SettingsType | null>(null);
  const [modelPreferences, setModelPreferences] = useState<ModelPreference[]>([]);
  const [loading, setLoading] = useState(true);
  const [saveStatus, setSaveStatus] = useState<'idle' | 'saving' | 'saved' | 'error'>('idle');

  // Load settings on component mount
  useEffect(() => {
    loadSettings();
    loadModelPreferences();
  }, []);

  const loadSettings = async () => {
    console.log('[Settings] Loading settings...');
    try {
      const loadedSettings = await invoke<SettingsType>('get_settings');
      console.log('[Settings] Settings loaded:', loadedSettings);
      setSettings(loadedSettings);
    } catch (error) {
      console.error('[Settings] Failed to load settings:', error);
    } finally {
      setLoading(false);
    }
  };

  const loadModelPreferences = async () => {
    console.log('[Settings] Loading model preferences...');
    try {
      const prefs = await invoke<ModelPreference[]>('get_model_preferences');
      console.log('[Settings] Model preferences loaded:', prefs);
      console.log('[Settings] Preference count:', prefs.length);
      setModelPreferences(prefs);
    } catch (error) {
      console.error('[Settings] Failed to load model preferences:', error);
    }
  };

  const updateSetting = async (key: keyof SettingsType, value: any) => {
    if (!settings) return;

    const updatedSettings = { ...settings, [key]: value };
    setSettings(updatedSettings);

    // Save to backend
    setSaveStatus('saving');
    try {
      await invoke('update_settings', { settings: updatedSettings });
      setSaveStatus('saved');
      setTimeout(() => setSaveStatus('idle'), 2000);
    } catch (error) {
      console.error('Failed to save settings:', error);
      setSaveStatus('error');
      setTimeout(() => setSaveStatus('idle'), 2000);
    }
  };

  const updateModelPreference = async (
    agentType: 'Admin' | 'PM' | 'Worker',
    preferredFamily: string,
    allowFallback: boolean
  ) => {
    console.log('[Settings] Updating model preference:', { agentType, preferredFamily, allowFallback });
    
    // Save old state for rollback
    const oldPreferences = [...modelPreferences];
    console.log('[Settings] Old preferences:', oldPreferences);
    
    // Optimistic update
    const updated = modelPreferences.filter(p => p.agent_type !== agentType);
    updated.push({ agent_type: agentType, preferred_family: preferredFamily, allow_fallback: allowFallback });
    console.log('[Settings] Optimistic update - new preferences:', updated);
    setModelPreferences(updated);
    
    setSaveStatus('saving');
    try {
      console.log('[Settings] Invoking save_model_preference command...');
      await invoke('save_model_preference', {
        agent_type: agentType,
        family: preferredFamily,
        allow_fallback: allowFallback,
      });
      console.log('[Settings] Save command completed successfully');
      
      // Reload from database to confirm
      console.log('[Settings] Reloading preferences from database for confirmation...');
      const confirmedPrefs = await invoke<ModelPreference[]>('get_model_preferences');
      console.log('[Settings] Confirmed preferences from database:', confirmedPrefs);
      setModelPreferences(confirmedPrefs);
      
      setSaveStatus('saved');
      setTimeout(() => setSaveStatus('idle'), 2000);
    } catch (error) {
      console.error('[Settings] Failed to save model preference:', error);
      console.log('[Settings] Rolling back to old preferences:', oldPreferences);
      
      // Rollback on failure
      setModelPreferences(oldPreferences);
      setSaveStatus('error');
      setTimeout(() => setSaveStatus('idle'), 2000);
    }
  };

  const getModelPreference = (agentType: 'Admin' | 'PM' | 'Worker'): ModelPreference | undefined => {
    return modelPreferences.find(p => p.agent_type === agentType);
  };

  if (loading || !settings) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <div className="text-white">Loading settings...</div>
      </div>
    );
  }

  return (
    <div className="flex-1 overflow-y-auto px-4 py-6 space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-bold text-white">Settings</h2>
          <p className="text-sm text-gray-400">Configure your HAI-Net Portal</p>
        </div>
        {saveStatus === 'saving' && (
          <span className="text-sm text-blue-400">Saving...</span>
        )}
        {saveStatus === 'saved' && (
          <span className="text-sm text-green-400">✓ Saved</span>
        )}
        {saveStatus === 'error' && (
          <span className="text-sm text-red-400">Save failed</span>
        )}
      </div>

      {/* AI Model Preferences */}
      <SettingsSection
        icon={<Cpu className="w-6 h-6" />}
        title="AI Model Preferences"
        description="Configure preferred model families for each agent type"
      >
        {(['Admin', 'PM', 'Worker'] as const).map(agentType => {
          const pref = getModelPreference(agentType);
          return (
            <div key={agentType} className="border-b border-gray-700 last:border-0 pb-4 last:pb-0">
              <div className="flex items-start gap-4">
                <div className="flex-1">
                  <label className="block text-white font-medium mb-2">
                    {agentType} Agent Model Family
                  </label>
                  <select
                    value={pref?.preferred_family || 'auto'}
                    onChange={(e) => updateModelPreference(
                      agentType,
                      e.target.value,
                      pref?.allow_fallback ?? true
                    )}
                    className="w-full bg-gray-700 text-white rounded px-3 py-2 border border-gray-600 focus:border-blue-500 focus:outline-none"
                  >
                    {MODEL_FAMILIES.map(family => (
                      <option key={family.id} value={family.id}>
                        {family.name}
                      </option>
                    ))}
                  </select>
                  <p className="text-sm text-gray-400 mt-1">
                    {MODEL_FAMILIES.find(f => f.id === (pref?.preferred_family || 'auto'))?.description}
                  </p>
                </div>
              </div>
              <div className="mt-3">
                <label className="flex items-center gap-2 text-sm text-gray-300 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={pref?.allow_fallback ?? true}
                    onChange={(e) => updateModelPreference(
                      agentType,
                      pref?.preferred_family || 'auto',
                      e.target.checked
                    )}
                    className="w-4 h-4 rounded border-gray-600 bg-gray-700 text-blue-600 focus:ring-blue-500"
                  />
                  Allow fallback to other families if preferred unavailable
                </label>
              </div>
            </div>
          );
        })}
      </SettingsSection>

      {/* Multimodal Settings */}
      <SettingsSection
        icon={<MessageSquare className="w-6 h-6" />}
        title="Multimodal Models"
        description="Configure STT, TTS, and Vision models"
      >
        <div className="space-y-4">
          <div>
            <label className="block text-white font-medium mb-2">Speech-to-Text Model</label>
            <select
              value={settings.stt_model || 'whisper-base'}
              onChange={(e) => updateSetting('stt_model', e.target.value)}
              className="w-full bg-gray-700 text-white rounded px-3 py-2 border border-gray-600 focus:border-blue-500 focus:outline-none"
            >
              <option value="whisper-tiny">Whisper Tiny (fast, less accurate)</option>
              <option value="whisper-base">Whisper Base (balanced)</option>
              <option value="whisper-small">Whisper Small (accurate)</option>
              <option value="whisper-medium">Whisper Medium (very accurate, slower)</option>
            </select>
          </div>

          <div>
            <label className="block text-white font-medium mb-2">Text-to-Speech Model</label>
            <select
              value={settings.tts_model || 'piper'}
              onChange={(e) => updateSetting('tts_model', e.target.value)}
              className="w-full bg-gray-700 text-white rounded px-3 py-2 border border-gray-600 focus:border-blue-500 focus:outline-none"
            >
              <option value="piper">Piper (offline, fast)</option>
              <option value="coqui">Coqui TTS (high quality)</option>
            </select>
          </div>

          <div>
            <label className="block text-white font-medium mb-2">Vision Model</label>
            <select
              value={settings.vision_model || 'llama3.2-vision'}
              onChange={(e) => updateSetting('vision_model', e.target.value)}
              className="w-full bg-gray-700 text-white rounded px-3 py-2 border border-gray-600 focus:border-blue-500 focus:outline-none"
            >
              <option value="llama3.2-vision">Llama 3.2 Vision (multimodal)</option>
              <option value="llava">LLaVA (image understanding)</option>
              <option value="bakllava">BakLLaVA (enhanced vision)</option>
            </select>
          </div>
        </div>
      </SettingsSection>

      {/* Audio/Video Devices */}
      <SettingsSection
        icon={<Mic className="w-6 h-6" />}
        title="Audio Devices"
        description="Configure microphone and speaker"
      >
        <div className="space-y-4">
          <div>
            <label className="block text-white font-medium mb-2">Microphone</label>
            <select
              value={settings.audio_input_device || 'default'}
              onChange={(e) => updateSetting('audio_input_device', e.target.value)}
              className="w-full bg-gray-700 text-white rounded px-3 py-2 border border-gray-600 focus:border-blue-500 focus:outline-none"
            >
              <option value="default">System Default</option>
              {/* Device list would be populated dynamically */}
            </select>
            <p className="text-sm text-gray-400 mt-1">
              Choose your preferred microphone for voice input
            </p>
          </div>
        </div>
      </SettingsSection>

      <SettingsSection
        icon={<Video className="w-6 h-6" />}
        title="Video Devices"
        description="Configure webcam for vision input"
      >
        <div className="space-y-4">
          <div>
            <label className="block text-white font-medium mb-2">Camera</label>
            <select
              value={settings.video_input_device || 'default'}
              onChange={(e) => updateSetting('video_input_device', e.target.value)}
              className="w-full bg-gray-700 text-white rounded px-3 py-2 border border-gray-600 focus:border-blue-500 focus:outline-none"
            >
              <option value="default">System Default</option>
              {/* Device list would be populated dynamically */}
            </select>
            <p className="text-sm text-gray-400 mt-1">
              Choose your preferred camera for vision input
            </p>
          </div>
        </div>
      </SettingsSection>

      {/* Privacy Settings */}
      <SettingsSection
        icon={<Shield className="w-6 h-6" />}
        title="Privacy & Security"
        description="Guardian AI protection and monitoring"
      >
        <SettingToggle
          label="PII Detection"
          description="Detect and flag personally identifiable information"
          checked={settings.pii_detection}
          onChange={(value) => updateSetting('pii_detection', value)}
        />
        <SettingToggle
          label="Bias Detection"
          description="Monitor for biased language and recommendations"
          checked={settings.bias_detection}
          onChange={(value) => updateSetting('bias_detection', value)}
        />
        <SettingToggle
          label="Harm Detection"
          description="Check for potentially harmful content"
          checked={settings.harm_detection}
          onChange={(value) => updateSetting('harm_detection', value)}
        />
      </SettingsSection>

      {/* Appearance */}
      <SettingsSection
        icon={<Moon className="w-6 h-6" />}
        title="Appearance"
        description="Customize the interface"
      >
        <div className="py-2">
          <label className="block text-white font-medium mb-2">Theme</label>
          <select
            value={settings.theme}
            onChange={(e) => updateSetting('theme', e.target.value)}
            className="w-full bg-gray-700 text-white rounded px-3 py-2 border border-gray-600 focus:border-blue-500 focus:outline-none"
          >
            <option value="dark">Dark</option>
            <option value="light">Light</option>
            <option value="system">System</option>
          </select>
        </div>
      </SettingsSection>

      {/* Storage */}
      <SettingsSection
        icon={<Database className="w-6 h-6" />}
        title="Storage"
        description="Manage local data storage"
      >
        <div className="space-y-2">
          <div className="flex justify-between text-sm">
            <span className="text-gray-400">Database size</span>
            <span className="text-white">~2.4 MB</span>
          </div>
          <div className="flex justify-between text-sm">
            <span className="text-gray-400">Model cache</span>
            <span className="text-white">~580 MB</span>
          </div>
          <button className="mt-4 px-4 py-2 bg-red-600 text-white rounded hover:bg-red-700 transition text-sm">
            Clear cache
          </button>
        </div>
      </SettingsSection>

      {/* Notifications */}
      <SettingsSection
        icon={<Bell className="w-6 h-6" />}
        title="Notifications"
        description="Configure system alerts"
      >
        <SettingToggle
          label="Enable Notifications"
          description="Show desktop notifications for important events"
          checked={settings.enable_notifications}
          onChange={(value) => updateSetting('enable_notifications', value)}
        />
        <SettingToggle
          label="Enable Sound"
          description="Play sounds for notifications and alerts"
          checked={settings.enable_sound}
          onChange={(value) => updateSetting('enable_sound', value)}
        />
      </SettingsSection>

      {/* System Info */}
      <SettingsSection
        icon={<Info className="w-6 h-6" />}
        title="System Information"
        description="HAI-Net Portal details"
      >
        <div className="space-y-2 text-sm">
          <div className="flex justify-between">
            <span className="text-gray-400">Version</span>
            <span className="text-white">0.40.0</span>
          </div>
          <div className="flex justify-between">
            <span className="text-gray-400">Current Phase</span>
            <span className="text-white">Session 40 - Model Selection Enhancement</span>
          </div>
          <div className="flex justify-between">
            <span className="text-gray-400">Total LOC</span>
            <span className="text-white">39,574</span>
          </div>
          <div className="flex justify-between">
            <span className="text-gray-400">Tests Passing</span>
            <span className="text-white">577</span>
          </div>
          <div className="flex justify-between">
            <span className="text-gray-400">License</span>
            <span className="text-white">Open Source</span>
          </div>
        </div>
      </SettingsSection>
    </div>
  );
}

// Settings Section Component
interface SettingsSectionProps {
  icon: React.ReactNode;
  title: string;
  description: string;
  children: React.ReactNode;
}

function SettingsSection({ icon, title, description, children }: SettingsSectionProps) {
  return (
    <div className="bg-gray-800 rounded-lg p-6">
      <div className="flex items-start gap-4 mb-4">
        <div className="w-12 h-12 bg-blue-500/10 rounded-lg flex items-center justify-center text-blue-500">
          {icon}
        </div>
        <div className="flex-1">
          <h3 className="text-lg font-semibold text-white">{title}</h3>
          <p className="text-sm text-gray-400">{description}</p>
        </div>
      </div>
      <div className="space-y-4">
        {children}
      </div>
    </div>
  );
}

// Setting Toggle Component
interface SettingToggleProps {
  label: string;
  description: string;
  checked: boolean;
  onChange: (value: boolean) => void;
}

function SettingToggle({ label, description, checked, onChange }: SettingToggleProps) {
  return (
    <div className="flex items-center justify-between py-2">
      <div className="flex-1">
        <p className="text-white font-medium">{label}</p>
        <p className="text-sm text-gray-400">{description}</p>
      </div>
      <button
        onClick={() => onChange(!checked)}
        className={`relative w-12 h-6 rounded-full transition-colors ${
          checked ? 'bg-blue-600' : 'bg-gray-600'
        }`}
        aria-label={`Toggle ${label}`}
      >
        <div
          className={`absolute top-0.5 left-0.5 w-5 h-5 bg-white rounded-full transition-transform ${
            checked ? 'translate-x-6' : 'translate-x-0'
          }`}
        />
      </button>
    </div>
  );
}
