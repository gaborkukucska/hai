import React from 'react';

export default function SocialFeed() {
  return (
    <div className="flex-1 h-full overflow-y-auto bg-theme-bg-primary text-theme-text-primary p-6">
      <div className="max-w-3xl mx-auto space-y-6">
        
        {/* Header */}
        <div className="flex justify-between items-center mb-8">
          <h1 className="text-2xl font-bold">Mesh Social Feed</h1>
          <div className="flex gap-2">
             <button className="px-3 py-1.5 bg-theme-bg-tertiary rounded-md text-sm font-medium">Global</button>
             <button className="px-3 py-1.5 bg-theme-bg-secondary text-theme-text-muted rounded-md text-sm">Following</button>
          </div>
        </div>

        {/* Composer */}
        <div className="bg-theme-bg-secondary border border-theme-border rounded-xl p-4">
          <textarea 
            placeholder="Share something with the mesh..." 
            className="w-full bg-transparent resize-none focus:outline-none min-h-[80px] text-theme-text-primary"
          />
          <div className="flex justify-between items-center mt-2 pt-2 border-t border-theme-border">
            <div className="flex gap-2">
              <button className="p-2 text-theme-text-muted hover:text-theme-accent-primary rounded-full hover:bg-theme-bg-tertiary transition-colors">🖼️</button>
              <button className="p-2 text-theme-text-muted hover:text-theme-accent-primary rounded-full hover:bg-theme-bg-tertiary transition-colors">🎥</button>
            </div>
            <button className="px-4 py-1.5 bg-theme-accent-primary text-theme-bg-primary font-bold rounded-full hover:bg-theme-accent-secondary transition-colors text-sm">
              Post to Mesh
            </button>
          </div>
        </div>

        {/* Feed Posts Placeholder */}
        <div className="space-y-4">
          <div className="bg-theme-bg-secondary border border-theme-border rounded-xl p-5">
            <div className="flex items-center gap-3 mb-3">
               <div className="w-10 h-10 rounded-full bg-theme-bg-tertiary"></div>
               <div>
                 <p className="font-semibold text-sm">Satoshi Node</p>
                 <p className="text-xs text-theme-text-muted">10 minutes ago via P2P</p>
               </div>
            </div>
            <p className="text-theme-text-secondary text-sm">
              Testing the new HAI-Net decentralized feed. The integration with TrippleEffect agents means we can auto-generate content directly into the mesh! 🚀
            </p>
          </div>
        </div>

      </div>
    </div>
  );
}
