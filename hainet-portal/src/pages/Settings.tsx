//! # START OF FILE hainet-portal/src/pages/Settings.tsx
import React from 'react';
import { Shield, Moon, Database, Bell, Info } from 'lucide-react';

export default function Settings() {
  return (
    <div className="flex-1 overflow-y-auto px-4 py-6 space-y-6">
      {/* Header */}
      <div>
        <h2 className="text-2xl font-bold text-white">Settings</h2>
        <p className="text-sm text-gray-400">Configure your HAI-Net Portal</p>
      </div>

      {/* Privacy Settings */}
      <SettingsSection
        icon={<Shield className="w-6 h-6" />}
        title="Privacy & Security"
        description="All data remains local and private"
      >
        <SettingToggle
          label="Local-only mode"
          description="Never connect to external services"
          defaultChecked={true}
        />
        <SettingToggle
          label="Encrypt local data"
          description="AES-256 encryption for all stored data"
          defaultChecked={true}
        />
        <SettingToggle
          label="Audit logging"
          description="Track all AI interactions for transparency"
          defaultChecked={true}
        />
      </SettingsSection>

      {/* Appearance */}
      <SettingsSection
        icon={<Moon className="w-6 h-6" />}
        title="Appearance"
        description="Customize the interface"
      >
        <SettingToggle
          label="Dark mode"
          description="Use dark theme (currently active)"
          defaultChecked={true}
        />
        <SettingToggle
          label="Reduced motion"
          description="Minimize animations for accessibility"
          defaultChecked={false}
        />
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
          label="Guardian alerts"
          description="Notify when constitutional violations detected"
          defaultChecked={true}
        />
        <SettingToggle
          label="Task completion"
          description="Alert when agent tasks complete"
          defaultChecked={true}
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
  defaultChecked?: boolean;
}

function SettingToggle({ label, description, defaultChecked = false }: SettingToggleProps) {
  const [checked, setChecked] = React.useState(defaultChecked);

  return (
    <div className="flex items-center justify-between py-2">
      <div className="flex-1">
        <p className="text-white font-medium">{label}</p>
        <p className="text-sm text-gray-400">{description}</p>
      </div>
      <button
        onClick={() => setChecked(!checked)}
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
