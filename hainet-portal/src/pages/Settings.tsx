//! # START OF FILE hainet-portal/src/pages/Settings.tsx
import React, { useEffect, useState } from 'react';
import { Shield, Moon, Database, Bell, Info } from 'lucide-react';
import { invoke } from '../lib/tauri';
import type { Settings as SettingsType } from '../types';

export default function Settings() {
  const [settings, setSettings] = useState<SettingsType | null>(null);
  const [loading, setLoading] = useState(true);
  const [saveStatus, setSaveStatus] = useState<'idle' | 'saving' | 'saved' | 'error'>('idle');

  // Load settings on component mount
  useEffect(() => {
    loadSettings();
  }, []);

  const loadSettings = async () => {
    try {
      const loadedSettings = await invoke<SettingsType>('get_settings');
      setSettings(loadedSettings);
    } catch (error) {
      console.error('Failed to load settings:', error);
    } finally {
      setLoading(false);
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
            <span className="text-white">0.19.0</span>
          </div>
          <div className="flex justify-between">
            <span className="text-gray-400">Phase</span>
            <span className="text-white">6B - Portal UI Enhancements</span>
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
