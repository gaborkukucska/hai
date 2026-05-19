import React, { useState } from 'react';

export default function ComputeNode() {
  const [isContributing, setIsContributing] = useState(true);

  return (
    <div className="flex-1 h-full overflow-y-auto bg-theme-bg-primary text-theme-text-primary p-6">
      <div className="max-w-5xl mx-auto space-y-6">
        
        <div className="flex justify-between items-center mb-8">
          <div>
            <h1 className="text-2xl font-bold">Compute Router</h1>
            <p className="text-theme-text-muted text-sm mt-1">Resource sharing and hardware profiling (Powered by pplpwr)</p>
          </div>
          <div className="flex items-center gap-3">
            <span className="text-sm text-theme-text-muted">Contribution:</span>
            <button 
              onClick={() => setIsContributing(!isContributing)}
              className={`w-12 h-6 rounded-full relative cursor-pointer transition-colors ${isContributing ? 'bg-theme-accent-success' : 'bg-theme-bg-tertiary'}`}>
               <div className={`w-4 h-4 rounded-full bg-white absolute top-1 transition-all ${isContributing ? 'right-1' : 'left-1'}`}></div>
            </button>
          </div>
        </div>

        {/* Hardware Profile */}
        <div className="bg-theme-bg-secondary border border-theme-border rounded-xl p-5 mb-6">
          <h2 className="text-lg font-semibold mb-4">Local Hardware Profile</h2>
          <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
             <div className="bg-theme-bg-tertiary rounded-lg p-4">
                <p className="text-xs text-theme-text-muted uppercase tracking-wider">CPU</p>
                <p className="text-lg font-medium mt-1">24 Cores (Ryzen 9)</p>
                <p className="text-xs text-theme-accent-success mt-2">12% Utilization</p>
             </div>
             <div className="bg-theme-bg-tertiary rounded-lg p-4 border border-theme-accent-primary/50">
                <p className="text-xs text-theme-text-muted uppercase tracking-wider">GPU (Primary Compute)</p>
                <p className="text-lg font-medium mt-1">RTX 3060 (12GB)</p>
                {isContributing ? (
                  <p className="text-xs text-theme-accent-success mt-2">Active (84% Usage)</p>
                ) : (
                  <p className="text-xs text-theme-text-secondary mt-2">Idle (0% Usage)</p>
                )}
             </div>
             <div className="bg-theme-bg-tertiary rounded-lg p-4">
                <p className="text-xs text-theme-text-muted uppercase tracking-wider">RAM</p>
                <p className="text-lg font-medium mt-1">31 GB Total</p>
                <p className="text-xs text-theme-text-secondary mt-2">14 GB Available</p>
             </div>
          </div>
        </div>

        {/* Networks */}
        <h2 className="text-lg font-semibold mb-4 mt-8">Active Mesh Networks</h2>
        <div className="space-y-3">
           <div className="bg-theme-bg-secondary border border-theme-border rounded-lg p-4 flex items-center justify-between">
              <div>
                 <h3 className="font-medium text-theme-text-primary">Petals Network</h3>
                 <p className="text-xs text-theme-text-muted">Serving Llama-3-70B blocks</p>
              </div>
              <div className="text-right">
                 <p className="text-sm font-medium">Earned: 1,240 compute units</p>
                 <p className="text-xs text-theme-accent-success">Active</p>
              </div>
           </div>
           
           <div className="bg-theme-bg-secondary border border-theme-border rounded-lg p-4 flex items-center justify-between opacity-60">
              <div>
                 <h3 className="font-medium text-theme-text-primary">Prime Intellect</h3>
                 <p className="text-xs text-theme-text-muted">Distributed Training</p>
              </div>
              <div className="text-right">
                 <button className="px-3 py-1 bg-theme-bg-tertiary rounded text-xs hover:bg-theme-border">Connect Node</button>
              </div>
           </div>
        </div>

      </div>
    </div>
  );
}
