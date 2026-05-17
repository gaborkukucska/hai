import React, { useState } from 'react';

export default function Auth() {
  const [isGenerating, setIsGenerating] = useState(false);

  const handleGenerate = () => {
    setIsGenerating(true);
    // Placeholder for actual P2P key generation logic
    setTimeout(() => {
      setIsGenerating(false);
      // Navigate or set auth state here
    }, 2000);
  };

  return (
    <div className="flex flex-col items-center justify-center h-full bg-theme-bg-primary text-theme-text-primary p-4">
      <div className="max-w-md w-full bg-theme-bg-secondary border border-theme-border rounded-xl shadow-xl overflow-hidden">
        <div className="p-8 text-center border-b border-theme-border">
          <h1 className="text-3xl font-bold text-theme-accent-primary mb-2">HAI-Net Portal</h1>
          <p className="text-theme-text-muted">Decentralized P2P Mesh Network</p>
        </div>
        
        <div className="p-8 space-y-6">
          <div className="space-y-2 text-center">
            <h2 className="text-xl font-semibold">Create Your Node Identity</h2>
            <p className="text-sm text-theme-text-secondary">
              Generate a unique cryptographic keypair to participate in the decentralized mesh.
            </p>
          </div>

          <div className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-theme-text-secondary mb-1">Display Name</label>
              <input 
                type="text" 
                placeholder="How others will see you"
                className="w-full bg-theme-bg-tertiary border border-theme-border rounded-md px-4 py-2 text-theme-text-primary focus:outline-none focus:ring-1 focus:ring-theme-accent-primary"
              />
            </div>
            
            <button 
              onClick={handleGenerate}
              disabled={isGenerating}
              className="w-full bg-theme-accent-primary hover:bg-theme-accent-secondary text-theme-bg-primary font-bold py-2.5 px-4 rounded-md transition-colors flex justify-center items-center"
            >
              {isGenerating ? 'Generating Keys...' : 'Generate P2P Identity'}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
