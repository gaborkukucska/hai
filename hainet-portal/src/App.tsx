import { BrowserRouter as Router, Routes, Route, Navigate } from 'react-router-dom';
import { Sidebar } from './components/Sidebar';
import Auth from './pages/Auth';
import ChatView from './pages/ChatView';
import SocialFeed from './pages/SocialFeed';
import AgentStudio from './pages/AgentStudio';
import ComputeNode from './pages/ComputeNode';
import NetworkSettings from './pages/NetworkSettings';
import { useState } from 'react';

function App() {
  // Temporary auth state for UI testing
  const [isAuthenticated, setIsAuthenticated] = useState(false);

  // In a real app, AuthRoute would protect the routes
  // For now, we'll just show the app or auth screen based on a manual toggle
  // You can set isAuthenticated to true to see the main app

  return (
    <Router>
      <div className="flex h-screen bg-theme-bg-primary overflow-hidden font-sans text-theme-text-primary">
        
        {/* Temporary Auth Toggle for Development */}
        <div className="fixed top-2 right-2 z-50">
          <button 
            onClick={() => setIsAuthenticated(!isAuthenticated)}
            className="px-3 py-1 bg-theme-accent-primary text-theme-bg-primary text-xs rounded opacity-50 hover:opacity-100"
          >
            Toggle Auth
          </button>
        </div>

        {!isAuthenticated ? (
          <div className="flex-1 w-full h-full">
            <Routes>
              <Route path="*" element={<Auth />} />
            </Routes>
          </div>
        ) : (
          <>
            <Sidebar />
            <main className="flex-1 flex flex-col overflow-hidden relative">
              <Routes>
                <Route path="/" element={<Navigate to="/chat" replace />} />
                <Route path="/chat" element={<ChatView />} />
                <Route path="/feed" element={<SocialFeed />} />
                <Route path="/studio" element={<AgentStudio />} />
                <Route path="/compute" element={<ComputeNode />} />
                <Route path="/network" element={<NetworkSettings />} />
                <Route path="/settings" element={<NetworkSettings />} />
                <Route path="*" element={<Navigate to="/chat" replace />} />
              </Routes>
            </main>
          </>
        )}
      </div>
    </Router>
  );
}

export default App;
