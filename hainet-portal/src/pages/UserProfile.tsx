import React, { useState, useEffect } from 'react';
import { User, Save, Shield, HardDrive, Share2 } from 'lucide-react';
import { invoke } from '../lib/tauri';

interface UserProfileData {
  username: string;
  nodeName: string;
  bio: string;
  isPublic: boolean;
}

export default function UserProfile() {
  const [profile, setProfile] = useState<UserProfileData>({
    username: 'admin',
    nodeName: 'Primary Node',
    bio: 'HAI-Net System Administrator',
    isPublic: true,
  });
  const [peerCount, setPeerCount] = useState<number>(0);
  const [saveStatus, setSaveStatus] = useState<'idle' | 'saving' | 'saved' | 'error'>('idle');

  // Since there's no backend DB for profile yet, we just simulate loading from local storage
  useEffect(() => {
    const saved = localStorage.getItem('hai_user_profile');
    if (saved) {
      try {
        setProfile(JSON.parse(saved));
      } catch (e) {
        console.error('Failed to parse profile', e);
      }
    }

    const fetchPeerCount = async () => {
      try {
        const result = await invoke<{ peer_count: number }>('get_peer_count');
        setPeerCount(result?.peer_count || 0);
      } catch (e) {
        console.debug('Failed to get peer count', e);
      }
    };
    fetchPeerCount();
  }, []);

  const handleSave = () => {
    setSaveStatus('saving');
    try {
      localStorage.setItem('hai_user_profile', JSON.stringify(profile));
      setTimeout(() => setSaveStatus('saved'), 500);
      setTimeout(() => setSaveStatus('idle'), 2500);
    } catch (error) {
      setSaveStatus('error');
      setTimeout(() => setSaveStatus('idle'), 2500);
    }
  };

  return (
    <div className="flex-1 overflow-y-auto px-4 py-6 space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-bold text-white">User Profile</h2>
          <p className="text-sm text-gray-400">Manage your node identity and privacy</p>
        </div>
        <button
          onClick={handleSave}
          className="flex items-center gap-2 px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded-md transition-colors"
        >
          <Save size={16} />
          {saveStatus === 'saving' ? 'Saving...' : saveStatus === 'saved' ? 'Saved!' : 'Save Profile'}
        </button>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
        <div className="col-span-1 space-y-6">
          {/* Avatar / Identity Card */}
          <div className="bg-gray-800 rounded-lg p-6 flex flex-col items-center text-center border border-gray-700">
            <div className="w-24 h-24 bg-gray-700 rounded-full flex items-center justify-center mb-4 border-2 border-blue-500 shadow-lg">
              <User size={48} className="text-blue-400" />
            </div>
            <h3 className="text-xl font-bold text-white">{profile.username}</h3>
            <p className="text-sm text-gray-400 mb-4">{profile.nodeName}</p>
            
            <div className="w-full pt-4 border-t border-gray-700 flex justify-between text-sm">
              <span className="text-gray-400">Node Status</span>
              <span className="text-green-400 flex items-center gap-1">
                <span className="w-2 h-2 rounded-full bg-green-400 animate-pulse"></span>
                Online
              </span>
            </div>
            <div className="w-full pt-2 flex justify-between text-sm">
              <span className="text-gray-400">Mesh Peers</span>
              <span className="text-blue-400">{peerCount} Connected</span>
            </div>
          </div>
        </div>

        <div className="col-span-2 space-y-6">
          {/* Profile Details */}
          <div className="bg-gray-800 rounded-lg p-6 border border-gray-700">
            <h3 className="text-lg font-semibold text-white mb-4 flex items-center gap-2">
              <User size={18} className="text-blue-500" />
              Identity Details
            </h3>
            
            <div className="space-y-4">
              <div>
                <label className="block text-sm font-medium text-gray-400 mb-1">Username</label>
                <input
                  type="text"
                  value={profile.username}
                  onChange={(e) => setProfile({ ...profile, username: e.target.value })}
                  className="w-full bg-gray-700 border border-gray-600 rounded-md px-3 py-2 text-white focus:outline-none focus:border-blue-500"
                />
              </div>
              <div>
                <label className="block text-sm font-medium text-gray-400 mb-1">Node Display Name</label>
                <input
                  type="text"
                  value={profile.nodeName}
                  onChange={(e) => setProfile({ ...profile, nodeName: e.target.value })}
                  className="w-full bg-gray-700 border border-gray-600 rounded-md px-3 py-2 text-white focus:outline-none focus:border-blue-500"
                />
              </div>
              <div>
                <label className="block text-sm font-medium text-gray-400 mb-1">Bio / Description</label>
                <textarea
                  value={profile.bio}
                  onChange={(e) => setProfile({ ...profile, bio: e.target.value })}
                  rows={3}
                  className="w-full bg-gray-700 border border-gray-600 rounded-md px-3 py-2 text-white focus:outline-none focus:border-blue-500"
                />
              </div>
            </div>
          </div>

          {/* Privacy Settings */}
          <div className="bg-gray-800 rounded-lg p-6 border border-gray-700">
            <h3 className="text-lg font-semibold text-white mb-4 flex items-center gap-2">
              <Shield size={18} className="text-blue-500" />
              Privacy & Discovery
            </h3>
            
            <div className="space-y-4">
              <label className="flex items-center justify-between cursor-pointer p-3 bg-gray-700/50 rounded-md border border-gray-700">
                <div>
                  <div className="text-white font-medium">Public Mesh Visibility</div>
                  <div className="text-sm text-gray-400">Allow other nodes to discover your profile info</div>
                </div>
                <div className="relative">
                  <input
                    type="checkbox"
                    className="sr-only"
                    checked={profile.isPublic}
                    onChange={(e) => setProfile({ ...profile, isPublic: e.target.checked })}
                  />
                  <div className={`block w-10 h-6 rounded-full transition-colors ${profile.isPublic ? 'bg-blue-600' : 'bg-gray-600'}`}></div>
                  <div className={`dot absolute left-1 top-1 bg-white w-4 h-4 rounded-full transition-transform ${profile.isPublic ? 'transform translate-x-4' : ''}`}></div>
                </div>
              </label>
            </div>
          </div>
          
        </div>
      </div>
    </div>
  );
}
